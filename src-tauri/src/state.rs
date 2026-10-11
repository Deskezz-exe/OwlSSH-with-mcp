use std::path::PathBuf;
use std::sync::Arc;

use crate::error::AppResult;
use crate::hosts::HostStore;
use crate::mcp::audit::AuditLog;
use crate::mcp::guard::Confirmations;
use crate::mcp::McpServer;
use crate::settings::SettingsStore;
use crate::snippets::SnippetStore;
use crate::ssh::sftp::SftpManager;
use crate::ssh::terminal::TerminalManager;
use crate::ssh::SshPool;

/// Everything the commands share, managed by Tauri.
pub struct AppState {
    pub hosts: HostStore,
    pub settings: SettingsStore,
    pub pool: SshPool,
    pub terminals: TerminalManager,
    pub sftp: Arc<SftpManager>,
    pub mcp: McpServer,
    pub audit: AuditLog,
    pub confirmations: Confirmations,
    pub snippets: SnippetStore,
}

impl AppState {
    pub fn new(data_dir: PathBuf) -> AppResult<Self> {
        std::fs::create_dir_all(&data_dir)?;
        let hosts = HostStore::load(data_dir.join("hosts.json"))?;
        let pool = SshPool::new(data_dir.join("known_hosts"));
        let settings = SettingsStore::load(data_dir.join("settings.json"));
        let mut current = settings.get();
        if current.mcp.token.is_empty() {
            current.mcp.token = crate::mcp::new_token();
            settings.save(current)?;
        }
        Ok(Self {
            hosts,
            settings,
            pool,
            terminals: TerminalManager::default(),
            sftp: Arc::default(),
            mcp: McpServer::default(),
            audit: AuditLog::new(data_dir.join("mcp-audit.jsonl")),
            confirmations: Confirmations::default(),
            snippets: SnippetStore::load(data_dir.join("snippets.json")),
        })
    }
}
