use std::time::Duration;

use russh::ChannelMsg;
use serde::Serialize;

use super::pool::SshConn;
use crate::error::{AppError, AppResult};

/// Output above this size is cut off so a runaway command cannot eat memory.
const MAX_OUTPUT: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<u32>,
    pub truncated: bool,
}

/// Runs one command on a fresh exec channel and collects its output.
pub async fn exec(conn: &SshConn, command: &str, timeout: Duration) -> AppResult<ExecOutput> {
    let run = async {
        let mut channel = conn.handle.channel_open_session().await?;
        channel.exec(true, command).await?;

        let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
        let mut exit_code = None;
        let mut truncated = false;
        while let Some(msg) = channel.wait().await {
            match msg {
                ChannelMsg::Data { data } => append(&mut stdout, &data, &mut truncated),
                ChannelMsg::ExtendedData { data, ext: 1 } => {
                    append(&mut stderr, &data, &mut truncated)
                }
                ChannelMsg::ExitStatus { exit_status } => exit_code = Some(exit_status),
                _ => {}
            }
        }
        Ok::<_, AppError>(ExecOutput {
            stdout: String::from_utf8_lossy(&stdout).into_owned(),
            stderr: String::from_utf8_lossy(&stderr).into_owned(),
            exit_code,
            truncated,
        })
    };
    tokio::time::timeout(timeout, run).await.map_err(|_| {
        AppError::Timeout(format!("command did not finish in {}s", timeout.as_secs()))
    })?
}

fn append(buf: &mut Vec<u8>, data: &[u8], truncated: &mut bool) {
    let room = MAX_OUTPUT.saturating_sub(buf.len());
    if data.len() > room {
        *truncated = true;
    }
    buf.extend_from_slice(&data[..data.len().min(room)]);
}
