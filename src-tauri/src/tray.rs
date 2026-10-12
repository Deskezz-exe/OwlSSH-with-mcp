//! Tray icon. Closing the window can hide it there, so terminals and the MCP
//! server keep running; "Quit" in the tray menu ends the process for real.

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Runtime, WindowEvent};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

use crate::state::AppState;

const SHOW: &str = "show";
const QUIT: &str = "quit";

/// Menu entries kept so their text can follow the language setting.
pub struct TrayMenu<R: Runtime> {
    show: MenuItem<R>,
    quit: MenuItem<R>,
}

fn labels(locale: &str) -> (&'static str, &'static str) {
    match locale {
        "ru" => ("Открыть OwlSSH", "Выйти"),
        _ => ("Open OwlSSH", "Quit"),
    }
}

pub fn create<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let locale = app.state::<AppState>().settings.get().locale;
    let (show_text, quit_text) = labels(&locale);
    let show = MenuItem::with_id(app, SHOW, show_text, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, QUIT, quit_text, true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &PredefinedMenuItem::separator(app)?, &quit])?;

    let mut tray = TrayIconBuilder::with_id("main")
        .tooltip("OwlSSH")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            SHOW => show_window(app),
            QUIT => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_window(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    app.manage(TrayMenu { show, quit });
    Ok(())
}

/// Updates the menu after the language changed.
pub fn set_locale<R: Runtime>(app: &AppHandle<R>, locale: &str) {
    if let Some(menu) = app.try_state::<TrayMenu<R>>() {
        let (show_text, quit_text) = labels(locale);
        let _ = menu.show.set_text(show_text);
        let _ = menu.quit.set_text(quit_text);
    }
}

pub fn show_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// The window close button: hide to the tray when the setting is on.
pub fn on_window_event<R: Runtime>(window: &tauri::Window<R>, event: &WindowEvent) {
    let WindowEvent::CloseRequested { api, .. } = event else {
        return;
    };
    let app = window.app_handle();
    let state = app.state::<AppState>();
    let mut settings = state.settings.get();
    if !settings.close_to_tray {
        // Closing the only window ends the app, sessions and the MCP server with it.
        app.exit(0);
        return;
    }
    api.prevent_close();
    let _ = window.hide();

    // Say once where the app went, so it does not look like it is stuck in memory.
    if !settings.tray_hint_shown {
        settings.tray_hint_shown = true;
        let _ = state.settings.save(settings.clone());
        let (title, text) = match settings.locale.as_str() {
            "ru" => (
                "OwlSSH работает в трее",
                "Окно скрыто, а сессии и MCP-сервер продолжают работать. Чтобы закрыть программу полностью, выберите «Выйти» в меню значка в трее. Это поведение можно выключить в настройках.",
            ),
            _ => (
                "OwlSSH is still running",
                "The window is hidden, while sessions and the MCP server keep running. To close the app completely, choose Quit in the tray icon menu. You can turn this off in Settings.",
            ),
        };
        app.dialog()
            .message(text)
            .title(title)
            .kind(MessageDialogKind::Info)
            .show(|_| {});
    }
}
