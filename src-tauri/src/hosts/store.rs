use std::path::{Path, PathBuf};
use std::sync::Mutex;

use super::{normalize_tags, Host, HostInput};
use crate::error::{AppError, AppResult};
use crate::secrets;

/// All hosts in memory, mirrored to `hosts.json` after every change.
pub struct HostStore {
    path: PathBuf,
    hosts: Mutex<Vec<Host>>,
}

impl HostStore {
    pub fn load(path: PathBuf) -> AppResult<Self> {
        let hosts = match std::fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str(&text)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(e.into()),
        };
        Ok(Self {
            path,
            hosts: Mutex::new(hosts),
        })
    }

    pub fn list(&self) -> Vec<Host> {
        let mut hosts = self.hosts.lock().unwrap().clone();
        hosts.sort_by_key(|h| h.sort_order);
        hosts
    }

    pub fn save(&self, input: HostInput) -> AppResult<Host> {
        input.validate().map_err(AppError::Invalid)?;
        let mut hosts = self.hosts.lock().unwrap();
        if let Some(jump) = &input.jump_host_id {
            if !hosts.iter().any(|h| &h.id == jump) {
                return Err(AppError::Invalid("jump host does not exist".into()));
            }
        }

        let existing = input
            .id
            .as_ref()
            .and_then(|id| hosts.iter().position(|h| &h.id == id));
        let id = match (&input.id, existing) {
            (Some(id), Some(_)) => id.clone(),
            (Some(id), None) => return Err(not_found(id)),
            (None, _) => uuid::Uuid::new_v4().to_string(),
        };

        let mut has_password = existing.is_some_and(|i| hosts[i].has_password);
        match input.password.as_deref() {
            Some("") => {
                if has_password {
                    secrets::delete_password(&id)?;
                }
                has_password = false;
            }
            Some(p) => {
                secrets::set_password(&id, p)?;
                has_password = true;
            }
            None => {}
        }

        let name = match input.name.trim() {
            "" => input.hostname.trim().to_string(),
            n => n.to_string(),
        };
        let sort_order = existing
            .map(|i| hosts[i].sort_order)
            .unwrap_or_else(|| next_order(&hosts));
        let host = Host {
            id,
            name,
            hostname: input.hostname.trim().to_string(),
            port: input.port,
            username: input.username.trim().to_string(),
            auth: input.auth,
            key_path: input
                .key_path
                .map(|p| p.trim().to_string())
                .filter(|p| !p.is_empty()),
            has_password,
            tags: normalize_tags(&input.tags),
            favorite: input.favorite,
            jump_host_id: input.jump_host_id,
            sort_order,
        };
        match existing {
            Some(i) => hosts[i] = host.clone(),
            None => hosts.push(host.clone()),
        }
        write_atomic(&self.path, &hosts)?;
        Ok(host)
    }

    /// Adds already-built hosts (the `~/.ssh/config` import) in one write.
    pub fn insert_many(&self, new_hosts: Vec<Host>) -> AppResult<()> {
        let mut hosts = self.hosts.lock().unwrap();
        let start = next_order(&hosts);
        for (i, mut h) in new_hosts.into_iter().enumerate() {
            h.sort_order = start + i as i64;
            hosts.push(h);
        }
        write_atomic(&self.path, &hosts)
    }

    pub fn delete(&self, id: &str) -> AppResult<()> {
        let mut hosts = self.hosts.lock().unwrap();
        let pos = hosts
            .iter()
            .position(|h| h.id == id)
            .ok_or_else(|| not_found(id))?;
        let removed = hosts.remove(pos);
        for h in hosts.iter_mut() {
            if h.jump_host_id.as_deref() == Some(id) {
                h.jump_host_id = None;
            }
        }
        write_atomic(&self.path, &hosts)?;
        drop(hosts);
        if removed.has_password {
            secrets::delete_password(id)?;
        }
        Ok(())
    }

    pub fn set_favorite(&self, id: &str, favorite: bool) -> AppResult<Host> {
        let mut hosts = self.hosts.lock().unwrap();
        let host = hosts
            .iter_mut()
            .find(|h| h.id == id)
            .ok_or_else(|| not_found(id))?;
        host.favorite = favorite;
        let updated = host.clone();
        write_atomic(&self.path, &hosts)?;
        Ok(updated)
    }

    /// Applies a manual order. Hosts missing from `ids` go after the listed ones,
    /// keeping their relative order.
    pub fn reorder(&self, ids: &[String]) -> AppResult<()> {
        let mut hosts = self.hosts.lock().unwrap();
        let tail = ids.len() as i64;
        for h in hosts.iter_mut() {
            h.sort_order = match ids.iter().position(|id| id == &h.id) {
                Some(pos) => pos as i64,
                None => tail + h.sort_order,
            };
        }
        write_atomic(&self.path, &hosts)
    }
}

fn not_found(id: &str) -> AppError {
    AppError::NotFound(format!("host {id} not found"))
}

fn next_order(hosts: &[Host]) -> i64 {
    hosts.iter().map(|h| h.sort_order + 1).max().unwrap_or(0)
}

/// Writes through a temp file and a rename so a crash never leaves half a file.
fn write_atomic(path: &Path, hosts: &[Host]) -> AppResult<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(hosts)?)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hosts::AuthKind;

    fn input(name: &str) -> HostInput {
        HostInput {
            id: None,
            name: name.into(),
            hostname: format!("{name}.example.com"),
            port: 22,
            username: "root".into(),
            auth: AuthKind::Agent,
            key_path: None,
            password: None,
            tags: vec![],
            favorite: false,
            jump_host_id: None,
        }
    }

    fn temp_store() -> (tempfile::TempDir, HostStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = HostStore::load(dir.path().join("hosts.json")).unwrap();
        (dir, store)
    }

    #[test]
    fn saves_and_reloads() {
        let (dir, store) = temp_store();
        let a = store.save(input("a")).unwrap();
        store.save(input("b")).unwrap();
        let reloaded = HostStore::load(dir.path().join("hosts.json")).unwrap();
        assert_eq!(reloaded.list().len(), 2);
        assert!(reloaded
            .list()
            .iter()
            .any(|h| h.id == a.id && h.name == "a"));
    }

    #[test]
    fn updates_in_place_and_keeps_order() {
        let (_dir, store) = temp_store();
        let a = store.save(input("a")).unwrap();
        store.save(input("b")).unwrap();
        let edited = store
            .save(HostInput {
                id: Some(a.id.clone()),
                port: 2222,
                ..input("a2")
            })
            .unwrap();
        assert_eq!(edited.sort_order, a.sort_order);
        assert_eq!(store.list()[0].port, 2222);
        assert_eq!(store.list()[0].name, "a2");
    }

    #[test]
    fn delete_clears_jump_references() {
        let (_dir, store) = temp_store();
        let bastion = store.save(input("bastion")).unwrap();
        let inner = store
            .save(HostInput {
                jump_host_id: Some(bastion.id.clone()),
                ..input("inner")
            })
            .unwrap();
        store.delete(&bastion.id).unwrap();
        let inner = store.list().into_iter().find(|h| h.id == inner.id).unwrap();
        assert_eq!(inner.jump_host_id, None);
    }

    #[test]
    fn rejects_unknown_jump_host() {
        let (_dir, store) = temp_store();
        let res = store.save(HostInput {
            jump_host_id: Some("missing".into()),
            ..input("a")
        });
        assert!(matches!(res, Err(AppError::Invalid(_))));
    }

    #[test]
    fn reorders() {
        let (_dir, store) = temp_store();
        let a = store.save(input("a")).unwrap();
        store.save(input("b")).unwrap();
        let c = store.save(input("c")).unwrap();
        store.reorder(&[c.id.clone(), a.id.clone()]).unwrap();
        let names: Vec<_> = store.list().into_iter().map(|h| h.name).collect();
        assert_eq!(names, ["c", "a", "b"]);
    }

    #[test]
    fn empty_name_falls_back_to_hostname() {
        let (_dir, store) = temp_store();
        let h = store
            .save(HostInput {
                name: " ".into(),
                ..input("x")
            })
            .unwrap();
        assert_eq!(h.name, "x.example.com");
    }
}
