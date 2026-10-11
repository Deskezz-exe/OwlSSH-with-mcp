use tauri::State;

use crate::mcp::audit::AuditEntry;
use crate::mcp::McpStatus;
use crate::state::AppState;

#[tauri::command]
pub fn mcp_status(state: State<'_, AppState>) -> McpStatus {
    state.mcp.status()
}

#[tauri::command]
pub fn mcp_activity(state: State<'_, AppState>, limit: Option<usize>) -> Vec<AuditEntry> {
    state.audit.recent(limit.unwrap_or(200))
}

/// The user's answer to an approval dialog.
#[tauri::command]
pub fn mcp_confirm_answer(state: State<'_, AppState>, id: u64, approve: bool) {
    state.confirmations.answer(id, approve);
}
