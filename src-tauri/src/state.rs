use std::path::PathBuf;

use crate::error::AppResult;
use crate::hosts::HostStore;
use crate::settings::SettingsStore;
use crate::ssh::terminal::TerminalManager;
use crate::ssh::SshPool;

/// Everything the commands share, managed by Tauri.
pub struct AppState {
    pub hosts: HostStore,
    pub settings: SettingsStore,
    pub pool: SshPool,
    pub terminals: TerminalManager,
}

impl AppState {
    pub fn new(data_dir: PathBuf) -> AppResult<Self> {
        std::fs::create_dir_all(&data_dir)?;
        let hosts = HostStore::load(data_dir.join("hosts.json"))?;
        let pool = SshPool::new(data_dir.join("known_hosts"));
        let settings = SettingsStore::load(data_dir.join("settings.json"));
        Ok(Self {
            hosts,
            settings,
            pool,
            terminals: TerminalManager::default(),
        })
    }
}
