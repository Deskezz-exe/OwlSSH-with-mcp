//! Every MCP action is appended to `mcp-audit.jsonl` and announced to the UI.

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditEntry {
    /// Unix milliseconds.
    pub time: u64,
    pub tool: String,
    /// The assistant that made the call, e.g. "Claude Code" or "Codex".
    #[serde(default)]
    pub client: Option<String>,
    pub server: Option<String>,
    pub detail: String,
    /// `ok`, `error` or `denied`.
    pub outcome: String,
    pub exit_code: Option<u32>,
}

impl AuditEntry {
    pub fn new(tool: &str, server: Option<&str>, detail: impl Into<String>) -> Self {
        Self {
            time: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
            tool: tool.to_string(),
            client: None,
            server: server.map(str::to_string),
            detail: detail.into(),
            outcome: "ok".into(),
            exit_code: None,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditQuery {
    #[serde(default)]
    pub offset: usize,
    #[serde(default = "default_limit")]
    pub limit: usize,
    pub outcome: Option<String>,
    pub client: Option<String>,
}

fn default_limit() -> usize {
    15
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditPage {
    pub entries: Vec<AuditEntry>,
    /// How many entries match the filters, for the page count.
    pub total: usize,
    /// Every client seen in the log, for the filter menu.
    pub clients: Vec<String>,
}

pub struct AuditLog {
    path: PathBuf,
    lock: Mutex<()>,
}

/// The log is trimmed back to this many entries when it grows past twice that.
const KEEP: usize = 2000;

impl AuditLog {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            lock: Mutex::new(()),
        }
    }

    pub fn append(&self, entry: &AuditEntry) {
        let _guard = self.lock.lock().unwrap();
        let line = match serde_json::to_string(entry) {
            Ok(line) => line,
            Err(_) => return,
        };
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path);
        if let Ok(mut file) = file {
            let _ = writeln!(file, "{line}");
        }
        self.trim();
    }

    /// One page of entries, newest first, optionally filtered by outcome and client.
    pub fn page(&self, query: &AuditQuery) -> AuditPage {
        let _guard = self.lock.lock().unwrap();
        let mut all = self.read_all();
        all.reverse();
        let mut clients: Vec<String> = all.iter().filter_map(|e| e.client.clone()).collect();
        clients.sort();
        clients.dedup();
        let matching: Vec<AuditEntry> = all
            .into_iter()
            .filter(|e| query.outcome.as_ref().is_none_or(|o| &e.outcome == o))
            .filter(|e| {
                query
                    .client
                    .as_ref()
                    .is_none_or(|c| e.client.as_ref() == Some(c))
            })
            .collect();
        let total = matching.len();
        let entries = matching
            .into_iter()
            .skip(query.offset)
            .take(query.limit.clamp(1, 100))
            .collect();
        AuditPage {
            entries,
            total,
            clients,
        }
    }

    fn read_all(&self) -> Vec<AuditEntry> {
        let Ok(file) = std::fs::File::open(&self.path) else {
            return Vec::new();
        };
        BufReader::new(file)
            .lines()
            .map_while(Result::ok)
            .filter_map(|line| serde_json::from_str(&line).ok())
            .collect()
    }

    fn trim(&self) {
        let entries = self.read_all();
        if entries.len() <= KEEP * 2 {
            return;
        }
        let kept = &entries[entries.len() - KEEP..];
        let text: String = kept
            .iter()
            .filter_map(|e| serde_json::to_string(e).ok())
            .map(|l| l + "\n")
            .collect();
        let _ = std::fs::write(&self.path, text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_and_reads_newest_first() {
        let dir = tempfile::tempdir().unwrap();
        let log = AuditLog::new(dir.path().join("audit.jsonl"));
        log.append(&AuditEntry::new("run_command", Some("web"), "uptime"));
        log.append(&AuditEntry::new("list_servers", None, ""));
        let page = log.page(&AuditQuery {
            limit: 10,
            ..Default::default()
        });
        assert_eq!(page.total, 2);
        assert_eq!(page.entries[0].tool, "list_servers");
        assert_eq!(page.entries[1].detail, "uptime");
    }

    #[test]
    fn pages_and_filters() {
        let dir = tempfile::tempdir().unwrap();
        let log = AuditLog::new(dir.path().join("audit.jsonl"));
        for i in 0..25 {
            let mut e = AuditEntry::new("run_command", Some("web"), format!("cmd {i}"));
            e.client = Some(if i % 2 == 0 { "Claude Code" } else { "Codex" }.into());
            if i % 5 == 0 {
                e.outcome = "denied".into();
            }
            log.append(&e);
        }
        let second = log.page(&AuditQuery {
            offset: 10,
            limit: 10,
            ..Default::default()
        });
        assert_eq!(second.total, 25);
        assert_eq!(second.entries.len(), 10);
        assert_eq!(second.entries[0].detail, "cmd 14");
        assert_eq!(second.clients, ["Claude Code", "Codex"]);

        let denied = log.page(&AuditQuery {
            limit: 10,
            outcome: Some("denied".into()),
            ..Default::default()
        });
        assert_eq!(denied.total, 5);
        let codex = log.page(&AuditQuery {
            limit: 50,
            client: Some("Codex".into()),
            ..Default::default()
        });
        assert_eq!(codex.total, 12);
    }
}
