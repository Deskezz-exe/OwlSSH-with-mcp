//! One SSH connection per server, shared by terminals, SFTP, metrics and MCP.
//! Each user of a connection opens its own channel on it.

use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use russh::client;
use serde::Deserialize;

use super::auth::{self, Secrets};
use super::forward::RemoteForwards;
use super::handler::{ClientHandler, HostKeyIssue};
use crate::error::{AppError, AppResult};
use crate::hosts::{AuthKind, Host, HostStore};
use crate::secrets;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_JUMPS: usize = 4;

pub struct SshConn {
    pub handle: client::Handle<ClientHandler>,
    /// Keeps the jump connection alive while this one tunnels through it.
    _via: Option<Arc<SshConn>>,
}

impl SshConn {
    pub fn is_closed(&self) -> bool {
        self.handle.is_closed()
    }
}

/// What the UI sends along with a connect request.
#[derive(Debug, Default, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectOptions {
    pub password: Option<String>,
    #[serde(default)]
    pub save_password: bool,
    pub passphrase: Option<String>,
    /// Fingerprint the user accepted in the host key dialog.
    pub trust_fingerprint: Option<String>,
}

pub struct SshPool {
    known_hosts: PathBuf,
    conns: Mutex<HashMap<String, Arc<SshConn>>>,
    /// Serializes connects per host so two tabs never open two connections.
    locks: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
    /// Key passphrases typed this session, by key path. Memory only.
    passphrases: Mutex<HashMap<String, String>>,
    forwards: RemoteForwards,
}

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

impl SshPool {
    pub fn new(known_hosts: PathBuf) -> Self {
        Self {
            known_hosts,
            conns: Mutex::default(),
            locks: Mutex::default(),
            passphrases: Mutex::default(),
            forwards: RemoteForwards::default(),
        }
    }

    pub fn remote_forwards(&self) -> RemoteForwards {
        self.forwards.clone()
    }

    /// An open connection for `host_id`, if there is a live one.
    pub fn existing(&self, host_id: &str) -> Option<Arc<SshConn>> {
        let conns = self.conns.lock().unwrap();
        conns.get(host_id).filter(|c| !c.is_closed()).cloned()
    }

    pub fn connected_ids(&self) -> Vec<String> {
        let conns = self.conns.lock().unwrap();
        conns
            .iter()
            .filter(|(_, c)| !c.is_closed())
            .map(|(id, _)| id.clone())
            .collect()
    }

    pub async fn get_or_connect(
        &self,
        hosts: &HostStore,
        host_id: &str,
        opts: &ConnectOptions,
    ) -> AppResult<Arc<SshConn>> {
        self.connect_inner(hosts, host_id, opts, 0).await
    }

    pub async fn disconnect(&self, host_id: &str) {
        let conn = self.conns.lock().unwrap().remove(host_id);
        if let Some(conn) = conn {
            let _ = conn
                .handle
                .disconnect(russh::Disconnect::ByApplication, "", "en")
                .await;
        }
    }

    /// Logs in to `host` on a separate connection and closes it again; proves
    /// that its credentials work without touching the shared connection.
    pub async fn verify_login(&self, hosts: &HostStore, host: &Host) -> AppResult<()> {
        let via = match &host.jump_host_id {
            Some(jump_id) => Some(
                self.connect_inner(hosts, jump_id, &ConnectOptions::default(), 1)
                    .await?,
            ),
            None => None,
        };
        let conn = self.open(host, via, &ConnectOptions::default()).await?;
        let _ = conn
            .handle
            .disconnect(russh::Disconnect::ByApplication, "", "en")
            .await;
        Ok(())
    }

    fn connect_inner<'a>(
        &'a self,
        hosts: &'a HostStore,
        host_id: &'a str,
        opts: &'a ConnectOptions,
        depth: usize,
    ) -> BoxFuture<'a, AppResult<Arc<SshConn>>> {
        Box::pin(async move {
            if depth > MAX_JUMPS {
                return Err(AppError::Invalid("too many jump hosts in a chain".into()));
            }
            let lock = self
                .locks
                .lock()
                .unwrap()
                .entry(host_id.to_string())
                .or_default()
                .clone();
            let _guard = lock.lock().await;

            if let Some(conn) = self.existing(host_id) {
                return Ok(conn);
            }

            let host = hosts.get(host_id)?;
            let via = match &host.jump_host_id {
                // Jump hosts connect with their own stored credentials.
                Some(jump_id) => Some(
                    self.connect_inner(hosts, jump_id, &ConnectOptions::default(), depth + 1)
                        .await?,
                ),
                None => None,
            };

            let conn = Arc::new(self.open(&host, via, opts).await?);

            if host.auth == AuthKind::Password && opts.save_password {
                if let Some(password) = &opts.password {
                    secrets::set_password(&host.id, password)?;
                    hosts.mark_password(&host.id, true)?;
                }
            }
            self.conns
                .lock()
                .unwrap()
                .insert(host.id.clone(), conn.clone());
            Ok(conn)
        })
    }

    async fn open(
        &self,
        host: &Host,
        via: Option<Arc<SshConn>>,
        opts: &ConnectOptions,
    ) -> AppResult<SshConn> {
        let issue = Arc::new(Mutex::new(None));
        let handler = ClientHandler {
            hostname: host.hostname.clone(),
            port: host.port,
            app_known_hosts: self.known_hosts.clone(),
            trusted_fingerprint: opts.trust_fingerprint.clone(),
            issue: issue.clone(),
            host_id: host.id.clone(),
            forwards: self.forwards.clone(),
        };
        let config = client_config();

        let connecting = async {
            match &via {
                Some(jump) => {
                    let channel = jump
                        .handle
                        .channel_open_direct_tcpip(
                            host.hostname.clone(),
                            u32::from(host.port),
                            "127.0.0.1",
                            0,
                        )
                        .await?;
                    client::connect_stream(config, channel.into_stream(), handler).await
                }
                None => client::connect(config, (host.hostname.as_str(), host.port), handler).await,
            }
        };
        let result = tokio::time::timeout(CONNECT_TIMEOUT, connecting)
            .await
            .map_err(|_| {
                AppError::Timeout(format!(
                    "{}:{} did not answer in {}s",
                    host.hostname,
                    host.port,
                    CONNECT_TIMEOUT.as_secs()
                ))
            })?;

        let mut handle = match result {
            Ok(handle) => handle,
            Err(e) => {
                let rejected = issue.lock().unwrap().take();
                return Err(match rejected {
                    Some(HostKeyIssue::Unknown {
                        fingerprint,
                        algorithm,
                    }) => AppError::HostKeyUnknown {
                        host_id: host.id.clone(),
                        hostname: host.hostname.clone(),
                        port: host.port,
                        fingerprint,
                        algorithm,
                    },
                    Some(HostKeyIssue::Changed {
                        fingerprint,
                        algorithm,
                    }) => AppError::HostKeyChanged {
                        host_id: host.id.clone(),
                        hostname: host.hostname.clone(),
                        port: host.port,
                        fingerprint,
                        algorithm,
                    },
                    None => AppError::Ssh(format!(
                        "cannot connect to {}:{}: {e}",
                        host.hostname, host.port
                    )),
                });
            }
        };

        let secrets = self.secrets_for(host, opts)?;
        auth::authenticate(&mut handle, host, &secrets).await?;

        if let (Some(passphrase), Some(path)) = (&opts.passphrase, &host.key_path) {
            self.passphrases
                .lock()
                .unwrap()
                .insert(path.clone(), passphrase.clone());
        }

        Ok(SshConn { handle, _via: via })
    }

    fn secrets_for(&self, host: &Host, opts: &ConnectOptions) -> AppResult<Secrets> {
        let password = match &opts.password {
            Some(p) => Some(p.clone()),
            None if host.has_password => secrets::get_password(&host.id)?,
            None => None,
        };
        let passphrase = opts.passphrase.clone().or_else(|| {
            let path = host.key_path.as_ref()?;
            self.passphrases.lock().unwrap().get(path).cloned()
        });
        Ok(Secrets {
            password,
            passphrase,
        })
    }
}

fn client_config() -> Arc<client::Config> {
    Arc::new(client::Config {
        // Without TCP_NODELAY, Nagle's algorithm holds back small writes such as
        // single keystrokes until the previous packet is acknowledged. Combined
        // with delayed ACKs on the server that is a visible typing lag.
        nodelay: true,
        keepalive_interval: Some(Duration::from_secs(20)),
        keepalive_max: 3,
        inactivity_timeout: None,
        ..Default::default()
    })
}
