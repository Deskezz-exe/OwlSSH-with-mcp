use std::time::Duration;

use tauri::State;

use crate::error::{AppError, AppResult};
use crate::hosts::{AuthKind, Host};
use crate::ssh::exec::{exec, ExecOutput};
use crate::ssh::keysetup;
use crate::ssh::metrics::{self, Metrics};
use crate::ssh::ConnectOptions;
use crate::state::AppState;

/// Connects (or reuses the open connection). Errors such as `needPassword` or
/// `hostKeyUnknown` are answered by calling this again with `options` filled in.
#[tauri::command]
pub async fn connect_host(
    state: State<'_, AppState>,
    host_id: String,
    options: Option<ConnectOptions>,
) -> AppResult<()> {
    state
        .pool
        .get_or_connect(&state.hosts, &host_id, &options.unwrap_or_default())
        .await?;
    Ok(())
}

#[tauri::command]
pub async fn disconnect_host(state: State<'_, AppState>, host_id: String) -> AppResult<()> {
    state.terminals.close_host(&host_id);
    state.tunnel_runtime.stop_host(&host_id);
    state.sftp.forget(&host_id);
    state.pool.disconnect(&host_id).await;
    Ok(())
}

#[tauri::command]
pub fn connected_hosts(state: State<'_, AppState>) -> Vec<String> {
    state.pool.connected_ids()
}

#[tauri::command]
pub async fn run_command(
    state: State<'_, AppState>,
    host_id: String,
    command: String,
) -> AppResult<ExecOutput> {
    let conn = state
        .pool
        .get_or_connect(&state.hosts, &host_id, &ConnectOptions::default())
        .await?;
    exec(&conn, &command, Duration::from_secs(60)).await
}

/// Health numbers for a connected host. Does not connect on its own, so the
/// dashboard never opens connections the user did not ask for.
#[tauri::command]
pub async fn host_metrics(
    state: State<'_, AppState>,
    host_id: String,
) -> AppResult<Option<Metrics>> {
    match state.pool.existing(&host_id) {
        Some(conn) => Ok(Some(metrics::collect(&conn).await?)),
        None => Ok(None),
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeySetup {
    pub host: Host,
    pub key_path: String,
    /// A new key was created (otherwise an existing one was reused).
    pub created: bool,
}

/// Sets up key login on a host the user can already reach: installs a local
/// key on the server, checks that it logs in, then switches the host to it.
#[tauri::command]
pub async fn setup_key(state: State<'_, AppState>, host_id: String) -> AppResult<KeySetup> {
    let host = state.hosts.get(&host_id)?;
    let conn = state
        .pool
        .get_or_connect(&state.hosts, &host_id, &ConnectOptions::default())
        .await?;
    let ssh_dir = dirs::home_dir()
        .ok_or_else(|| AppError::NotFound("home folder".into()))?
        .join(".ssh");
    let key = keysetup::ensure_key(&ssh_dir)?;
    keysetup::install(&conn, &key.public_line).await?;

    let key_path = key.path.display().to_string();
    let mut with_key = host.clone();
    with_key.auth = AuthKind::Key;
    with_key.key_path = Some(key_path.clone());
    match state.pool.verify_login(&state.hosts, &with_key).await {
        // An existing key with a passphrase cannot be tried here; the next
        // connect asks for the passphrase as usual.
        Ok(()) | Err(AppError::NeedPassphrase { .. }) => {}
        Err(e) => {
            return Err(AppError::Auth(format!(
                "the key was added on the server, but logging in with it failed: {e}"
            )))
        }
    }
    let host = state.hosts.use_key(&host_id, &key_path)?;
    Ok(KeySetup {
        host,
        key_path,
        created: key.created,
    })
}
