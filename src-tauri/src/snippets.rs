//! Saved commands. `{{name}}` placeholders are filled in by the UI before running.

use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snippet {
    pub id: String,
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnippetInput {
    pub id: Option<String>,
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub description: String,
}

pub struct SnippetStore {
    path: PathBuf,
    items: Mutex<Vec<Snippet>>,
}

impl SnippetStore {
    pub fn load(path: PathBuf) -> Self {
        let items = std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_else(starter_snippets);
        Self {
            path,
            items: Mutex::new(items),
        }
    }

    pub fn list(&self) -> Vec<Snippet> {
        self.items.lock().unwrap().clone()
    }

    pub fn save(&self, input: SnippetInput) -> AppResult<Snippet> {
        if input.name.trim().is_empty() || input.command.trim().is_empty() {
            return Err(AppError::Invalid("name and command are required".into()));
        }
        let mut items = self.items.lock().unwrap();
        let snippet = Snippet {
            id: input
                .id
                .clone()
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            name: input.name.trim().to_string(),
            command: input.command.trim_end().to_string(),
            description: input.description.trim().to_string(),
        };
        match items.iter().position(|s| Some(&s.id) == input.id.as_ref()) {
            Some(i) => items[i] = snippet.clone(),
            None => items.push(snippet.clone()),
        }
        self.write(&items)?;
        Ok(snippet)
    }

    pub fn delete(&self, id: &str) -> AppResult<()> {
        let mut items = self.items.lock().unwrap();
        items.retain(|s| s.id != id);
        self.write(&items)
    }

    fn write(&self, items: &[Snippet]) -> AppResult<()> {
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(items)?)?;
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}

/// A few useful commands so the screen is not empty on first start.
fn starter_snippets() -> Vec<Snippet> {
    let make = |name: &str, command: &str, description: &str| Snippet {
        id: uuid::Uuid::new_v4().to_string(),
        name: name.into(),
        command: command.into(),
        description: description.into(),
    };
    vec![
        make("Disk usage", "df -h", "Free space on every mounted filesystem"),
        make(
            "Restart a service",
            "sudo systemctl restart {{service}} && systemctl status {{service}} --no-pager | head -n 5",
            "Restarts a systemd unit and shows its state",
        ),
        make(
            "Follow a log",
            "sudo journalctl -u {{service}} -n {{lines}} --no-pager",
            "Last lines of a service log",
        ),
        make(
            "Docker containers",
            "docker ps --format 'table {{.Names}}\\t{{.Status}}\\t{{.Ports}}'",
            "Running containers with status and ports",
        ),
        make(
            "Pending updates",
            "apt list --upgradable 2>/dev/null | tail -n +2 | wc -l",
            "How many packages can be upgraded (Debian, Ubuntu)",
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_with_examples_and_saves_changes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("snippets.json");
        let store = SnippetStore::load(path.clone());
        let first = store.list().len();
        assert!(first > 0);

        let saved = store
            .save(SnippetInput {
                id: None,
                name: "Uptime".into(),
                command: "uptime".into(),
                description: String::new(),
            })
            .unwrap();
        store.delete(&store.list()[0].id).unwrap();

        let reloaded = SnippetStore::load(path);
        assert_eq!(reloaded.list().len(), first);
        assert!(reloaded.list().contains(&saved));
    }

    #[test]
    fn rejects_empty_command() {
        let dir = tempfile::tempdir().unwrap();
        let store = SnippetStore::load(dir.path().join("s.json"));
        let res = store.save(SnippetInput {
            id: None,
            name: "x".into(),
            command: "  ".into(),
            description: String::new(),
        });
        assert!(res.is_err());
    }
}
