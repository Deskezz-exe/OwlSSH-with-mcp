use std::time::Duration;

use tauri::State;

use crate::error::AppResult;
use crate::ssh::exec::{exec, ExecOutput};
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
