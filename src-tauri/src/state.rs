use std::path::PathBuf;

use crate::error::AppResult;
use crate::hosts::HostStore;

/// Everything the commands share, managed by Tauri.
pub struct AppState {
    pub hosts: HostStore,
}

impl AppState {
    pub fn new(data_dir: PathBuf) -> AppResult<Self> {
        std::fs::create_dir_all(&data_dir)?;
        let hosts = HostStore::load(data_dir.join("hosts.json"))?;
        Ok(Self { hosts })
    }
}
