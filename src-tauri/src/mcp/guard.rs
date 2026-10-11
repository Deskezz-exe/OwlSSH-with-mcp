//! Human in the loop for AI actions.
//!
//! Commands that delete data, stop services or reboot, and any file write, are
//! held until the user answers a dialog in the app. The assistant cannot
//! approve its own request: the answer only comes from the UI.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use regex::RegexSet;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::oneshot;

/// How long a request waits for the user before it counts as denied.
const ANSWER_TIMEOUT: Duration = Duration::from_secs(180);

static DANGEROUS: LazyLock<RegexSet> = LazyLock::new(|| {
    RegexSet::new([
        // Deleting files
        r"(^|[\s;&|(])(rm|rmdir|unlink|shred|srm)\s",
        r"\bfind\b.*\s-delete\b",
        r"\bmv\b.*\s/dev/null\b",
        // Disks and filesystems
        r"\b(mkfs(\.\w+)?|fdisk|sfdisk|parted|wipefs|mkswap)\b",
        r"\bdd\b.*\bof=",
        r">\s*/dev/(sd|nvme|vd|hd|xvd)",
        // Power and services
        r"\b(shutdown|reboot|poweroff|halt)\b",
        r"\binit\s+[06]\b",
        r"\bsystemctl\s+(stop|disable|mask|kill|poweroff|reboot|halt)\b",
        r"\bservice\s+\S+\s+stop\b",
        r"\b(kill\s+-9|killall|pkill)\b",
        // Permissions and accounts
        r"\b(chmod|chown|chgrp)\s+(-\w*R|--recursive)",
        r"\b(userdel|deluser|groupdel|passwd)\b",
        // Firewall
        r"\biptables\s+(-F|--flush)",
        r"\bufw\s+(disable|reset)\b",
        r"\bnft\s+flush\b",
        // Packages
        r"\b(apt|apt-get|aptitude)\s+(remove|purge|autoremove)\b",
        r"\b(yum|dnf|zypper|pacman)\s+(remove|erase|-R)",
        // Containers and data
        r"\bdocker\s+(rm|rmi|kill|system\s+prune|volume\s+(rm|prune)|network\s+rm)\b",
        r"\bdocker[\s-]compose\s+(down|rm)\b",
        r"(?i)\b(drop\s+(database|table|schema)|truncate\s+table)\b",
        // Version control and schedules
        r"\bgit\s+(reset\s+--hard|clean\s+-\w*f|push\s+.*--force)",
        r"\bcrontab\s+-r\b",
        // Fork bomb
        r":\(\)\s*\{",
    ])
    .expect("valid patterns")
});

/// True when the command could destroy data or take the server down.
pub fn is_dangerous(command: &str) -> bool {
    DANGEROUS.is_match(command)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmRequest {
    pub id: u64,
    /// Which assistant is asking, e.g. "Claude Code".
    pub client: String,
    pub server: String,
    /// `command`, `writeFile`, `delete`, `rename` or `edit`.
    pub action: String,
    pub detail: String,
}

#[derive(Default)]
pub struct Confirmations {
    next: AtomicU64,
    pending: Mutex<HashMap<u64, oneshot::Sender<bool>>>,
}

impl Confirmations {
    /// Shows the dialog in the app and waits for the user's decision.
    pub async fn ask(
        &self,
        app: &AppHandle,
        client: &str,
        server: &str,
        action: &str,
        detail: &str,
    ) -> bool {
        let id = self.next.fetch_add(1, Ordering::Relaxed) + 1;
        let (tx, rx) = oneshot::channel();
        self.pending.lock().unwrap().insert(id, tx);

        if let Some(window) = app.get_webview_window("main") {
            let _ = window.unminimize();
            let _ = window.show();
            let _ = window.set_focus();
        }
        let _ = app.emit(
            "mcp-confirm",
            ConfirmRequest {
                id,
                client: client.to_string(),
                server: server.to_string(),
                action: action.to_string(),
                detail: detail.to_string(),
            },
        );

        let approved = matches!(tokio::time::timeout(ANSWER_TIMEOUT, rx).await, Ok(Ok(true)));
        self.pending.lock().unwrap().remove(&id);
        let _ = app.emit("mcp-confirm-closed", id);
        approved
    }

    pub fn answer(&self, id: u64, approve: bool) {
        if let Some(tx) = self.pending.lock().unwrap().remove(&id) {
            let _ = tx.send(approve);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_destructive_commands() {
        for cmd in [
            "rm -rf /var/www",
            "sudo rm file.txt",
            "cd /tmp && rm *.log",
            "find . -name '*.tmp' -delete",
            "mkfs.ext4 /dev/sdb1",
            "dd if=/dev/zero of=/dev/sda bs=1M",
            "sudo reboot",
            "systemctl stop nginx",
            "chmod -R 777 /",
            "docker system prune -a",
            "docker compose down",
            "psql -c 'DROP TABLE users'",
            "git reset --hard HEAD~3",
            "apt-get purge nginx",
            "killall node",
            ":(){ :|:& };:",
        ] {
            assert!(is_dangerous(cmd), "should flag: {cmd}");
        }
    }

    #[test]
    fn lets_everyday_commands_through() {
        for cmd in [
            "ls -la /var/log",
            "df -h",
            "systemctl status nginx",
            "docker ps",
            "tail -n 100 /var/log/syslog",
            "grep -r error /var/log/nginx",
            "cat /etc/os-release",
            "git status",
            "free -m && uptime",
            "echo firmware",
        ] {
            assert!(!is_dangerous(cmd), "should allow: {cmd}");
        }
    }
}
