mod commands;
mod error;
mod hosts;
mod secrets;
mod settings;
mod ssh;
mod state;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            app.manage(state::AppState::new(data_dir)?);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::hosts::list_hosts,
            commands::hosts::save_host,
            commands::hosts::delete_host,
            commands::hosts::set_favorite,
            commands::hosts::reorder_hosts,
            commands::hosts::preview_ssh_config,
            commands::hosts::import_ssh_config,
            commands::ssh::connect_host,
            commands::ssh::disconnect_host,
            commands::ssh::connected_hosts,
            commands::ssh::run_command,
            commands::ssh::host_metrics,
            commands::terminal::terminal_open,
            commands::terminal::terminal_write,
            commands::terminal::terminal_resize,
            commands::terminal::terminal_close,
            commands::settings::get_settings,
            commands::settings::save_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running OwlSSH");
}
