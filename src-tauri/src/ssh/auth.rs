//! User authentication on an established SSH transport.

use std::path::Path;
use std::sync::Arc;

use russh::client::{self, KeyboardInteractiveAuthResponse};
use russh::keys::agent::client::AgentClient;
use russh::keys::agent::AgentIdentity;
use russh::keys::{self, PrivateKeyWithHashAlg};
use tokio::io::{AsyncRead, AsyncWrite};

use super::handler::ClientHandler;
use crate::error::{AppError, AppResult};
use crate::hosts::ssh_config::expand_tilde;
use crate::hosts::{AuthKind, Host};

type Handle = client::Handle<ClientHandler>;

/// Secrets the user typed for this attempt. Stored ones are looked up by the caller.
#[derive(Default, Clone)]
pub struct Secrets {
    pub password: Option<String>,
    pub passphrase: Option<String>,
}

pub async fn authenticate(handle: &mut Handle, host: &Host, secrets: &Secrets) -> AppResult<()> {
    match host.auth {
        AuthKind::Password => {
            let password = secrets
                .password
                .clone()
                .ok_or_else(|| AppError::NeedPassword {
                    host_id: host.id.clone(),
                    hostname: host.hostname.clone(),
                    username: host.username.clone(),
                })?;
            password_auth(handle, &host.username, &password).await
        }
        AuthKind::Key => {
            let path = expand_tilde(host.key_path.as_deref().unwrap_or_default());
            key_auth(
                handle,
                host,
                Path::new(&path),
                secrets.passphrase.as_deref(),
            )
            .await
        }
        AuthKind::Agent => agent_auth(handle, &host.username).await,
    }
}

async fn password_auth(handle: &mut Handle, user: &str, password: &str) -> AppResult<()> {
    if handle
        .authenticate_password(user, password)
        .await?
        .success()
    {
        return Ok(());
    }
    // Many servers disable "password" but accept the same password over
    // keyboard-interactive, so answer every prompt with it.
    let mut reply = handle
        .authenticate_keyboard_interactive_start(user, None)
        .await?;
    for _ in 0..4 {
        match reply {
            KeyboardInteractiveAuthResponse::Success => return Ok(()),
            KeyboardInteractiveAuthResponse::Failure { .. } => break,
            KeyboardInteractiveAuthResponse::InfoRequest { prompts, .. } => {
                let answers = prompts.iter().map(|_| password.to_string()).collect();
                reply = handle
                    .authenticate_keyboard_interactive_respond(answers)
                    .await?;
            }
        }
    }
    Err(AppError::Auth("the server rejected the password".into()))
}

async fn key_auth(
    handle: &mut Handle,
    host: &Host,
    path: &Path,
    passphrase: Option<&str>,
) -> AppResult<()> {
    let key = match keys::load_secret_key(path, passphrase) {
        Ok(key) => key,
        Err(keys::Error::KeyIsEncrypted) => {
            return Err(AppError::NeedPassphrase {
                host_id: host.id.clone(),
                key_path: path.display().to_string(),
            })
        }
        Err(_) if passphrase.is_some() => {
            return Err(AppError::Auth("wrong passphrase for the key".into()))
        }
        Err(e) => {
            return Err(AppError::Auth(format!(
                "cannot read key {}: {e}",
                path.display()
            )))
        }
    };
    let hash = handle.best_supported_rsa_hash().await?.flatten();
    let ok = handle
        .authenticate_publickey(
            &host.username,
            PrivateKeyWithHashAlg::new(Arc::new(key), hash),
        )
        .await?
        .success();
    if ok {
        Ok(())
    } else {
        Err(AppError::Auth("the server rejected the key".into()))
    }
}

async fn agent_auth(handle: &mut Handle, user: &str) -> AppResult<()> {
    #[cfg(windows)]
    {
        if let Ok(agent) = AgentClient::connect_named_pipe(r"\\.\pipe\openssh-ssh-agent").await {
            if try_agent(handle, user, agent).await? {
                return Ok(());
            }
        }
        if let Ok(agent) = AgentClient::connect_pageant().await {
            if try_agent(handle, user, agent).await? {
                return Ok(());
            }
        }
    }
    #[cfg(unix)]
    {
        if let Ok(agent) = AgentClient::connect_env().await {
            if try_agent(handle, user, agent).await? {
                return Ok(());
            }
        }
    }
    Err(AppError::Auth(
        "no SSH agent key was accepted (is the agent running and the key added?)".into(),
    ))
}

async fn try_agent<S>(handle: &mut Handle, user: &str, mut agent: AgentClient<S>) -> AppResult<bool>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let identities = match agent.request_identities().await {
        Ok(ids) => ids,
        Err(_) => return Ok(false),
    };
    let hash = handle.best_supported_rsa_hash().await?.flatten();
    for identity in identities {
        let AgentIdentity::PublicKey { key, .. } = identity else {
            continue;
        };
        match handle
            .authenticate_publickey_with(user, key, hash, &mut agent)
            .await
        {
            Ok(res) if res.success() => return Ok(true),
            Ok(_) => {}
            Err(e) => tracing::debug!("agent key rejected: {e}"),
        }
    }
    Ok(false)
}
