use tauri::{AppHandle, State};

use crate::error::AppResult;
use crate::settings::Settings;
use crate::state::AppState;

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Settings {
    state.settings.get()
}

/// Saves settings; a changed MCP section restarts the MCP server.
#[tauri::command]
pub fn save_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    mut settings: Settings,
) -> AppResult<Settings> {
    let previous = state.settings.get();
    if settings.mcp.token.is_empty() {
        settings.mcp.token = previous.mcp.token.clone();
    }
    let saved = state.settings.save(settings)?;
    if saved.mcp != previous.mcp {
        state.mcp.restart(&app, &saved.mcp);
    }
    Ok(saved)
}
