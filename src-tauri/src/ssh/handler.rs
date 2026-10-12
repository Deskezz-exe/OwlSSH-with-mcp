//! Host key verification.
//!
//! A key is accepted when it is listed in the user's `~/.ssh/known_hosts`
//! (read only, so OwlSSH and OpenSSH agree) or in OwlSSH's own known_hosts
//! file, or when the user just confirmed this exact fingerprint. Anything else
//! aborts the handshake and is reported back so the UI can ask the user.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use super::forward::{serve_forwarded, RemoteForwards};

use russh::client::{self, Msg, Session};
use russh::keys::{self, HashAlg, PublicKey, PublicKeyOrCertificate};
use russh::Channel;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostKeyIssue {
    Unknown {
        fingerprint: String,
        algorithm: String,
    },
    Changed {
        fingerprint: String,
        algorithm: String,
    },
}

pub struct ClientHandler {
    pub hostname: String,
    pub port: u16,
    pub app_known_hosts: PathBuf,
    /// A fingerprint the user accepted for this connection attempt.
    pub trusted_fingerprint: Option<String>,
    /// Filled when the key was rejected, read by the caller after the failure.
    pub issue: Arc<Mutex<Option<HostKeyIssue>>>,
    pub host_id: String,
    /// Where remote forwards of this host lead on this computer.
    pub forwards: RemoteForwards,
}

impl client::Handler for ClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_key: &PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        let key = match server_key {
            PublicKeyOrCertificate::PublicKey { key, .. } => key.clone(),
            PublicKeyOrCertificate::Certificate(cert) => PublicKey::from(cert.public_key().clone()),
        };
        Ok(self.verify(&key))
    }

    /// A connection arrived on a remote forward. Unknown ports are refused.
    async fn server_channel_open_forwarded_tcpip(
        &mut self,
        channel: Channel<Msg>,
        _connected_address: &str,
        connected_port: u32,
        _originator_address: &str,
        _originator_port: u32,
        reply: client::ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        if let Some(target) = self.forwards.target(&self.host_id, connected_port) {
            reply.accept().await;
            serve_forwarded(channel, target);
        }
        Ok(())
    }
}

impl ClientHandler {
    fn verify(&self, key: &PublicKey) -> bool {
        let fingerprint = key.fingerprint(HashAlg::Sha256).to_string();
        let algorithm = key.algorithm().to_string();

        if self.trusted_fingerprint.as_deref() == Some(fingerprint.as_str()) {
            forget_host(&self.app_known_hosts, &self.hostname, self.port);
            if let Err(e) = keys::known_hosts::learn_known_hosts_path(
                &self.hostname,
                self.port,
                key,
                &self.app_known_hosts,
            ) {
                tracing::warn!("could not save host key: {e}");
            }
            return true;
        }

        let user_file = dirs::home_dir().map(|h| h.join(".ssh").join("known_hosts"));
        let files = [Some(self.app_known_hosts.clone()), user_file];
        for path in files.iter().flatten().filter(|p| p.exists()) {
            match keys::check_known_hosts_path(&self.hostname, self.port, key, path) {
                Ok(true) => return true,
                Ok(false) => {}
                Err(keys::Error::KeyChanged { .. }) => {
                    self.report(HostKeyIssue::Changed {
                        fingerprint,
                        algorithm,
                    });
                    return false;
                }
                Err(e) => tracing::warn!("cannot read {}: {e}", path.display()),
            }
        }

        self.report(HostKeyIssue::Unknown {
            fingerprint,
            algorithm,
        });
        false
    }

    fn report(&self, issue: HostKeyIssue) {
        *self.issue.lock().unwrap() = Some(issue);
    }
}

/// Drops the entries for one host from OwlSSH's own known_hosts file, used
/// before saving a key the user accepted to replace a changed one.
fn forget_host(path: &Path, hostname: &str, port: u16) {
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    let entry = if port == 22 {
        hostname.to_string()
    } else {
        format!("[{hostname}]:{port}")
    };
    let kept: Vec<&str> = text
        .lines()
        .filter(|line| {
            let hosts = line.split_whitespace().next().unwrap_or("");
            !hosts.split(',').any(|h| h == entry)
        })
        .collect();
    let mut out = kept.join("\n");
    if !out.is_empty() {
        out.push('\n');
    }
    if let Err(e) = std::fs::write(path, out) {
        tracing::warn!("cannot update {}: {e}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forget_host_removes_only_matching_entries() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("known_hosts");
        std::fs::write(
            &path,
            "a.example ssh-ed25519 AAAA1\n[a.example]:2222 ssh-ed25519 AAAA2\nb.example,a.example ssh-rsa AAAA3\n",
        )
        .unwrap();
        forget_host(&path, "a.example", 22);
        let left = std::fs::read_to_string(&path).unwrap();
        assert_eq!(left, "[a.example]:2222 ssh-ed25519 AAAA2\n");
    }
}
