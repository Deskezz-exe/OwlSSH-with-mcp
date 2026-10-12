//! End-to-end tests of the SSH engine against an in-process russh server:
//! host key trust, password auth, exec, jump hosts, interactive shells and
//! one-click key setup.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use russh::keys::{decode_secret_key, PrivateKey, PublicKey};
use russh::server::{self, Auth, Msg, Session};
use russh::{Channel, ChannelId};
use tokio::net::{TcpListener, TcpStream};

use super::exec::exec;
use super::keysetup;
use super::terminal::TerminalManager;
use super::{ConnectOptions, SshPool};
use crate::error::AppError;
use crate::hosts::{AuthKind, Host, HostInput, HostStore};

// Throwaway keys generated for these tests only.
const KEY_A: &str = "-----BEGIN OPENSSH PRIVATE KEY-----
b3BlbnNzaC1rZXktdjEAAAAABG5vbmUAAAAEbm9uZQAAAAAAAAABAAAAMwAAAAtzc2gtZW
QyNTUxOQAAACCxfLcBhDTg5cS6kbj8qVQ8MHP9tuT8oQ6BtVvdjZ9JzgAAAJCdeG6qnXhu
qgAAAAtzc2gtZWQyNTUxOQAAACCxfLcBhDTg5cS6kbj8qVQ8MHP9tuT8oQ6BtVvdjZ9Jzg
AAAECuYJq5KUKDDLfwZnb2ywUoTgZ95bU5WBq+a0kUKU0eJrF8twGENODlxLqRuPypVDww
c/225PyhDoG1W92Nn0nOAAAADW93bHNzaC10ZXN0LWE=
-----END OPENSSH PRIVATE KEY-----";

const KEY_B: &str = "-----BEGIN OPENSSH PRIVATE KEY-----
b3BlbnNzaC1rZXktdjEAAAAABG5vbmUAAAAEbm9uZQAAAAAAAAABAAAAMwAAAAtzc2gtZW
QyNTUxOQAAACB8m0mdQvwfrFX6JAL1impj3RF0Mrm3heXVPpqktNcuOgAAAJCA0sROgNLE
TgAAAAtzc2gtZWQyNTUxOQAAACB8m0mdQvwfrFX6JAL1impj3RF0Mrm3heXVPpqktNcuOg
AAAEBC+6J+Zf4aHlgR6D02qbU9qBj3fBNDpV+vz+UNrAq4SHybSZ1C/B+sVfokAvWKamPd
EXQyubeF5dU+mqS01y46AAAADW93bHNzaC10ZXN0LWI=
-----END OPENSSH PRIVATE KEY-----";

const PASSWORD: &str = "hunter2";

/// Session channels, kept until the client asks for a shell on one, and the
/// public keys added through the key setup command (shared by all sessions).
#[derive(Clone, Default)]
struct TestServer {
    channels: Arc<std::sync::Mutex<HashMap<ChannelId, Channel<Msg>>>>,
    authorized: Arc<std::sync::Mutex<Vec<String>>>,
}

/// The base64 part of an OpenSSH public key line.
fn key_blob(line: &str) -> String {
    line.split_whitespace()
        .nth(1)
        .unwrap_or_default()
        .to_string()
}

impl server::Handler for TestServer {
    type Error = russh::Error;

    async fn auth_password(&mut self, _user: &str, password: &str) -> Result<Auth, Self::Error> {
        Ok(if password == PASSWORD {
            Auth::Accept
        } else {
            Auth::reject()
        })
    }

    async fn auth_publickey(&mut self, _user: &str, key: &PublicKey) -> Result<Auth, Self::Error> {
        let blob = key_blob(&key.to_openssh().unwrap_or_default());
        let known = self
            .authorized
            .lock()
            .unwrap()
            .iter()
            .any(|line| key_blob(line) == blob);
        Ok(if known { Auth::Accept } else { Auth::reject() })
    }

    async fn channel_open_session(
        &mut self,
        channel: Channel<Msg>,
        reply: server::ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.channels.lock().unwrap().insert(channel.id(), channel);
        reply.accept().await;
        Ok(())
    }

    async fn exec_request(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.channel_success(channel)?;
        let command = String::from_utf8_lossy(data);
        // Plays the part of the shell for the key setup command.
        if let Some(rest) = command.split(r"printf '%s\n' '").nth(1) {
            let line = rest.split('\'').next().unwrap_or_default().to_string();
            self.authorized.lock().unwrap().push(line);
        }
        let out = format!("ran: {command}\n");
        session.data(channel, out.into_bytes())?;
        session.exit_status_request(channel, 0)?;
        session.eof(channel)?;
        session.close(channel)?;
        Ok(())
    }

    async fn pty_request(
        &mut self,
        channel: ChannelId,
        _term: &str,
        _cols: u32,
        _rows: u32,
        _pix_width: u32,
        _pix_height: u32,
        _modes: &[(russh::Pty, u32)],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.channel_success(channel)?;
        Ok(())
    }

    async fn shell_request(
        &mut self,
        channel: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.channel_success(channel)?;
        let Some(mut shell) = self.channels.lock().unwrap().remove(&channel) else {
            return Ok(());
        };
        // A tiny shell: prints a prompt, echoes input, `exit` ends with status 7.
        tokio::spawn(async move {
            let _ = shell.data_bytes(b"$ ".to_vec()).await;
            while let Some(msg) = shell.wait().await {
                if let russh::ChannelMsg::Data { data } = msg {
                    if data.as_ref() == b"exit\r" {
                        let _ = shell.exit_status(7).await;
                        let _ = shell.eof().await;
                        let _ = shell.close().await;
                        break;
                    }
                    let _ = shell.data_bytes(data).await;
                }
            }
        });
        Ok(())
    }

    /// Lets the server act as a jump host: pipes the channel to the target.
    async fn channel_open_direct_tcpip(
        &mut self,
        channel: Channel<Msg>,
        host: &str,
        port: u32,
        _originator: &str,
        _originator_port: u32,
        reply: server::ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        let addr = format!("{host}:{port}");
        reply.accept().await;
        tokio::spawn(async move {
            if let Ok(mut tcp) = TcpStream::connect(addr).await {
                let mut stream = channel.into_stream();
                let _ = tokio::io::copy_bidirectional(&mut stream, &mut tcp).await;
            }
        });
        Ok(())
    }
}

fn key(pem: &str) -> PrivateKey {
    decode_secret_key(pem, None).unwrap()
}

/// Starts a server on a random local port and returns the port.
async fn start_server(host_key: PrivateKey) -> u16 {
    let config = Arc::new(server::Config {
        keys: vec![host_key],
        auth_rejection_time: Duration::from_millis(10),
        auth_rejection_time_initial: Some(Duration::ZERO),
        ..Default::default()
    });
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let authorized = Arc::<std::sync::Mutex<Vec<String>>>::default();
    tokio::spawn(async move {
        while let Ok((socket, _)) = listener.accept().await {
            let config = config.clone();
            let handler = TestServer {
                authorized: authorized.clone(),
                ..Default::default()
            };
            tokio::spawn(async move {
                if let Ok(session) = server::run_stream(config, socket, handler).await {
                    let _ = session.await;
                }
            });
        }
    });
    port
}

struct Fixture {
    _dir: tempfile::TempDir,
    store: HostStore,
    pool: SshPool,
    known_hosts: std::path::PathBuf,
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let store = HostStore::load(dir.path().join("hosts.json")).unwrap();
    let known_hosts = dir.path().join("known_hosts");
    Fixture {
        pool: SshPool::new(known_hosts.clone()),
        known_hosts,
        store,
        _dir: dir,
    }
}

fn add_host(store: &HostStore, name: &str, hostname: &str, port: u16, jump: Option<&Host>) -> Host {
    store
        .save(HostInput {
            id: None,
            name: name.into(),
            hostname: hostname.into(),
            port,
            username: "tester".into(),
            auth: AuthKind::Password,
            key_path: None,
            password: None,
            tags: vec![],
            favorite: false,
            jump_host_id: jump.map(|h| h.id.clone()),
        })
        .unwrap()
}

fn with_password() -> ConnectOptions {
    ConnectOptions {
        password: Some(PASSWORD.into()),
        ..Default::default()
    }
}

/// Connects once to learn the fingerprint, then again trusting it.
async fn trust(f: &Fixture, host: &Host, opts: &ConnectOptions) {
    let err = f
        .pool
        .get_or_connect(&f.store, &host.id, opts)
        .await
        .err()
        .unwrap();
    let AppError::HostKeyUnknown { fingerprint, .. } = err else {
        panic!("expected unknown host key, got {err:?}");
    };
    assert!(fingerprint.starts_with("SHA256:"));
    let opts = ConnectOptions {
        trust_fingerprint: Some(fingerprint),
        ..opts.clone()
    };
    f.pool
        .get_or_connect(&f.store, &host.id, &opts)
        .await
        .unwrap();
}

#[tokio::test]
async fn trusts_host_key_runs_command_and_remembers_key() {
    let port = start_server(key(KEY_A)).await;
    let f = fixture();
    let host = add_host(&f.store, "a", "127.0.0.1", port, None);

    trust(&f, &host, &with_password()).await;
    let conn = f.pool.existing(&host.id).expect("connection is pooled");
    let out = exec(&conn, "uptime", Duration::from_secs(5)).await.unwrap();
    assert_eq!(out.stdout, "ran: uptime\n");
    assert_eq!(out.exit_code, Some(0));

    // A new pool with the same known_hosts file connects without asking.
    let pool = SshPool::new(f.known_hosts.clone());
    pool.get_or_connect(&f.store, &host.id, &with_password())
        .await
        .unwrap();
}

#[tokio::test]
async fn reports_changed_host_key() {
    let port = start_server(key(KEY_A)).await;
    let f = fixture();
    let host = add_host(&f.store, "a", "127.0.0.1", port, None);
    // Pretend we saw a different key for this address before.
    russh::keys::known_hosts::learn_known_hosts_path(
        "127.0.0.1",
        port,
        key(KEY_B).public_key(),
        &f.known_hosts,
    )
    .unwrap();

    let err = f
        .pool
        .get_or_connect(&f.store, &host.id, &with_password())
        .await
        .err()
        .unwrap();
    assert!(matches!(err, AppError::HostKeyChanged { .. }), "{err:?}");
}

#[tokio::test]
async fn asks_for_password_and_rejects_wrong_one() {
    let port = start_server(key(KEY_A)).await;
    let f = fixture();
    let host = add_host(&f.store, "a", "127.0.0.1", port, None);
    trust(&f, &host, &with_password()).await;
    f.pool.disconnect(&host.id).await;

    let err = f
        .pool
        .get_or_connect(&f.store, &host.id, &ConnectOptions::default())
        .await
        .err()
        .unwrap();
    assert!(matches!(err, AppError::NeedPassword { .. }), "{err:?}");

    let wrong = ConnectOptions {
        password: Some("nope".into()),
        ..Default::default()
    };
    let err = f
        .pool
        .get_or_connect(&f.store, &host.id, &wrong)
        .await
        .err()
        .unwrap();
    assert!(matches!(err, AppError::Auth(_)), "{err:?}");
}

#[tokio::test]
async fn connects_through_jump_host() {
    let jump_port = start_server(key(KEY_A)).await;
    let target_port = start_server(key(KEY_B)).await;
    let f = fixture();
    let jump = add_host(&f.store, "jump", "127.0.0.1", jump_port, None);
    let target = add_host(&f.store, "target", "127.0.0.1", target_port, Some(&jump));

    // The jump host connects with stored credentials only, so connect it first.
    trust(&f, &jump, &with_password()).await;
    trust(&f, &target, &with_password()).await;

    let conn = f.pool.existing(&target.id).unwrap();
    let out = exec(&conn, "hostname", Duration::from_secs(5))
        .await
        .unwrap();
    assert_eq!(out.stdout, "ran: hostname\n");
}

#[tokio::test]
async fn terminal_echoes_input_and_reports_exit() {
    let port = start_server(key(KEY_A)).await;
    let f = fixture();
    let host = add_host(&f.store, "a", "127.0.0.1", port, None);
    trust(&f, &host, &with_password()).await;
    let conn = f.pool.existing(&host.id).unwrap();

    let (out_tx, mut out_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
    let output = tauri::ipc::Channel::new(move |body| {
        if let tauri::ipc::InvokeResponseBody::Raw(bytes) = body {
            let _ = out_tx.send(bytes);
        }
        Ok(())
    });
    let (exit_tx, exit_rx) = tokio::sync::oneshot::channel();
    let terminals = TerminalManager::default();
    let id = terminals
        .open(conn, &host.id, 80, 24, output, move |exit| {
            let _ = exit_tx.send(exit);
        })
        .await
        .unwrap();

    let mut seen = Vec::new();
    let wait_for = |seen: &[u8], needle: &[u8]| seen.windows(needle.len()).any(|w| w == needle);
    terminals.write(id, b"hello".to_vec()).unwrap();
    while !wait_for(&seen, b"$ hello") {
        let chunk = tokio::time::timeout(Duration::from_secs(5), out_rx.recv())
            .await
            .expect("output in time")
            .expect("channel open");
        seen.extend(chunk);
    }

    terminals.resize(id, 120, 40).unwrap();
    terminals.write(id, b"exit\r".to_vec()).unwrap();
    let exit = tokio::time::timeout(Duration::from_secs(5), exit_rx)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(exit.id, id);
    assert_eq!(exit.exit_code, Some(7));
    assert!(
        terminals.write(id, b"x".to_vec()).is_err(),
        "closed terminals reject input"
    );
}

#[tokio::test]
async fn sets_up_key_login_and_checks_it() {
    let port = start_server(key(KEY_A)).await;
    let f = fixture();
    let host = add_host(&f.store, "a", "127.0.0.1", port, None);
    trust(&f, &host, &with_password()).await;

    let ssh_dir = tempfile::tempdir().unwrap();
    let local = keysetup::ensure_key(ssh_dir.path()).unwrap();
    let mut with_key = host.clone();
    with_key.auth = AuthKind::Key;
    with_key.key_path = Some(local.path.display().to_string());

    // Before the key is installed the server turns it away.
    let err = f
        .pool
        .verify_login(&f.store, &with_key)
        .await
        .err()
        .unwrap();
    assert!(matches!(err, AppError::Auth(_)), "got {err:?}");

    let conn = f.pool.existing(&host.id).unwrap();
    keysetup::install(&conn, &local.public_line).await.unwrap();
    f.pool.verify_login(&f.store, &with_key).await.unwrap();
}
