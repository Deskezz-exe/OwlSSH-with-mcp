use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{AppHandle, Emitter, State};

use crate::error::AppResult;
use crate::ssh::ConnectOptions;
use crate::state::AppState;

/// Opens a shell. Output arrives on `output` as raw bytes; the end of the
/// session is announced with a `terminal-exit` event.
#[tauri::command]
pub async fn terminal_open(
    app: AppHandle,
    state: State<'_, AppState>,
    host_id: String,
    cols: u32,
    rows: u32,
    output: Channel<InvokeResponseBody>,
) -> AppResult<u32> {
    let conn = state
        .pool
        .get_or_connect(&state.hosts, &host_id, &ConnectOptions::default())
        .await?;
    state
        .terminals
        .open(conn, &host_id, cols, rows, output, move |exit| {
            let _ = app.emit("terminal-exit", exit);
        })
        .await
}

/// Keystrokes and pastes. Async so it runs on the Tokio pool, not on the UI thread.
#[tauri::command]
pub async fn terminal_write(state: State<'_, AppState>, id: u32, data: String) -> AppResult<()> {
    state.terminals.write(id, data.into_bytes())
}

#[tauri::command]
pub async fn terminal_resize(
    state: State<'_, AppState>,
    id: u32,
    cols: u32,
    rows: u32,
) -> AppResult<()> {
    state.terminals.resize(id, cols, rows)
}

#[tauri::command]
pub async fn terminal_close(state: State<'_, AppState>, id: u32) -> AppResult<()> {
    state.terminals.close(id)
}
