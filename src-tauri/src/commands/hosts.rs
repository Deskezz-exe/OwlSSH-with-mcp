use tauri::State;

use crate::error::AppResult;
use crate::hosts::ssh_config::{self, SshConfigEntry};
use crate::hosts::{AuthKind, Host, HostInput};
use crate::state::AppState;

#[tauri::command]
pub fn list_hosts(state: State<'_, AppState>) -> Vec<Host> {
    state.hosts.list()
}

#[tauri::command]
pub fn save_host(state: State<'_, AppState>, input: HostInput) -> AppResult<Host> {
    state.hosts.save(input)
}

#[tauri::command]
pub fn delete_host(state: State<'_, AppState>, id: String) -> AppResult<()> {
    state.hosts.delete(&id)
}

#[tauri::command]
pub fn set_favorite(state: State<'_, AppState>, id: String, favorite: bool) -> AppResult<Host> {
    state.hosts.set_favorite(&id, favorite)
}

#[tauri::command]
pub fn reorder_hosts(state: State<'_, AppState>, ids: Vec<String>) -> AppResult<()> {
    state.hosts.reorder(&ids)
}

/// A host from `~/.ssh/config` plus whether an identical one is already saved.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportCandidate {
    #[serde(flatten)]
    pub entry: SshConfigEntry,
    pub already_added: bool,
}

#[tauri::command]
pub fn preview_ssh_config(state: State<'_, AppState>) -> Vec<ImportCandidate> {
    let Some(path) = ssh_config::default_path() else {
        return Vec::new();
    };
    let existing = state.hosts.list();
    ssh_config::load(&path)
        .into_iter()
        .map(|entry| {
            let user = entry.user.clone().unwrap_or_else(local_user);
            let already_added = existing.iter().any(|h| {
                h.hostname.eq_ignore_ascii_case(&entry.hostname)
                    && h.port == entry.port
                    && h.username == user
            });
            ImportCandidate {
                entry,
                already_added,
            }
        })
        .collect()
}

/// Imports the chosen aliases. `ProxyJump` aliases are linked to the matching
/// imported or existing host by name.
#[tauri::command]
pub fn import_ssh_config(state: State<'_, AppState>, aliases: Vec<String>) -> AppResult<Vec<Host>> {
    let Some(path) = ssh_config::default_path() else {
        return Ok(Vec::new());
    };
    let entries: Vec<_> = ssh_config::load(&path)
        .into_iter()
        .filter(|e| aliases.contains(&e.alias))
        .collect();
    let existing = state.hosts.list();
    let mut created: Vec<Host> = entries
        .iter()
        .map(|e| Host {
            id: uuid::Uuid::new_v4().to_string(),
            name: e.alias.clone(),
            hostname: e.hostname.clone(),
            port: e.port,
            username: e.user.clone().unwrap_or_else(local_user),
            auth: if e.identity_file.is_some() {
                AuthKind::Key
            } else {
                AuthKind::Agent
            },
            key_path: e.identity_file.clone(),
            has_password: false,
            tags: vec!["ssh-config".into()],
            favorite: false,
            jump_host_id: None,
            sort_order: 0,
        })
        .collect();

    let ids_by_name: Vec<(String, String)> = created
        .iter()
        .chain(existing.iter())
        .map(|h| (h.name.clone(), h.id.clone()))
        .collect();
    for (host, entry) in created.iter_mut().zip(&entries) {
        if let Some(jump) = &entry.proxy_jump {
            host.jump_host_id = ids_by_name
                .iter()
                .find(|(name, id)| name == jump && *id != host.id)
                .map(|(_, id)| id.clone());
        }
    }

    state.hosts.insert_many(created.clone())?;
    Ok(created)
}

fn local_user() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "root".into())
}
