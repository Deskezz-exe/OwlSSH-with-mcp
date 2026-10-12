use tauri::{AppHandle, Emitter, State};
use tokio_util::sync::CancellationToken;

use crate::error::AppResult;
use crate::ssh::{forward, ConnectOptions};
use crate::state::AppState;
use crate::tunnels::{Tunnel, TunnelInput, TunnelKind, TunnelStatus};

/// Tells the UI to reload tunnel statuses.
fn changed(app: &AppHandle) {
    let _ = app.emit("tunnels-changed", ());
}

#[tauri::command]
pub fn list_tunnels(state: State<'_, AppState>) -> Vec<Tunnel> {
    state.tunnels.list()
}

#[tauri::command]
pub fn tunnel_statuses(state: State<'_, AppState>) -> Vec<TunnelStatus> {
    state.tunnel_runtime.statuses()
}

/// Saves a tunnel. A running one is stopped first so it never keeps old ports.
#[tauri::command]
pub fn save_tunnel(
    app: AppHandle,
    state: State<'_, AppState>,
    input: TunnelInput,
) -> AppResult<Tunnel> {
    if let Some(id) = &input.id {
        state.tunnel_runtime.stop(id);
        changed(&app);
    }
    state.tunnels.save(input)
}

#[tauri::command]
pub fn delete_tunnel(app: AppHandle, state: State<'_, AppState>, id: String) -> AppResult<()> {
    state.tunnel_runtime.stop(&id);
    changed(&app);
    state.tunnels.delete(&id)
}

/// Starts a tunnel on the host's connection (the UI connects the host first,
/// answering password and host key questions) and returns the port in use.
#[tauri::command]
pub async fn start_tunnel(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> AppResult<u16> {
    let tunnel = state.tunnels.get(&id)?;
    let cancel = CancellationToken::new();
    if !state
        .tunnel_runtime
        .begin(&id, &tunnel.host_id, cancel.clone())
    {
        return Ok(state
            .tunnel_runtime
            .statuses()
            .into_iter()
            .find(|s| s.id == id)
            .and_then(|s| s.port)
            .unwrap_or(tunnel.bind_port));
    }
    changed(&app);

    let runtime = state.tunnel_runtime.clone();
    let on_end: forward::OnEnd = {
        let app = app.clone();
        let id = id.clone();
        Box::new(move |error| {
            runtime.finish(&id, error);
            changed(&app);
        })
    };
    let started = async {
        let conn = state
            .pool
            .get_or_connect(&state.hosts, &tunnel.host_id, &ConnectOptions::default())
            .await?;
        match tunnel.kind {
            TunnelKind::Local => {
                forward::start_local(
                    conn,
                    &tunnel.bind_host,
                    tunnel.bind_port,
                    tunnel.target_host.clone(),
                    tunnel.target_port,
                    cancel,
                    on_end,
                )
                .await
            }
            TunnelKind::Remote => {
                forward::start_remote(
                    conn,
                    state.pool.remote_forwards(),
                    &tunnel.host_id,
                    &tunnel.bind_host,
                    tunnel.bind_port,
                    tunnel.target_host.clone(),
                    tunnel.target_port,
                    cancel,
                    on_end,
                )
                .await
            }
        }
    }
    .await;

    match started {
        Ok(port) => {
            state.tunnel_runtime.started(&id, port);
            changed(&app);
            Ok(port)
        }
        Err(e) => {
            state.tunnel_runtime.finish(&id, Some(e.to_string()));
            changed(&app);
            Err(e)
        }
    }
}

#[tauri::command]
pub fn stop_tunnel(app: AppHandle, state: State<'_, AppState>, id: String) {
    state.tunnel_runtime.stop(&id);
    changed(&app);
}
