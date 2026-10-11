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
            server: server.map(str::to_string),
            detail: detail.into(),
            outcome: "ok".into(),
            exit_code: None,
        }
    }
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

    /// The newest entries first.
    pub fn recent(&self, limit: usize) -> Vec<AuditEntry> {
        let _guard = self.lock.lock().unwrap();
        let mut entries = self.read_all();
        entries.reverse();
        entries.truncate(limit);
        entries
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
        let recent = log.recent(10);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].tool, "list_servers");
        assert_eq!(recent[1].detail, "uptime");
    }
}
