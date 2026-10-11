use tauri::State;

use crate::error::AppResult;
use crate::settings::Settings;
use crate::state::AppState;

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Settings {
    state.settings.get()
}

#[tauri::command]
pub fn save_settings(state: State<'_, AppState>, settings: Settings) -> AppResult<Settings> {
    state.settings.save(settings)
}
