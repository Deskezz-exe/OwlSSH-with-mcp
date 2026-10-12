//! Port forwarding over an SSH connection.
//!
//! Local: OwlSSH listens on this computer and carries each connection through
//! a `direct-tcpip` channel to an address the server can reach.
//! Remote: the server listens (`tcpip-forward`) and sends each connection back
//! as a `forwarded-tcpip` channel, which OwlSSH connects to an address here.
//!
//! A forward lives on one connection. When that connection closes the forward
//! ends with an error, so the UI can show it and offer to start it again.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use russh::client::Msg;
use russh::Channel;
use tokio::net::{TcpListener, TcpStream};
use tokio_util::sync::CancellationToken;

use super::pool::SshConn;
use crate::error::{AppError, AppResult};

/// How a forward ended: `None` when it was stopped, otherwise the reason.
pub type OnEnd = Box<dyn FnOnce(Option<String>) + Send + 'static>;

/// Remote forwards by (host id, port on the server), read by the client
/// handler when the server opens a `forwarded-tcpip` channel.
/// `(host, port)` addresses.
type Target = (String, u16);

#[derive(Clone, Default)]
pub struct RemoteForwards(Arc<Mutex<HashMap<(String, u32), Target>>>);

impl RemoteForwards {
    pub fn target(&self, host_id: &str, port: u32) -> Option<(String, u16)> {
        self.0
            .lock()
            .unwrap()
            .get(&(host_id.to_string(), port))
            .cloned()
    }

    fn add(&self, host_id: &str, port: u32, target: (String, u16)) {
        self.0
            .lock()
            .unwrap()
            .insert((host_id.to_string(), port), target);
    }

    fn remove(&self, host_id: &str, port: u32) {
        self.0.lock().unwrap().remove(&(host_id.to_string(), port));
    }
}

/// Connects a channel from the server to `target` on this computer.
pub fn serve_forwarded(channel: Channel<Msg>, target: (String, u16)) {
    tokio::spawn(async move {
        if let Ok(mut tcp) = TcpStream::connect((target.0.as_str(), target.1)).await {
            let _ = tcp.set_nodelay(true);
            let mut stream = channel.into_stream();
            let _ = tokio::io::copy_bidirectional(&mut stream, &mut tcp).await;
        }
    });
}

/// Starts a local forward and returns the port it listens on.
pub async fn start_local(
    conn: Arc<SshConn>,
    bind_host: &str,
    bind_port: u16,
    target_host: String,
    target_port: u16,
    cancel: CancellationToken,
    on_end: OnEnd,
) -> AppResult<u16> {
    let listener = TcpListener::bind((bind_host, bind_port))
        .await
        .map_err(|e| AppError::Invalid(format!("cannot listen on {bind_host}:{bind_port}: {e}")))?;
    let port = listener.local_addr()?.port();

    tokio::spawn(async move {
        let mut check = tokio::time::interval(Duration::from_secs(1));
        let reason = loop {
            tokio::select! {
                _ = cancel.cancelled() => break None,
                _ = check.tick() => {
                    if conn.is_closed() {
                        break Some("the SSH connection closed".to_string());
                    }
                }
                accepted = listener.accept() => {
                    let Ok((mut socket, peer)) = accepted else { continue };
                    let _ = socket.set_nodelay(true);
                    let conn = conn.clone();
                    let target_host = target_host.clone();
                    let cancel = cancel.clone();
                    tokio::spawn(async move {
                        let Ok(channel) = conn
                            .handle
                            .channel_open_direct_tcpip(
                                target_host,
                                u32::from(target_port),
                                peer.ip().to_string(),
                                u32::from(peer.port()),
                            )
                            .await
                        else {
                            return;
                        };
                        let mut stream = channel.into_stream();
                        tokio::select! {
                            _ = cancel.cancelled() => {}
                            _ = tokio::io::copy_bidirectional(&mut socket, &mut stream) => {}
                        }
                    });
                }
            }
        };
        on_end(reason);
    });
    Ok(port)
}

/// Asks the server to listen and returns the port it listens on.
#[allow(clippy::too_many_arguments)]
pub async fn start_remote(
    conn: Arc<SshConn>,
    forwards: RemoteForwards,
    host_id: &str,
    bind_host: &str,
    bind_port: u16,
    target_host: String,
    target_port: u16,
    cancel: CancellationToken,
    on_end: OnEnd,
) -> AppResult<u16> {
    let port = conn
        .handle
        .tcpip_forward(bind_host, u32::from(bind_port))
        .await
        .map_err(|e| {
            AppError::Ssh(format!(
                "the server refused to listen on {bind_host}:{bind_port}: {e}"
            ))
        })?;
    // Port 0 asks the server to pick one; otherwise it echoes nothing useful.
    let port = if bind_port == 0 {
        port
    } else {
        u32::from(bind_port)
    };
    forwards.add(host_id, port, (target_host, target_port));

    let host_id = host_id.to_string();
    let bind_host = bind_host.to_string();
    tokio::spawn(async move {
        let mut check = tokio::time::interval(Duration::from_secs(1));
        let reason = loop {
            tokio::select! {
                _ = cancel.cancelled() => break None,
                _ = check.tick() => {
                    if conn.is_closed() {
                        break Some("the SSH connection closed".to_string());
                    }
                }
            }
        };
        forwards.remove(&host_id, port);
        if reason.is_none() {
            let _ = conn.handle.cancel_tcpip_forward(bind_host, port).await;
        }
        on_end(reason);
    });
    Ok(port as u16)
}
