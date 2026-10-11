//! The tools an AI assistant sees. They run on the same pooled connections
//! as the app, so the assistant works on exactly the servers the user has.
//!
//! The base set reads, runs commands, writes and copies files. Full access mode
//! (off by default, see settings) adds tools that delete, rename and edit.

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerConfig};
use rmcp::service::RequestContext;
use rmcp::{schemars, tool, tool_handler, tool_router, ErrorData, RoleServer, ServerHandler};
use russh_sftp::client::SftpSession;
use serde::Deserialize;
use tauri::ipc::Channel;
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::audit::AuditEntry;
use super::guard;
use crate::error::AppError;
use crate::hosts::Host;
use crate::ssh::sftp::{self, Source, TransferEvent};
use crate::ssh::{exec, metrics, ConnectOptions, SshConn};
use crate::state::AppState;

const MAX_READ: u64 = 1024 * 1024;

/// Tools that only exist while full access is on.
pub const FULL_ACCESS_TOOLS: &[&str] = &[
    "delete_path",
    "rename_path",
    "create_directory",
    "edit_file",
];

#[derive(Clone)]
pub struct OwlTools {
    app: AppHandle,
    tool_router: ToolRouter<Self>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ServerArg {
    #[schemars(description = "Server name, id or address as shown by list_servers")]
    pub server: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RunCommandArgs {
    #[schemars(description = "Server name, id or address as shown by list_servers")]
    pub server: String,
    #[schemars(description = "Shell command to run (non-interactive, no TTY)")]
    pub command: String,
    #[schemars(description = "Seconds to wait before giving up, 1-600. Default 60")]
    pub timeout_secs: Option<u64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PathArgs {
    #[schemars(description = "Server name, id or address as shown by list_servers")]
    pub server: String,
    #[schemars(description = "Absolute path, or relative to the login folder")]
    pub path: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct WriteFileArgs {
    #[schemars(description = "Server name, id or address as shown by list_servers")]
    pub server: String,
    #[schemars(description = "Absolute path of the file to create or replace")]
    pub path: String,
    #[schemars(description = "Full new content of the file (UTF-8 text)")]
    pub content: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct EditFileArgs {
    #[schemars(description = "Server name, id or address as shown by list_servers")]
    pub server: String,
    #[schemars(description = "Absolute path of the text file to change")]
    pub path: String,
    #[schemars(
        description = "Exact text to find. Must appear exactly once unless replace_all is true"
    )]
    pub old_text: String,
    #[schemars(description = "Text to put in its place")]
    pub new_text: String,
    #[schemars(description = "Replace every occurrence instead of exactly one")]
    pub replace_all: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RenameArgs {
    #[schemars(description = "Server name, id or address as shown by list_servers")]
    pub server: String,
    #[schemars(description = "Current path of the file or folder")]
    pub from: String,
    #[schemars(description = "New path (a new name in the same folder, or another folder)")]
    pub to: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct UploadArgs {
    #[schemars(description = "Server name, id or address as shown by list_servers")]
    pub server: String,
    #[schemars(description = "File or folder on this computer")]
    pub local_path: String,
    #[schemars(description = "Folder on the server to copy into")]
    pub remote_dir: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DownloadArgs {
    #[schemars(description = "Server name, id or address as shown by list_servers")]
    pub server: String,
    #[schemars(description = "File or folder on the server")]
    pub remote_path: String,
    #[schemars(description = "Folder on this computer to copy into")]
    pub local_dir: String,
}

fn ok(text: impl Into<String>) -> Result<CallToolResult, ErrorData> {
    Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
}

fn fail(text: impl Into<String>) -> Result<CallToolResult, ErrorData> {
    Ok(CallToolResult::error(vec![ContentBlock::text(text)]))
}

fn explain(e: &AppError) -> String {
    match e {
        AppError::NeedPassword { .. } | AppError::NeedPassphrase { .. } => format!(
            "{e}. Ask the user to connect to this server once in OwlSSH (and save the password), then retry."
        ),
        AppError::HostKeyUnknown { .. } | AppError::HostKeyChanged { .. } => {
            format!("{e}. The user has to confirm the server key in OwlSSH first.")
        }
        _ => e.to_string(),
    }
}

/// A readable name for the MCP client, e.g. "claude-code" -> "Claude Code".
pub fn client_label(name: &str, title: Option<&str>) -> String {
    let lower = name.to_lowercase();
    let known = [
        ("claude-code", "Claude Code"),
        ("claude code", "Claude Code"),
        ("claude-ai", "Claude Desktop"),
        ("claude desktop", "Claude Desktop"),
        ("codex", "Codex"),
        ("cursor", "Cursor"),
        ("windsurf", "Windsurf"),
        ("gemini", "Gemini CLI"),
        ("visual studio code", "VS Code"),
        ("vscode", "VS Code"),
        ("zed", "Zed"),
        ("cline", "Cline"),
        ("continue", "Continue"),
    ];
    if let Some((_, label)) = known.iter().find(|(key, _)| lower.contains(key)) {
        return label.to_string();
    }
    match title {
        Some(t) if !t.trim().is_empty() => t.trim().to_string(),
        _ if name.trim().is_empty() => "Unknown client".into(),
        _ => name.trim().to_string(),
    }
}

fn client_of(ctx: &RequestContext<RoleServer>) -> String {
    match ctx.peer.peer_info() {
        Some(info) => client_label(&info.client_info.name, info.client_info.title.as_deref()),
        None => "Unknown client".into(),
    }
}

/// Returns early with a tool error.
macro_rules! try_tool {
    ($e:expr) => {
        match $e {
            Ok(v) => v,
            Err(e) => return fail(e),
        }
    };
}

#[tool_router]
impl OwlTools {
    pub fn new(app: AppHandle) -> Self {
        let mut tool_router = Self::tool_router();
        if !app.state::<AppState>().settings.get().mcp.full_access {
            for name in FULL_ACCESS_TOOLS {
                tool_router.remove_route(name);
            }
        }
        Self { app, tool_router }
    }

    fn state(&self) -> tauri::State<'_, AppState> {
        self.app.state::<AppState>()
    }

    fn entry(
        &self,
        ctx: &RequestContext<RoleServer>,
        tool: &str,
        host: Option<&Host>,
        detail: impl Into<String>,
    ) -> AuditEntry {
        let mut entry = AuditEntry::new(tool, host.map(|h| h.name.as_str()), detail);
        entry.client = Some(client_of(ctx));
        entry
    }

    fn audit(&self, entry: AuditEntry) {
        self.state().audit.append(&entry);
        let _ = self.app.emit("mcp-activity", &entry);
    }

    /// Records the entry with its outcome and turns the result into a tool answer.
    fn finish(
        &self,
        mut entry: AuditEntry,
        result: Result<String, String>,
    ) -> Result<CallToolResult, ErrorData> {
        if result.is_err() {
            entry.outcome = "error".into();
        }
        self.audit(entry);
        match result {
            Ok(text) => ok(text),
            Err(e) => fail(e),
        }
    }

    /// Asks the user when risky actions need approval. False means denied (already logged).
    async fn approve(
        &self,
        entry: &mut AuditEntry,
        action: &str,
        detail: &str,
        risky: bool,
    ) -> bool {
        if !risky || !self.state().settings.get().mcp.confirm_dangerous {
            return true;
        }
        let client = entry.client.clone().unwrap_or_default();
        let server = entry.server.clone().unwrap_or_default();
        let approved = self
            .state()
            .confirmations
            .ask(&self.app, &client, &server, action, detail)
            .await;
        if !approved {
            entry.outcome = "denied".into();
            self.audit(entry.clone());
        }
        approved
    }

    fn full_access(&self) -> Result<(), String> {
        if self.state().settings.get().mcp.full_access {
            Ok(())
        } else {
            Err("Full access is turned off in OwlSSH settings.".into())
        }
    }

    fn find(&self, query: &str) -> Result<Host, String> {
        let q = query.trim();
        let hosts = self.state().hosts.list();
        hosts
            .iter()
            .find(|h| h.id == q)
            .or_else(|| hosts.iter().find(|h| h.name.eq_ignore_ascii_case(q)))
            .or_else(|| hosts.iter().find(|h| h.hostname.eq_ignore_ascii_case(q)))
            .cloned()
            .ok_or_else(|| format!("No server named \"{q}\". Call list_servers to see the names."))
    }

    async fn connect(&self, host: &Host) -> Result<Arc<SshConn>, String> {
        let state = self.state();
        let conn = state
            .pool
            .get_or_connect(&state.hosts, &host.id, &ConnectOptions::default())
            .await
            .map_err(|e| explain(&e))?;
        let _ = self.app.emit("host-connected", &host.id);
        Ok(conn)
    }

    async fn sftp(&self, host: &Host) -> Result<Arc<SftpSession>, String> {
        let conn = self.connect(host).await?;
        self.state()
            .sftp
            .session(&host.id, &conn)
            .await
            .map_err(|e| e.to_string())
    }

    #[tool(
        description = "List the servers saved in OwlSSH with their address, user, tags and whether they are connected right now."
    )]
    async fn list_servers(
        &self,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let state = self.state();
        let connected = state.pool.connected_ids();
        let list: Vec<_> = state
            .hosts
            .list()
            .into_iter()
            .map(|h| {
                serde_json::json!({
                    "name": h.name,
                    "id": h.id,
                    "address": format!("{}@{}:{}", h.username, h.hostname, h.port),
                    "tags": h.tags,
                    "connected": connected.contains(&h.id),
                })
            })
            .collect();
        self.audit(self.entry(&ctx, "list_servers", None, ""));
        ok(serde_json::to_string_pretty(&list).unwrap_or_default())
    }

    #[tool(
        description = "Open the SSH connection to a server using the credentials saved in OwlSSH."
    )]
    async fn connect_server(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(ServerArg { server }): Parameters<ServerArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let host = try_tool!(self.find(&server));
        let entry = self.entry(&ctx, "connect_server", Some(&host), "");
        let result = self
            .connect(&host)
            .await
            .map(|_| format!("Connected to {}.", host.name));
        self.finish(entry, result)
    }

    #[tool(
        description = "Run a shell command on a server and return stdout, stderr and the exit code. Commands that delete data, stop services or reboot wait for the user to approve them in OwlSSH."
    )]
    async fn run_command(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(args): Parameters<RunCommandArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let host = try_tool!(self.find(&args.server));
        let mut entry = self.entry(&ctx, "run_command", Some(&host), &args.command);
        let risky = guard::is_dangerous(&args.command);
        if !self
            .approve(&mut entry, "command", &args.command, risky)
            .await
        {
            return fail(
                "The user did not approve this command. Do not retry it without asking them.",
            );
        }
        let conn = match self.connect(&host).await {
            Ok(c) => c,
            Err(e) => return self.finish(entry, Err(e)),
        };
        let timeout = Duration::from_secs(args.timeout_secs.unwrap_or(60).clamp(1, 600));
        match exec::exec(&conn, &args.command, timeout).await {
            Ok(out) => {
                entry.exit_code = out.exit_code;
                let mut text = format!(
                    "exit code: {}\n",
                    out.exit_code.map_or("unknown".into(), |c| c.to_string())
                );
                if !out.stdout.is_empty() {
                    text.push_str(&format!("--- stdout ---\n{}\n", out.stdout));
                }
                if !out.stderr.is_empty() {
                    text.push_str(&format!("--- stderr ---\n{}\n", out.stderr));
                }
                if out.truncated {
                    text.push_str("(output was cut at 4 MB)\n");
                }
                self.finish(entry, Ok(text))
            }
            Err(e) => self.finish(entry, Err(e.to_string())),
        }
    }

    #[tool(
        description = "List a folder on a server: names, types, sizes, modification times and permissions."
    )]
    async fn list_directory(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(args): Parameters<PathArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let host = try_tool!(self.find(&args.server));
        let entry = self.entry(&ctx, "list_directory", Some(&host), &args.path);
        let result = async {
            let sftp = self.sftp(&host).await?;
            let listing = sftp::list_remote(&sftp, &args.path)
                .await
                .map_err(|e| e.to_string())?;
            Ok(serde_json::to_string_pretty(&listing).unwrap_or_default())
        }
        .await;
        self.finish(entry, result)
    }

    #[tool(description = "Read a text file from a server (up to 1 MB).")]
    async fn read_file(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(args): Parameters<PathArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let host = try_tool!(self.find(&args.server));
        let entry = self.entry(&ctx, "read_file", Some(&host), &args.path);
        let result = async {
            let sftp = self.sftp(&host).await?;
            let (text, cut) = read_text(&sftp, &args.path).await?;
            Ok(if cut {
                format!("{text}\n(file is longer than 1 MB, the rest was not read)")
            } else {
                text
            })
        }
        .await;
        self.finish(entry, result)
    }

    #[tool(
        description = "Create or replace a text file on a server. The user approves every write in OwlSSH."
    )]
    async fn write_file(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(args): Parameters<WriteFileArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let host = try_tool!(self.find(&args.server));
        let mut entry = self.entry(
            &ctx,
            "write_file",
            Some(&host),
            format!("{}\n\n{}", args.path, preview(&args.content)),
        );
        let detail = format!("{}\n\n{}", args.path, preview(&args.content));
        if !self.approve(&mut entry, "writeFile", &detail, true).await {
            return fail("The user did not approve writing this file.");
        }
        let result = async {
            let sftp = self.sftp(&host).await?;
            write_text(&sftp, &args.path, &args.content).await?;
            Ok(format!(
                "Wrote {} bytes to {}.",
                args.content.len(),
                args.path
            ))
        }
        .await;
        self.finish(entry, result)
    }

    #[tool(description = "Copy a file or folder from this computer to a server.")]
    async fn upload_file(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(args): Parameters<UploadArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let host = try_tool!(self.find(&args.server));
        let entry = self.entry(
            &ctx,
            "upload_file",
            Some(&host),
            format!("{} -> {}", args.local_path, args.remote_dir),
        );
        let result = self
            .transfer(&host, |sftp, events, cancel| {
                let sources = vec![Source::plain(args.local_path.clone())];
                let remote_dir = args.remote_dir.clone();
                async move { sftp::upload(&sftp, &sources, &remote_dir, 0, &cancel, &events).await }
            })
            .await;
        self.finish(entry, result)
    }

    #[tool(description = "Copy a file or folder from a server to this computer.")]
    async fn download_file(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(args): Parameters<DownloadArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let host = try_tool!(self.find(&args.server));
        let entry = self.entry(
            &ctx,
            "download_file",
            Some(&host),
            format!("{} -> {}", args.remote_path, args.local_dir),
        );
        let result = self
            .transfer(&host, |sftp, events, cancel| {
                let sources = vec![Source::plain(args.remote_path.clone())];
                let local_dir = args.local_dir.clone();
                async move { sftp::download(&sftp, &sources, &local_dir, 0, &cancel, &events).await }
            })
            .await;
        self.finish(entry, result)
    }

    #[tool(
        description = "Current CPU, memory, disk, uptime, load, top processes and Docker containers of a server."
    )]
    async fn server_metrics(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(ServerArg { server }): Parameters<ServerArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let host = try_tool!(self.find(&server));
        let entry = self.entry(&ctx, "server_metrics", Some(&host), "");
        let result = async {
            let conn = self.connect(&host).await?;
            let m = metrics::collect(&conn).await.map_err(|e| e.to_string())?;
            Ok(serde_json::to_string_pretty(&m).unwrap_or_default())
        }
        .await;
        self.finish(entry, result)
    }

    // ---- Full access ----

    #[tool(
        description = "Delete a file, or a folder with everything inside it. Full access mode only; the user may have to approve it."
    )]
    async fn delete_path(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(args): Parameters<PathArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        try_tool!(self.full_access());
        let host = try_tool!(self.find(&args.server));
        let mut entry = self.entry(&ctx, "delete_path", Some(&host), &args.path);
        let sftp = match self.sftp(&host).await {
            Ok(s) => s,
            Err(e) => return self.finish(entry, Err(e)),
        };
        if sftp.symlink_metadata(args.path.clone()).await.is_err() {
            return self.finish(entry, Err(format!("{} does not exist.", args.path)));
        }
        if !self.approve(&mut entry, "delete", &args.path, true).await {
            return fail("The user did not approve deleting this path.");
        }
        let result = sftp::remove_remote(&sftp, &args.path)
            .await
            .map(|_| format!("Deleted {}.", args.path))
            .map_err(|e| e.to_string());
        self.finish(entry, result)
    }

    #[tool(
        description = "Rename or move a file or folder on a server. Full access mode only; the user may have to approve it."
    )]
    async fn rename_path(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(args): Parameters<RenameArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        try_tool!(self.full_access());
        let host = try_tool!(self.find(&args.server));
        let detail = format!("{} -> {}", args.from, args.to);
        let mut entry = self.entry(&ctx, "rename_path", Some(&host), &detail);
        let sftp = match self.sftp(&host).await {
            Ok(s) => s,
            Err(e) => return self.finish(entry, Err(e)),
        };
        // Check what can be checked before bothering the user with a dialog.
        if sftp.symlink_metadata(args.from.clone()).await.is_err() {
            return self.finish(entry, Err(format!("{} does not exist.", args.from)));
        }
        if sftp.symlink_metadata(args.to.clone()).await.is_ok() {
            return self.finish(
                entry,
                Err(format!(
                    "{} already exists. Choose another name or delete it first.",
                    args.to
                )),
            );
        }
        if !self.approve(&mut entry, "rename", &detail, true).await {
            return fail("The user did not approve this rename.");
        }
        let result = sftp
            .rename(args.from.clone(), args.to.clone())
            .await
            .map(|_| format!("Renamed {} to {}.", args.from, args.to))
            .map_err(|e| e.to_string());
        self.finish(entry, result)
    }

    #[tool(
        description = "Create a folder (and missing parent folders) on a server. Full access mode only."
    )]
    async fn create_directory(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(args): Parameters<PathArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        try_tool!(self.full_access());
        let host = try_tool!(self.find(&args.server));
        let entry = self.entry(&ctx, "create_directory", Some(&host), &args.path);
        let result = async {
            let sftp = self.sftp(&host).await?;
            let mut current = if args.path.starts_with('/') {
                String::from("/")
            } else {
                String::new()
            };
            for part in args.path.split('/').filter(|p| !p.is_empty()) {
                current = if current.is_empty() {
                    part.to_string()
                } else {
                    sftp::remote_join(&current, part)
                };
                if sftp.symlink_metadata(current.clone()).await.is_err() {
                    sftp.create_dir(current.clone())
                        .await
                        .map_err(|e| e.to_string())?;
                }
            }
            Ok(format!("Folder {} is ready.", args.path))
        }
        .await;
        self.finish(entry, result)
    }

    #[tool(
        description = "Change part of a text file on a server by replacing exact text. Safer than rewriting the whole file. Full access mode only; the user may have to approve it."
    )]
    async fn edit_file(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(args): Parameters<EditFileArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        try_tool!(self.full_access());
        let host = try_tool!(self.find(&args.server));
        if args.old_text.is_empty() {
            return fail("old_text must not be empty.");
        }
        let detail = format!(
            "{}\n\n- {}\n+ {}",
            args.path,
            preview(&args.old_text),
            preview(&args.new_text)
        );
        let mut entry = self.entry(&ctx, "edit_file", Some(&host), &detail);

        let sftp = match self.sftp(&host).await {
            Ok(s) => s,
            Err(e) => return self.finish(entry, Err(e)),
        };
        let (text, cut) = match read_text(&sftp, &args.path).await {
            Ok(t) => t,
            Err(e) => return self.finish(entry, Err(e)),
        };
        if cut {
            return self.finish(
                entry,
                Err("The file is larger than 1 MB; edit it with run_command instead.".into()),
            );
        }
        let count = text.matches(&args.old_text).count();
        let replace_all = args.replace_all.unwrap_or(false);
        if count == 0 {
            return self.finish(entry, Err("old_text was not found in the file.".into()));
        }
        if count > 1 && !replace_all {
            return self.finish(
                entry,
                Err(format!(
                    "old_text appears {count} times. Add more surrounding text or set replace_all."
                )),
            );
        }
        if !self.approve(&mut entry, "edit", &detail, true).await {
            return fail("The user did not approve this edit.");
        }
        let updated = if replace_all {
            text.replace(&args.old_text, &args.new_text)
        } else {
            text.replacen(&args.old_text, &args.new_text, 1)
        };
        let result = write_text(&sftp, &args.path, &updated).await.map(|_| {
            format!(
                "Replaced {} occurrence(s) in {}.",
                if replace_all { count } else { 1 },
                args.path
            )
        });
        self.finish(entry, result)
    }
}

fn preview(text: &str) -> String {
    let mut p: String = text.chars().take(2000).collect();
    if p.len() < text.len() {
        p.push_str("\n…");
    }
    p
}

/// Reads up to 1 MB of a remote file as text. The flag says whether it was cut.
async fn read_text(sftp: &SftpSession, path: &str) -> Result<(String, bool), String> {
    let file = sftp
        .open(path.to_string())
        .await
        .map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    file.take(MAX_READ + 1)
        .read_to_end(&mut buf)
        .await
        .map_err(|e| e.to_string())?;
    let cut = buf.len() as u64 > MAX_READ;
    buf.truncate(MAX_READ as usize);
    Ok((String::from_utf8_lossy(&buf).into_owned(), cut))
}

async fn write_text(sftp: &SftpSession, path: &str, content: &str) -> Result<(), String> {
    let mut file = sftp
        .create(path.to_string())
        .await
        .map_err(|e| e.to_string())?;
    file.write_all(content.as_bytes())
        .await
        .map_err(|e| e.to_string())?;
    file.shutdown().await.map_err(|e| e.to_string())
}

impl OwlTools {
    /// Runs a transfer to the end and returns a summary, or the error message.
    async fn transfer<F, Fut>(&self, host: &Host, start: F) -> Result<String, String>
    where
        F: FnOnce(Arc<SftpSession>, Channel<TransferEvent>, Arc<AtomicBool>) -> Fut,
        Fut: std::future::Future<Output = ()>,
    {
        let sftp = self.sftp(host).await?;
        let outcome = Arc::new(Mutex::new(None::<Result<String, String>>));
        let totals = Arc::new(Mutex::new((0u64, 0u64)));
        let (sink, totals_sink) = (outcome.clone(), totals.clone());
        let events = Channel::new(move |body| {
            if let tauri::ipc::InvokeResponseBody::Json(json) = body {
                if let Ok(ev) = serde_json::from_str::<serde_json::Value>(&json) {
                    match ev["event"].as_str() {
                        Some("started") => {
                            *totals_sink.lock().unwrap() = (
                                ev["filesTotal"].as_u64().unwrap_or(0),
                                ev["bytesTotal"].as_u64().unwrap_or(0),
                            );
                        }
                        Some("finished") => {
                            let (files, bytes) = *totals_sink.lock().unwrap();
                            *sink.lock().unwrap() =
                                Some(Ok(format!("Copied {files} files, {bytes} bytes.")));
                        }
                        Some("failed") => {
                            *sink.lock().unwrap() = Some(Err(ev["message"]
                                .as_str()
                                .unwrap_or("transfer failed")
                                .to_string()));
                        }
                        _ => {}
                    }
                }
            }
            Ok(())
        });
        start(sftp, events, Arc::new(AtomicBool::new(false))).await;
        let result = outcome.lock().unwrap().take();
        result.unwrap_or_else(|| Err("transfer did not finish".into()))
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for OwlTools {
    fn get_info(&self) -> ServerConfig {
        let full = self.state().settings.get().mcp.full_access;
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("owlssh", env!("CARGO_PKG_VERSION")))
            .with_instructions(if full {
                "OwlSSH gives you the user's SSH servers. Start with list_servers. Full access \
                 is on: you can also delete, rename, create folders and edit files. Risky \
                 actions may wait for the user's approval in the OwlSSH window; a denied \
                 request must not be retried without asking the user."
            } else {
                "OwlSSH gives you the user's SSH servers. Start with list_servers. Prefer \
                 read-only commands; destructive commands and file writes wait for the user's \
                 approval in the OwlSSH window, and a denied request must not be retried \
                 without asking the user."
            })
    }
}

#[cfg(test)]
mod tests {
    use super::client_label;

    #[test]
    fn names_known_clients() {
        assert_eq!(client_label("claude-code", None), "Claude Code");
        assert_eq!(client_label("codex-mcp-client", None), "Codex");
        assert_eq!(client_label("Cursor", None), "Cursor");
        assert_eq!(client_label("my-tool", Some("My Tool")), "My Tool");
        assert_eq!(client_label("my-tool", None), "my-tool");
        assert_eq!(client_label("", None), "Unknown client");
    }
}
