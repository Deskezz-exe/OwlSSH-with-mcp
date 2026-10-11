//! SFTP on top of the pooled connections, plus the local side of the file manager.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, UNIX_EPOCH};

use russh_sftp::client::SftpSession;
use russh_sftp::protocol::{FileAttributes, OpenFlags};
use serde::Serialize;
use tauri::ipc::Channel;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::pool::SshConn;
use crate::error::{AppError, AppResult};

const CHUNK: usize = 256 * 1024;
const PROGRESS_EVERY: Duration = Duration::from_millis(120);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EntryKind {
    Dir,
    File,
    Symlink,
    Other,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub name: String,
    pub path: String,
    pub kind: EntryKind,
    pub size: u64,
    /// Unix seconds.
    pub modified: Option<u64>,
    /// `rwxr-xr-x` style, remote only.
    pub permissions: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Listing {
    pub path: String,
    pub entries: Vec<Entry>,
}

fn sftp_err(e: impl std::fmt::Display) -> AppError {
    AppError::Sftp(e.to_string())
}

/// Joins remote paths with `/` whatever the local OS is.
pub fn remote_join(dir: &str, name: &str) -> String {
    if dir.ends_with('/') {
        format!("{dir}{name}")
    } else {
        format!("{dir}/{name}")
    }
}

fn mode_string(mode: u32) -> String {
    let bits = [
        (0o400, 'r'),
        (0o200, 'w'),
        (0o100, 'x'),
        (0o040, 'r'),
        (0o020, 'w'),
        (0o010, 'x'),
        (0o004, 'r'),
        (0o002, 'w'),
        (0o001, 'x'),
    ];
    bits.iter()
        .map(|&(bit, ch)| if mode & bit != 0 { ch } else { '-' })
        .collect()
}

fn remote_kind(attrs: &FileAttributes) -> EntryKind {
    let t = attrs.file_type();
    if t.is_dir() {
        EntryKind::Dir
    } else if t.is_symlink() {
        EntryKind::Symlink
    } else if t.is_file() {
        EntryKind::File
    } else {
        EntryKind::Other
    }
}

/// Folders first, then by name, case-insensitive.
fn sort_entries(entries: &mut [Entry]) {
    entries.sort_by(|a, b| {
        (a.kind != EntryKind::Dir)
            .cmp(&(b.kind != EntryKind::Dir))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
}

/// One SFTP subsystem channel per host, opened on first use.
#[derive(Default)]
pub struct SftpManager {
    sessions: Mutex<HashMap<String, Arc<SftpSession>>>,
    next_job: AtomicU32,
    cancels: Mutex<HashMap<u32, Arc<AtomicBool>>>,
}

impl SftpManager {
    pub async fn session(&self, host_id: &str, conn: &SshConn) -> AppResult<Arc<SftpSession>> {
        if let Some(s) = self.sessions.lock().unwrap().get(host_id).cloned() {
            if !conn.is_closed() {
                return Ok(s);
            }
        }
        let channel = conn.handle.channel_open_session().await?;
        channel.request_subsystem(true, "sftp").await?;
        let session = Arc::new(
            SftpSession::new(channel.into_stream())
                .await
                .map_err(sftp_err)?,
        );
        session.set_timeout(30);
        self.sessions
            .lock()
            .unwrap()
            .insert(host_id.to_string(), session.clone());
        Ok(session)
    }

    /// Drops the cached session, e.g. after the host disconnected.
    pub fn forget(&self, host_id: &str) {
        self.sessions.lock().unwrap().remove(host_id);
    }

    pub fn new_job(&self) -> (u32, Arc<AtomicBool>) {
        let id = self.next_job.fetch_add(1, Ordering::Relaxed) + 1;
        let flag = Arc::new(AtomicBool::new(false));
        self.cancels.lock().unwrap().insert(id, flag.clone());
        (id, flag)
    }

    pub fn finish_job(&self, id: u32) {
        self.cancels.lock().unwrap().remove(&id);
    }

    pub fn cancel(&self, id: u32) {
        if let Some(flag) = self.cancels.lock().unwrap().get(&id) {
            flag.store(true, Ordering::Relaxed);
        }
    }
}

pub async fn list_remote(sftp: &SftpSession, path: &str) -> AppResult<Listing> {
    let path = sftp.canonicalize(path).await.map_err(sftp_err)?;
    let mut entries: Vec<Entry> = sftp
        .read_dir(path.clone())
        .await
        .map_err(sftp_err)?
        .filter(|e| e.file_name() != "." && e.file_name() != "..")
        .map(|e| {
            let attrs = e.metadata();
            Entry {
                name: e.file_name(),
                path: remote_join(&path, &e.file_name()),
                kind: remote_kind(&attrs),
                size: attrs.size.unwrap_or(0),
                modified: attrs.mtime.map(u64::from),
                permissions: attrs.permissions.map(mode_string),
            }
        })
        .collect();
    sort_entries(&mut entries);
    Ok(Listing { path, entries })
}

/// Removes a file, or a folder with everything in it.
pub async fn remove_remote(sftp: &SftpSession, path: &str) -> AppResult<()> {
    let attrs = sftp.symlink_metadata(path).await.map_err(sftp_err)?;
    if remote_kind(&attrs) != EntryKind::Dir {
        return sftp.remove_file(path).await.map_err(sftp_err);
    }
    // Depth-first without recursion: collect folders, delete files on the way.
    let mut dirs = vec![path.to_string()];
    let mut stack = vec![path.to_string()];
    while let Some(dir) = stack.pop() {
        for e in sftp.read_dir(dir.clone()).await.map_err(sftp_err)? {
            let name = e.file_name();
            if name == "." || name == ".." {
                continue;
            }
            let child = remote_join(&dir, &name);
            if remote_kind(&e.metadata()) == EntryKind::Dir {
                dirs.push(child.clone());
                stack.push(child);
            } else {
                sftp.remove_file(child).await.map_err(sftp_err)?;
            }
        }
    }
    for dir in dirs.iter().rev() {
        sftp.remove_dir(dir.clone()).await.map_err(sftp_err)?;
    }
    Ok(())
}

// ---------- Local side ----------

pub fn local_home() -> String {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/"))
        .to_string_lossy()
        .into_owned()
}

pub fn list_local(path: &str) -> AppResult<Listing> {
    let dir = std::fs::canonicalize(path)?;
    let mut entries = Vec::new();
    for item in std::fs::read_dir(&dir)? {
        let Ok(item) = item else { continue };
        let Ok(meta) = item.metadata() else { continue };
        let ft = item.file_type().ok();
        let kind = if ft.is_some_and(|t| t.is_symlink()) {
            EntryKind::Symlink
        } else if meta.is_dir() {
            EntryKind::Dir
        } else if meta.is_file() {
            EntryKind::File
        } else {
            EntryKind::Other
        };
        entries.push(Entry {
            name: item.file_name().to_string_lossy().into_owned(),
            path: item.path().to_string_lossy().into_owned(),
            kind,
            size: if meta.is_file() { meta.len() } else { 0 },
            modified: meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs()),
            permissions: None,
        });
    }
    sort_entries(&mut entries);
    Ok(Listing {
        path: display_path(&dir),
        entries,
    })
}

/// `canonicalize` on Windows returns `\\?\C:\...`; show the familiar form.
fn display_path(p: &Path) -> String {
    let s = p.to_string_lossy();
    s.strip_prefix(r"\\?\").unwrap_or(&s).to_string()
}

/// Drive roots on Windows (`C:\`, `D:\`, ...), `/` elsewhere.
pub fn local_roots() -> Vec<String> {
    #[cfg(windows)]
    {
        (b'A'..=b'Z')
            .map(|c| format!("{}:\\", c as char))
            .filter(|root| Path::new(root).exists())
            .collect()
    }
    #[cfg(not(windows))]
    {
        vec!["/".to_string()]
    }
}

// ---------- Transfers ----------

#[derive(Debug, Clone, Serialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "event"
)]
pub enum TransferEvent {
    Started {
        job: u32,
        files_total: u32,
        bytes_total: u64,
    },
    Progress {
        job: u32,
        file: String,
        files_done: u32,
        bytes_done: u64,
    },
    Finished {
        job: u32,
    },
    Failed {
        job: u32,
        message: String,
    },
    Cancelled {
        job: u32,
    },
}

/// One file to copy: where from, where to, how big.
struct Item {
    from: String,
    to: String,
    size: u64,
}

struct Progress<'a> {
    job: u32,
    events: &'a Channel<TransferEvent>,
    cancel: &'a AtomicBool,
    files_done: u32,
    bytes_done: u64,
    last: Instant,
}

impl Progress<'_> {
    fn add(&mut self, bytes: u64, file: &str, force: bool) -> AppResult<()> {
        if self.cancel.load(Ordering::Relaxed) {
            return Err(AppError::Sftp("cancelled".into()));
        }
        self.bytes_done += bytes;
        if force || self.last.elapsed() >= PROGRESS_EVERY {
            self.last = Instant::now();
            let _ = self.events.send(TransferEvent::Progress {
                job: self.job,
                file: file.to_string(),
                files_done: self.files_done,
                bytes_done: self.bytes_done,
            });
        }
        Ok(())
    }
}

/// Runs a transfer and reports the outcome on `events`. Never returns an error:
/// failures and cancellation are events, so the UI has one place to look.
async fn run_job<F>(job: u32, cancel: &AtomicBool, events: &Channel<TransferEvent>, work: F)
where
    F: std::future::Future<Output = AppResult<()>>,
{
    let result = work.await;
    let event = match result {
        Ok(()) => TransferEvent::Finished { job },
        Err(_) if cancel.load(Ordering::Relaxed) => TransferEvent::Cancelled { job },
        Err(e) => TransferEvent::Failed {
            job,
            message: e.to_string(),
        },
    };
    let _ = events.send(event);
}

/// Something to transfer. `name` gives it a different name at the destination
/// (used when the user keeps both copies after a name conflict).
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub path: String,
    #[serde(default)]
    pub name: Option<String>,
}

impl Source {
    pub fn plain(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            name: None,
        }
    }
}

/// Which of these remote paths already exist.
pub async fn remote_exists(sftp: &SftpSession, paths: &[String]) -> Vec<bool> {
    let mut out = Vec::with_capacity(paths.len());
    for path in paths {
        out.push(sftp.symlink_metadata(path.clone()).await.is_ok());
    }
    out
}

/// Copies local files and folders into `remote_dir`.
pub async fn upload(
    sftp: &SftpSession,
    sources: &[Source],
    remote_dir: &str,
    job: u32,
    cancel: &AtomicBool,
    events: &Channel<TransferEvent>,
) {
    run_job(job, cancel, events, async {
        let mut items = Vec::new();
        let mut dirs = Vec::new();
        for src in sources {
            plan_upload(
                Path::new(&src.path),
                remote_dir,
                src.name.as_deref(),
                &mut items,
                &mut dirs,
            )?;
        }
        start(job, &items, events);
        for dir in &dirs {
            // An existing folder is fine; any other problem shows up on the first file.
            let _ = sftp.create_dir(dir.clone()).await;
        }
        let mut p = progress(job, events, cancel);
        for item in &items {
            let mut src = tokio::fs::File::open(&item.from).await?;
            let mut dst = sftp
                .open_with_flags(
                    item.to.clone(),
                    OpenFlags::CREATE | OpenFlags::TRUNCATE | OpenFlags::WRITE,
                )
                .await
                .map_err(sftp_err)?;
            let mut buf = vec![0u8; CHUNK];
            loop {
                let n = src.read(&mut buf).await?;
                if n == 0 {
                    break;
                }
                dst.write_all(&buf[..n]).await?;
                p.add(n as u64, &item.to, false)?;
            }
            dst.shutdown().await?;
            p.files_done += 1;
            p.add(0, &item.to, true)?;
        }
        Ok(())
    })
    .await
}

fn plan_upload(
    local: &Path,
    remote_dir: &str,
    rename: Option<&str>,
    items: &mut Vec<Item>,
    dirs: &mut Vec<String>,
) -> AppResult<()> {
    let name = match rename {
        Some(n) => n.to_string(),
        None => local
            .file_name()
            .ok_or_else(|| AppError::Invalid(format!("cannot upload {}", local.display())))?
            .to_string_lossy()
            .into_owned(),
    };
    let target = remote_join(remote_dir, &name);
    let meta = std::fs::metadata(local)?;
    if meta.is_dir() {
        dirs.push(target.clone());
        for child in std::fs::read_dir(local)? {
            plan_upload(&child?.path(), &target, None, items, dirs)?;
        }
    } else {
        items.push(Item {
            from: local.to_string_lossy().into_owned(),
            to: target,
            size: meta.len(),
        });
    }
    Ok(())
}

/// Copies remote files and folders into `local_dir`.
pub async fn download(
    sftp: &SftpSession,
    sources: &[Source],
    local_dir: &str,
    job: u32,
    cancel: &AtomicBool,
    events: &Channel<TransferEvent>,
) {
    run_job(job, cancel, events, async {
        let mut items = Vec::new();
        let mut dirs = Vec::new();
        for src in sources {
            plan_download(
                sftp,
                &src.path,
                Path::new(local_dir),
                src.name.as_deref(),
                &mut items,
                &mut dirs,
            )
            .await?;
        }
        start(job, &items, events);
        for dir in &dirs {
            tokio::fs::create_dir_all(dir).await?;
        }
        let mut p = progress(job, events, cancel);
        for item in &items {
            let mut src = sftp.open(item.from.clone()).await.map_err(sftp_err)?;
            let mut dst = tokio::fs::File::create(&item.to).await?;
            let mut buf = vec![0u8; CHUNK];
            loop {
                let n = src.read(&mut buf).await?;
                if n == 0 {
                    break;
                }
                dst.write_all(&buf[..n]).await?;
                p.add(n as u64, &item.to, false)?;
            }
            dst.flush().await?;
            p.files_done += 1;
            p.add(0, &item.to, true)?;
        }
        Ok(())
    })
    .await
}

async fn plan_download(
    sftp: &SftpSession,
    remote: &str,
    local_dir: &Path,
    rename: Option<&str>,
    items: &mut Vec<Item>,
    dirs: &mut Vec<String>,
) -> AppResult<()> {
    let name = match rename {
        Some(n) => n,
        None => remote
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .filter(|n| !n.is_empty())
            .ok_or_else(|| AppError::Invalid(format!("cannot download {remote}")))?,
    };
    let mut stack = vec![(remote.to_string(), local_dir.join(name))];
    while let Some((from, to)) = stack.pop() {
        let attrs = sftp.metadata(from.clone()).await.map_err(sftp_err)?;
        if remote_kind(&attrs) == EntryKind::Dir {
            dirs.push(to.to_string_lossy().into_owned());
            for e in sftp.read_dir(from.clone()).await.map_err(sftp_err)? {
                let child = e.file_name();
                if child != "." && child != ".." {
                    stack.push((remote_join(&from, &child), to.join(&child)));
                }
            }
        } else {
            items.push(Item {
                from,
                to: to.to_string_lossy().into_owned(),
                size: attrs.size.unwrap_or(0),
            });
        }
    }
    Ok(())
}

fn start(job: u32, items: &[Item], events: &Channel<TransferEvent>) {
    let _ = events.send(TransferEvent::Started {
        job,
        files_total: items.len() as u32,
        bytes_total: items.iter().map(|i| i.size).sum(),
    });
}

fn progress<'a>(
    job: u32,
    events: &'a Channel<TransferEvent>,
    cancel: &'a AtomicBool,
) -> Progress<'a> {
    Progress {
        job,
        events,
        cancel,
        files_done: 0,
        bytes_done: 0,
        last: Instant::now(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_permissions() {
        assert_eq!(mode_string(0o755), "rwxr-xr-x");
        assert_eq!(mode_string(0o100644), "rw-r--r--");
        assert_eq!(mode_string(0), "---------");
    }

    #[test]
    fn joins_remote_paths() {
        assert_eq!(remote_join("/", "etc"), "/etc");
        assert_eq!(remote_join("/home/user", "a.txt"), "/home/user/a.txt");
    }

    #[test]
    fn lists_local_folders_first() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("b.txt"), "hello").unwrap();
        std::fs::create_dir(dir.path().join("z-dir")).unwrap();
        std::fs::write(dir.path().join("A.txt"), "").unwrap();
        let listing = list_local(&dir.path().to_string_lossy()).unwrap();
        let names: Vec<_> = listing.entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["z-dir", "A.txt", "b.txt"]);
        assert_eq!(listing.entries[2].size, 5);
        assert!(!listing.path.starts_with(r"\\?\"));
    }

    #[test]
    fn upload_can_rename_the_top_item() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("report.pdf");
        std::fs::write(&file, "x").unwrap();
        let (mut items, mut dirs) = (Vec::new(), Vec::new());
        plan_upload(&file, "/srv", Some("report (1).pdf"), &mut items, &mut dirs).unwrap();
        assert_eq!(items[0].to, "/srv/report (1).pdf");
    }

    #[test]
    fn plans_nested_upload() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("site");
        std::fs::create_dir_all(root.join("css")).unwrap();
        std::fs::write(root.join("index.html"), "<h1>").unwrap();
        std::fs::write(root.join("css/app.css"), "body{}").unwrap();
        let (mut items, mut dirs) = (Vec::new(), Vec::new());
        plan_upload(&root, "/var/www", None, &mut items, &mut dirs).unwrap();
        dirs.sort();
        assert_eq!(dirs, ["/var/www/site", "/var/www/site/css"]);
        let mut targets: Vec<_> = items.iter().map(|i| i.to.as_str()).collect();
        targets.sort();
        assert_eq!(
            targets,
            ["/var/www/site/css/app.css", "/var/www/site/index.html"]
        );
        assert_eq!(items.iter().map(|i| i.size).sum::<u64>(), 10);
    }
}

#[cfg(test)]
mod event_tests {
    use super::TransferEvent;

    #[test]
    fn transfer_events_use_camel_case_fields() {
        let json = serde_json::to_value(TransferEvent::Started {
            job: 1,
            files_total: 3,
            bytes_total: 10,
        })
        .unwrap();
        assert_eq!(json["event"], "started");
        assert_eq!(json["filesTotal"], 3);
        assert_eq!(json["bytesTotal"], 10);
    }
}
