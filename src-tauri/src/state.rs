use std::path::PathBuf;

use crate::error::AppResult;
use crate::hosts::HostStore;
use crate::ssh::SshPool;

/// Everything the commands share, managed by Tauri.
pub struct AppState {
    pub hosts: HostStore,
    pub pool: SshPool,
}

impl AppState {
    pub fn new(data_dir: PathBuf) -> AppResult<Self> {
        std::fs::create_dir_all(&data_dir)?;
        let hosts = HostStore::load(data_dir.join("hosts.json"))?;
        let pool = SshPool::new(data_dir.join("known_hosts"));
        Ok(Self { hosts, pool })
    }
}
