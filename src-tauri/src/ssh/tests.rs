//! End-to-end tests of the SSH engine against an in-process russh server:
//! host key trust, password auth, exec and jump hosts.

use std::sync::Arc;
use std::time::Duration;

use russh::keys::{decode_secret_key, PrivateKey};
use russh::server::{self, Auth, Msg, Session};
use russh::{Channel, ChannelId};
use tokio::net::{TcpListener, TcpStream};

use super::exec::exec;
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

#[derive(Clone)]
struct TestServer;

impl server::Handler for TestServer {
    type Error = russh::Error;

    async fn auth_password(&mut self, _user: &str, password: &str) -> Result<Auth, Self::Error> {
        Ok(if password == PASSWORD {
            Auth::Accept
        } else {
            Auth::reject()
        })
    }

    async fn channel_open_session(
        &mut self,
        _channel: Channel<Msg>,
        reply: server::ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
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
        let out = format!("ran: {}\n", String::from_utf8_lossy(data));
        session.data(channel, out.into_bytes())?;
        session.exit_status_request(channel, 0)?;
        session.eof(channel)?;
        session.close(channel)?;
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
    tokio::spawn(async move {
        while let Ok((socket, _)) = listener.accept().await {
            let config = config.clone();
            tokio::spawn(async move {
                if let Ok(session) = server::run_stream(config, socket, TestServer).await {
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
