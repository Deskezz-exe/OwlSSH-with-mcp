//! User preferences, stored as `settings.json` in the app data folder.
//! Every field has a default so older files keep loading after updates.

use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::error::AppResult;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// `en` or `ru`.
    pub locale: String,
    /// `dark`, `light` or `system`.
    pub theme: String,
    pub terminal: TerminalSettings,
    pub keybindings: Keybindings,
    /// Replaces real addresses on screen with placeholders.
    pub streamer_mode: bool,
    /// The window close button hides the app to the tray instead of quitting.
    pub close_to_tray: bool,
    /// The one-time note about the tray was shown.
    pub tray_hint_shown: bool,
    pub mcp: McpSettings,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TerminalSettings {
    pub font_size: u16,
    /// A font installed on the system; empty means the bundled JetBrains Mono.
    pub font_family: String,
    pub scrollback: u32,
    pub cursor_blink: bool,
    pub copy_on_select: bool,
    /// Ask before closing a tab whose shell is still running.
    pub confirm_close: bool,
    /// Reopen the shell by itself when the connection drops.
    pub auto_reconnect: bool,
    /// `paste` pastes on right click, `menu` opens a context menu.
    pub right_click: String,
}

/// Terminal shortcuts, written like `Ctrl+Shift+C`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Keybindings {
    pub copy: String,
    pub paste: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct McpSettings {
    pub enabled: bool,
    pub port: u16,
    /// Commands that look destructive wait for confirmation in the app.
    pub confirm_dangerous: bool,
    /// Adds tools that delete, rename and edit files. Off until the user turns it on.
    pub full_access: bool,
    /// Bearer token MCP clients must send. Generated on first start.
    pub token: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            locale: "en".into(),
            theme: "dark".into(),
            terminal: TerminalSettings::default(),
            keybindings: Keybindings::default(),
            streamer_mode: false,
            close_to_tray: true,
            tray_hint_shown: false,
            mcp: McpSettings::default(),
        }
    }
}

impl Default for TerminalSettings {
    fn default() -> Self {
        Self {
            font_size: 14,
            font_family: String::new(),
            scrollback: 10_000,
            cursor_blink: true,
            copy_on_select: false,
            confirm_close: true,
            auto_reconnect: true,
            right_click: "menu".into(),
        }
    }
}

impl Default for Keybindings {
    fn default() -> Self {
        Self {
            copy: "Ctrl+Shift+C".into(),
            paste: "Ctrl+Shift+V".into(),
        }
    }
}

impl Default for McpSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            port: 47821,
            confirm_dangerous: true,
            full_access: false,
            token: String::new(),
        }
    }
}

pub struct SettingsStore {
    path: PathBuf,
    current: Mutex<Settings>,
}

impl SettingsStore {
    /// Loads the file; a missing or unreadable one falls back to defaults.
    pub fn load(path: PathBuf) -> Self {
        let current = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        Self {
            path,
            current: Mutex::new(current),
        }
    }

    pub fn get(&self) -> Settings {
        self.current.lock().unwrap().clone()
    }

    pub fn save(&self, settings: Settings) -> AppResult<Settings> {
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(&settings)?)?;
        std::fs::rename(&tmp, &self.path)?;
        *self.current.lock().unwrap() = settings.clone();
        Ok(settings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_file_fills_in_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(
            &path,
            r#"{ "locale": "ru", "terminal": { "fontSize": 16 } }"#,
        )
        .unwrap();
        let s = SettingsStore::load(path).get();
        assert_eq!(s.locale, "ru");
        assert_eq!(s.terminal.font_size, 16);
        assert_eq!(s.terminal.scrollback, 10_000);
        assert_eq!(s.keybindings.copy, "Ctrl+Shift+C");
    }

    #[test]
    fn saves_and_reloads() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let store = SettingsStore::load(path.clone());
        let mut s = store.get();
        s.keybindings.paste = "Shift+Insert".into();
        store.save(s.clone()).unwrap();
        assert_eq!(SettingsStore::load(path).get(), s);
    }
}
