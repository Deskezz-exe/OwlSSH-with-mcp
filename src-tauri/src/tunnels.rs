//! Saved port forwards and the ones running right now.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TunnelKind {
    /// A port here leads to an address the server can reach.
    Local,
    /// A port on the server leads to an address this computer can reach.
    Remote,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tunnel {
    pub id: String,
    pub host_id: String,
    #[serde(default)]
    pub name: String,
    pub kind: TunnelKind,
    /// Where to listen: here for local, on the server for remote.
    pub bind_host: String,
    pub bind_port: u16,
    /// Where connections go: as seen from the server for local, from here for remote.
    pub target_host: String,
    pub target_port: u16,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelInput {
    pub id: Option<String>,
    pub host_id: String,
    #[serde(default)]
    pub name: String,
    pub kind: TunnelKind,
    pub bind_host: String,
    pub bind_port: u16,
    pub target_host: String,
    pub target_port: u16,
}

impl TunnelInput {
    fn validate(&self) -> AppResult<()> {
        if self.bind_host.trim().is_empty() || self.target_host.trim().is_empty() {
            return Err(AppError::Invalid("addresses are required".into()));
        }
        if self.bind_port == 0 || self.target_port == 0 {
            return Err(AppError::Invalid(
                "ports must be between 1 and 65535".into(),
            ));
        }
        Ok(())
    }
}

pub struct TunnelStore {
    path: PathBuf,
    items: Mutex<Vec<Tunnel>>,
}

impl TunnelStore {
    pub fn load(path: PathBuf) -> Self {
        let items = std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        Self {
            path,
            items: Mutex::new(items),
        }
    }

    pub fn list(&self) -> Vec<Tunnel> {
        self.items.lock().unwrap().clone()
    }

    pub fn get(&self, id: &str) -> AppResult<Tunnel> {
        self.items
            .lock()
            .unwrap()
            .iter()
            .find(|t| t.id == id)
            .cloned()
            .ok_or_else(|| AppError::NotFound(format!("tunnel {id}")))
    }

    pub fn save(&self, input: TunnelInput) -> AppResult<Tunnel> {
        input.validate()?;
        let tunnel = Tunnel {
            id: input
                .id
                .clone()
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            host_id: input.host_id,
            name: input.name.trim().to_string(),
            kind: input.kind,
            bind_host: input.bind_host.trim().to_string(),
            bind_port: input.bind_port,
            target_host: input.target_host.trim().to_string(),
            target_port: input.target_port,
        };
        let mut items = self.items.lock().unwrap();
        match items.iter().position(|t| t.id == tunnel.id) {
            Some(i) => items[i] = tunnel.clone(),
            None => items.push(tunnel.clone()),
        }
        self.write(&items)?;
        Ok(tunnel)
    }

    pub fn delete(&self, id: &str) -> AppResult<()> {
        let mut items = self.items.lock().unwrap();
        items.retain(|t| t.id != id);
        self.write(&items)
    }

    /// Forgets the tunnels of a deleted host.
    pub fn delete_for_host(&self, host_id: &str) -> AppResult<()> {
        let mut items = self.items.lock().unwrap();
        items.retain(|t| t.host_id != host_id);
        self.write(&items)
    }

    fn write(&self, items: &[Tunnel]) -> AppResult<()> {
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(items)?)?;
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}

/// What the UI shows next to a tunnel.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelStatus {
    pub id: String,
    pub running: bool,
    pub port: Option<u16>,
    pub error: Option<String>,
}

struct Entry {
    host_id: String,
    cancel: Option<CancellationToken>,
    port: Option<u16>,
    error: Option<String>,
}

/// Tunnels that run or ended with an error.
#[derive(Clone, Default)]
pub struct TunnelRuntime(Arc<Mutex<HashMap<String, Entry>>>);

impl TunnelRuntime {
    /// Marks a tunnel as starting; false when it already runs.
    pub fn begin(&self, id: &str, host_id: &str, cancel: CancellationToken) -> bool {
        let mut map = self.0.lock().unwrap();
        if map.get(id).is_some_and(|e| e.cancel.is_some()) {
            return false;
        }
        map.insert(
            id.to_string(),
            Entry {
                host_id: host_id.to_string(),
                cancel: Some(cancel),
                port: None,
                error: None,
            },
        );
        true
    }

    pub fn started(&self, id: &str, port: u16) {
        if let Some(entry) = self.0.lock().unwrap().get_mut(id) {
            entry.port = Some(port);
        }
    }

    /// The tunnel ended: stopped (`None`) or failed with a reason.
    pub fn finish(&self, id: &str, error: Option<String>) {
        let mut map = self.0.lock().unwrap();
        match error {
            Some(error) => {
                if let Some(entry) = map.get_mut(id) {
                    entry.cancel = None;
                    entry.port = None;
                    entry.error = Some(error);
                }
            }
            None => {
                map.remove(id);
            }
        }
    }

    pub fn stop(&self, id: &str) {
        let cancel = self
            .0
            .lock()
            .unwrap()
            .get_mut(id)
            .and_then(|e| e.cancel.take());
        match cancel {
            Some(cancel) => cancel.cancel(),
            // Clears an old error.
            None => self.finish(id, None),
        }
    }

    pub fn stop_host(&self, host_id: &str) {
        let ids: Vec<String> = self
            .0
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, e)| e.host_id == host_id)
            .map(|(id, _)| id.clone())
            .collect();
        for id in ids {
            self.stop(&id);
        }
    }

    pub fn statuses(&self) -> Vec<TunnelStatus> {
        self.0
            .lock()
            .unwrap()
            .iter()
            .map(|(id, e)| TunnelStatus {
                id: id.clone(),
                running: e.cancel.is_some(),
                port: e.port,
                error: e.error.clone(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(port: u16) -> TunnelInput {
        TunnelInput {
            id: None,
            host_id: "h".into(),
            name: " db ".into(),
            kind: TunnelKind::Local,
            bind_host: "127.0.0.1".into(),
            bind_port: port,
            target_host: "localhost".into(),
            target_port: 5432,
        }
    }

    #[test]
    fn saves_edits_and_reloads() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tunnels.json");
        let store = TunnelStore::load(path.clone());
        let saved = store.save(input(15432)).unwrap();
        assert_eq!(saved.name, "db");
        let mut edit = input(15433);
        edit.id = Some(saved.id.clone());
        store.save(edit).unwrap();
        let reloaded = TunnelStore::load(path);
        assert_eq!(reloaded.list().len(), 1);
        assert_eq!(reloaded.get(&saved.id).unwrap().bind_port, 15433);
        assert!(store.save(input(0)).is_err());
    }

    #[test]
    fn runtime_tracks_errors_and_stops() {
        let rt = TunnelRuntime::default();
        let cancel = CancellationToken::new();
        assert!(rt.begin("t", "h", cancel.clone()));
        assert!(
            !rt.begin("t", "h", CancellationToken::new()),
            "already running"
        );
        rt.started("t", 8080);
        assert_eq!(rt.statuses()[0].port, Some(8080));

        rt.stop_host("h");
        assert!(cancel.is_cancelled());
        rt.finish("t", Some("the SSH connection closed".into()));
        let status = &rt.statuses()[0];
        assert!(!status.running);
        assert!(status.error.is_some());

        rt.stop("t");
        assert!(rt.statuses().is_empty(), "stopping clears the error");
    }
}
