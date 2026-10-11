use std::time::Duration;

use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};

use crate::error::AppResult;
use crate::snippets::{Snippet, SnippetInput};
use crate::ssh::exec::exec;
use crate::ssh::ConnectOptions;
use crate::state::AppState;

#[tauri::command]
pub fn list_snippets(state: State<'_, AppState>) -> Vec<Snippet> {
    state.snippets.list()
}

#[tauri::command]
pub fn save_snippet(state: State<'_, AppState>, input: SnippetInput) -> AppResult<Snippet> {
    state.snippets.save(input)
}

#[tauri::command]
pub fn delete_snippet(state: State<'_, AppState>, id: String) -> AppResult<()> {
    state.snippets.delete(&id)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostRun {
    pub host_id: String,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<u32>,
    pub error: Option<String>,
}

/// Runs one command on many hosts at once. Each result is sent on `results`
/// as soon as that host finishes, so a slow server never holds up the rest.
#[tauri::command]
pub fn broadcast_command(
    app: AppHandle,
    host_ids: Vec<String>,
    command: String,
    results: Channel<HostRun>,
) {
    for host_id in host_ids {
        let app = app.clone();
        let command = command.clone();
        let results = results.clone();
        tauri::async_runtime::spawn(async move {
            let state = app.state::<AppState>();
            let outcome = async {
                let conn = state
                    .pool
                    .get_or_connect(&state.hosts, &host_id, &ConnectOptions::default())
                    .await?;
                exec(&conn, &command, Duration::from_secs(300)).await
            }
            .await;
            let run = match outcome {
                Ok(out) => HostRun {
                    host_id,
                    stdout: out.stdout,
                    stderr: out.stderr,
                    exit_code: out.exit_code,
                    error: None,
                },
                Err(e) => HostRun {
                    host_id,
                    stdout: String::new(),
                    stderr: String::new(),
                    exit_code: None,
                    error: Some(e.to_string()),
                },
            };
            let _ = results.send(run);
        });
    }
}
