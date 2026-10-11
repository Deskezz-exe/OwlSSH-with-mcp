use tauri::ipc::Channel;
use tauri::State;

use crate::error::AppResult;
use crate::ssh::sftp::{self, Listing, TransferEvent};
use crate::ssh::ConnectOptions;
use crate::state::AppState;

async fn session(
    state: &AppState,
    host_id: &str,
) -> AppResult<std::sync::Arc<russh_sftp::client::SftpSession>> {
    let conn = state
        .pool
        .get_or_connect(&state.hosts, host_id, &ConnectOptions::default())
        .await?;
    state.sftp.session(host_id, &conn).await
}

/// `path: None` opens the login folder.
#[tauri::command]
pub async fn sftp_list(
    state: State<'_, AppState>,
    host_id: String,
    path: Option<String>,
) -> AppResult<Listing> {
    let s = session(&state, &host_id).await?;
    sftp::list_remote(&s, path.as_deref().unwrap_or(".")).await
}

#[tauri::command]
pub async fn sftp_mkdir(
    state: State<'_, AppState>,
    host_id: String,
    path: String,
) -> AppResult<()> {
    let s = session(&state, &host_id).await?;
    s.create_dir(path)
        .await
        .map_err(|e| crate::error::AppError::Sftp(e.to_string()))
}

#[tauri::command]
pub async fn sftp_rename(
    state: State<'_, AppState>,
    host_id: String,
    from: String,
    to: String,
) -> AppResult<()> {
    let s = session(&state, &host_id).await?;
    s.rename(from, to)
        .await
        .map_err(|e| crate::error::AppError::Sftp(e.to_string()))
}

/// Deletes files or whole folders. Only the UI calls this, after the user confirmed.
#[tauri::command]
pub async fn sftp_remove(
    state: State<'_, AppState>,
    host_id: String,
    paths: Vec<String>,
) -> AppResult<()> {
    let s = session(&state, &host_id).await?;
    for path in paths {
        sftp::remove_remote(&s, &path).await?;
    }
    Ok(())
}

/// Starts an upload in the background and returns its job id right away.
#[tauri::command]
pub async fn sftp_upload(
    state: State<'_, AppState>,
    host_id: String,
    local_paths: Vec<String>,
    remote_dir: String,
    events: Channel<TransferEvent>,
) -> AppResult<u32> {
    let s = session(&state, &host_id).await?;
    let (job, cancel) = state.sftp.new_job();
    let manager = state.sftp.clone();
    tokio::spawn(async move {
        sftp::upload(&s, &local_paths, &remote_dir, job, &cancel, &events).await;
        manager.finish_job(job);
    });
    Ok(job)
}

#[tauri::command]
pub async fn sftp_download(
    state: State<'_, AppState>,
    host_id: String,
    remote_paths: Vec<String>,
    local_dir: String,
    events: Channel<TransferEvent>,
) -> AppResult<u32> {
    let s = session(&state, &host_id).await?;
    let (job, cancel) = state.sftp.new_job();
    let manager = state.sftp.clone();
    tokio::spawn(async move {
        sftp::download(&s, &remote_paths, &local_dir, job, &cancel, &events).await;
        manager.finish_job(job);
    });
    Ok(job)
}

#[tauri::command]
pub fn sftp_cancel(state: State<'_, AppState>, job: u32) {
    state.sftp.cancel(job);
}

#[tauri::command]
pub async fn local_list(path: Option<String>) -> AppResult<Listing> {
    let path = path.unwrap_or_else(sftp::local_home);
    tokio::task::spawn_blocking(move || sftp::list_local(&path))
        .await
        .map_err(|e| crate::error::AppError::Io(std::io::Error::other(e)))?
}

#[tauri::command]
pub fn local_roots() -> Vec<String> {
    sftp::local_roots()
}

#[tauri::command]
pub async fn local_mkdir(path: String) -> AppResult<()> {
    Ok(tokio::fs::create_dir(path).await?)
}

#[tauri::command]
pub async fn local_remove(paths: Vec<String>) -> AppResult<()> {
    for path in paths {
        let meta = tokio::fs::symlink_metadata(&path).await?;
        if meta.is_dir() {
            tokio::fs::remove_dir_all(&path).await?;
        } else {
            tokio::fs::remove_file(&path).await?;
        }
    }
    Ok(())
}
