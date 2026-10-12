//! Interactive shells (PTY channels).
//!
//! Each terminal runs in its own task that owns the SSH channel. Keystrokes
//! arrive over an unbounded queue and are written to the channel right away;
//! output is forwarded to the webview as raw bytes. When several output packets
//! are already waiting they are merged into one message, which keeps fast
//! output (`cat`, `journalctl -f`) cheap without delaying a lone echo.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use russh::ChannelMsg;
use tauri::ipc::{Channel, InvokeResponseBody};
use tokio::sync::mpsc;

use super::pool::SshConn;
use crate::error::{AppError, AppResult};

/// Upper bound for one merged output message.
const MAX_BATCH: usize = 256 * 1024;

enum Command {
    Input(Vec<u8>),
    Resize { cols: u32, rows: u32 },
    Close,
}

struct Session {
    host_id: String,
    tx: mpsc::UnboundedSender<Command>,
}

/// Why a terminal ended, sent to the UI.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalExit {
    pub id: u32,
    pub exit_code: Option<u32>,
    /// The channel went away without an exit status and without us closing it:
    /// a dropped network or a killed server process. The UI may reconnect.
    pub lost: bool,
}

#[derive(Default)]
pub struct TerminalManager {
    next_id: AtomicU32,
    sessions: Arc<Mutex<HashMap<u32, Session>>>,
}

impl TerminalManager {
    /// Opens a shell on `conn`. Output goes to `output`, `on_exit` runs once when it ends.
    pub async fn open(
        &self,
        conn: Arc<SshConn>,
        host_id: &str,
        cols: u32,
        rows: u32,
        output: Channel<InvokeResponseBody>,
        on_exit: impl FnOnce(TerminalExit) + Send + 'static,
    ) -> AppResult<u32> {
        let channel = conn.handle.channel_open_session().await?;
        channel
            .request_pty(false, "xterm-256color", cols, rows, 0, 0, &[])
            .await?;
        channel.request_shell(false).await?;

        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        let (tx, mut rx) = mpsc::unbounded_channel();
        self.sessions.lock().unwrap().insert(
            id,
            Session {
                host_id: host_id.to_string(),
                tx,
            },
        );

        let sessions = self.sessions.clone();
        let (mut reader, writer) = channel.split();
        tokio::spawn(async move {
            // Keeps the connection alive for as long as the shell runs.
            let _conn = conn;
            let mut exit_code = None;
            let mut closed_by_us = false;
            loop {
                tokio::select! {
                    biased;
                    cmd = rx.recv() => match cmd {
                        Some(Command::Input(bytes)) => {
                            if writer.data_bytes(bytes).await.is_err() {
                                break;
                            }
                        }
                        Some(Command::Resize { cols, rows }) => {
                            let _ = writer.window_change(cols, rows, 0, 0).await;
                        }
                        Some(Command::Close) | None => {
                            closed_by_us = true;
                            let _ = writer.eof().await;
                            let _ = writer.close().await;
                            break;
                        }
                    },
                    msg = reader.wait() => match msg {
                        Some(ChannelMsg::Data { data }) => {
                            let mut batch = data.to_vec();
                            // Merge what is already queued, without waiting for more.
                            while batch.len() < MAX_BATCH {
                                match tokio::time::timeout(Duration::ZERO, reader.wait()).await {
                                    Ok(Some(ChannelMsg::Data { data })) => batch.extend_from_slice(&data),
                                    Ok(Some(ChannelMsg::ExitStatus { exit_status })) => {
                                        exit_code = Some(exit_status);
                                    }
                                    Ok(Some(_)) => {}
                                    Ok(None) | Err(_) => break,
                                }
                            }
                            if output.send(InvokeResponseBody::Raw(batch)).is_err() {
                                break;
                            }
                        }
                        Some(ChannelMsg::ExtendedData { data, .. }) => {
                            let _ = output.send(InvokeResponseBody::Raw(data.to_vec()));
                        }
                        Some(ChannelMsg::ExitStatus { exit_status }) => exit_code = Some(exit_status),
                        Some(ChannelMsg::Eof) | Some(ChannelMsg::Close) | None => break,
                        Some(_) => {}
                    },
                }
            }
            sessions.lock().unwrap().remove(&id);
            let lost = exit_code.is_none() && !closed_by_us;
            on_exit(TerminalExit {
                id,
                exit_code,
                lost,
            });
        });
        Ok(id)
    }

    pub fn write(&self, id: u32, data: Vec<u8>) -> AppResult<()> {
        self.send(id, Command::Input(data))
    }

    pub fn resize(&self, id: u32, cols: u32, rows: u32) -> AppResult<()> {
        self.send(id, Command::Resize { cols, rows })
    }

    pub fn close(&self, id: u32) -> AppResult<()> {
        self.send(id, Command::Close)
    }

    /// Ends every terminal of a host, used before disconnecting it.
    pub fn close_host(&self, host_id: &str) {
        for session in self.sessions.lock().unwrap().values() {
            if session.host_id == host_id {
                let _ = session.tx.send(Command::Close);
            }
        }
    }

    fn send(&self, id: u32, cmd: Command) -> AppResult<()> {
        let sessions = self.sessions.lock().unwrap();
        let session = sessions
            .get(&id)
            .ok_or_else(|| AppError::NotFound(format!("terminal {id} is closed")))?;
        session
            .tx
            .send(cmd)
            .map_err(|_| AppError::NotFound(format!("terminal {id} is closed")))
    }
}
