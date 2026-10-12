//! One-click key login: pick or create a local ed25519 key, add its public
//! half to `~/.ssh/authorized_keys` on the server, then prove that the key
//! works before the host is switched over to it.

use std::path::{Path, PathBuf};
use std::time::Duration;

use russh::keys::ssh_key::LineEnding;
use russh::keys::{Algorithm, PrivateKey};

use super::exec::exec;
use super::pool::SshConn;
use crate::error::{AppError, AppResult};

/// The key the host will use.
#[derive(Debug, Clone)]
pub struct LocalKey {
    pub path: PathBuf,
    /// `ssh-ed25519 AAAA... comment`
    pub public_line: String,
    /// True when OwlSSH created the key just now.
    pub created: bool,
}

/// Reuses `~/.ssh/id_ed25519` when it has a `.pub` next to it, otherwise
/// creates a key. An existing private key is never overwritten.
pub fn ensure_key(ssh_dir: &Path) -> AppResult<LocalKey> {
    for name in ["id_ed25519", "owlssh_ed25519"] {
        let path = ssh_dir.join(name);
        let public = path.with_extension("pub");
        if path.exists() && public.exists() {
            let public_line = std::fs::read_to_string(&public)?.trim().to_string();
            if public_line.starts_with("ssh-") {
                return Ok(LocalKey {
                    path,
                    public_line,
                    created: false,
                });
            }
        }
    }
    let name = if ssh_dir.join("id_ed25519").exists() {
        "owlssh_ed25519"
    } else {
        "id_ed25519"
    };
    if ssh_dir.join(name).exists() {
        return Err(AppError::Invalid(format!(
            "{} exists without a .pub file; add it or remove the key",
            ssh_dir.join(name).display()
        )));
    }
    create_key(&ssh_dir.join(name))
}

fn create_key(path: &Path) -> AppResult<LocalKey> {
    let mut key = PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519)
        .map_err(|e| AppError::Invalid(format!("cannot create a key: {e}")))?;
    key.set_comment(comment());
    let private = key
        .to_openssh(LineEnding::LF)
        .map_err(|e| AppError::Invalid(format!("cannot encode the key: {e}")))?;
    let public_line = key
        .public_key()
        .to_openssh()
        .map_err(|e| AppError::Invalid(format!("cannot encode the key: {e}")))?;

    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    write_private(path, private.as_bytes())?;
    std::fs::write(path.with_extension("pub"), format!("{public_line}\n"))?;
    Ok(LocalKey {
        path: path.to_path_buf(),
        public_line,
        created: true,
    })
}

/// OpenSSH refuses private keys other users can read.
fn write_private(path: &Path, bytes: &[u8]) -> AppResult<()> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    std::io::Write::write_all(&mut file, bytes)?;
    Ok(())
}

/// `user@computer`, limited to characters that are safe inside shell quotes.
fn comment() -> String {
    let user = std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "owlssh".into());
    let machine = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "owlssh".into());
    format!("{user}@{machine}")
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || "@._-".contains(*c))
        .collect()
}

/// The shell command that adds `line` to authorized_keys once, with the
/// permissions sshd expects. A file without a final newline gets one first.
pub fn install_command(line: &str) -> AppResult<String> {
    if line.contains(['\'', '\n', '\r']) {
        return Err(AppError::Invalid(
            "unexpected characters in the public key".into(),
        ));
    }
    Ok(format!(
        "umask 077 && mkdir -p ~/.ssh && touch ~/.ssh/authorized_keys && \
         chmod 700 ~/.ssh && chmod 600 ~/.ssh/authorized_keys && \
         {{ grep -qxF '{line}' ~/.ssh/authorized_keys || {{ \
         [ -s ~/.ssh/authorized_keys ] && [ -n \"$(tail -c1 ~/.ssh/authorized_keys)\" ] && echo >> ~/.ssh/authorized_keys; \
         printf '%s\\n' '{line}' >> ~/.ssh/authorized_keys; }}; }}"
    ))
}

pub async fn install(conn: &SshConn, line: &str) -> AppResult<()> {
    let out = exec(conn, &install_command(line)?, Duration::from_secs(20)).await?;
    if out.exit_code == Some(0) {
        Ok(())
    } else {
        let reason = out.stderr.trim();
        Err(AppError::Ssh(if reason.is_empty() {
            "the server did not accept the key".into()
        } else {
            format!("the server did not accept the key: {reason}")
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_then_reuses_the_default_key() {
        let dir = tempfile::tempdir().unwrap();
        let first = ensure_key(dir.path()).unwrap();
        assert!(first.created);
        assert_eq!(first.path, dir.path().join("id_ed25519"));
        assert!(first.public_line.starts_with("ssh-ed25519 "));
        russh::keys::load_secret_key(&first.path, None).unwrap();

        let again = ensure_key(dir.path()).unwrap();
        assert!(!again.created);
        assert_eq!(again.public_line, first.public_line);
    }

    #[test]
    fn never_overwrites_a_private_key_without_pub() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("id_ed25519"), "secret").unwrap();
        let key = ensure_key(dir.path()).unwrap();
        assert_eq!(key.path, dir.path().join("owlssh_ed25519"));
        assert_eq!(
            std::fs::read_to_string(dir.path().join("id_ed25519")).unwrap(),
            "secret"
        );
    }

    #[test]
    fn install_command_rejects_quotes() {
        assert!(install_command("ssh-ed25519 AAAA x'y").is_err());
        let cmd = install_command("ssh-ed25519 AAAA me@pc").unwrap();
        assert!(cmd.contains("grep -qxF 'ssh-ed25519 AAAA me@pc'"));
    }
}
