mod commands;
mod error;
mod hosts;
mod mcp;
mod secrets;
mod settings;
mod snippets;
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
            let state = state::AppState::new(data_dir)?;
            let mcp_settings = state.settings.get().mcp;
            app.manage(state);
            app.state::<state::AppState>()
                .mcp
                .restart(app.handle(), &mcp_settings);
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
            commands::sftp::sftp_list,
            commands::sftp::sftp_mkdir,
            commands::sftp::sftp_rename,
            commands::sftp::sftp_remove,
            commands::sftp::sftp_upload,
            commands::sftp::sftp_download,
            commands::sftp::sftp_cancel,
            commands::sftp::sftp_exists,
            commands::sftp::local_exists,
            commands::sftp::local_list,
            commands::sftp::local_roots,
            commands::sftp::local_mkdir,
            commands::sftp::local_remove,
            commands::mcp::mcp_status,
            commands::mcp::mcp_activity,
            commands::mcp::mcp_confirm_answer,
            commands::snippets::list_snippets,
            commands::snippets::save_snippet,
            commands::snippets::delete_snippet,
            commands::snippets::broadcast_command,
            commands::settings::get_settings,
            commands::settings::save_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running OwlSSH");
}
