//! The tools an AI assistant sees. They run on the same pooled connections
//! as the app, so the assistant works on exactly the servers the user has.

use std::sync::atomic::AtomicBool;
use std::time::Duration;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerConfig};
use rmcp::{schemars, tool, tool_handler, tool_router, ErrorData, ServerHandler};
use serde::Deserialize;
use tauri::ipc::Channel;
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::audit::AuditEntry;
use super::guard;
use crate::error::AppError;
use crate::hosts::Host;
use crate::ssh::sftp::{self, TransferEvent};
use crate::ssh::{exec, metrics, ConnectOptions, SshConn};
use crate::state::AppState;

const MAX_READ: u64 = 1024 * 1024;

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
        AppError::HostKeyUnknown { .. } | AppError::HostKeyChanged { .. } => format!(
            "{e}. The user has to confirm the server key in OwlSSH first."
        ),
        _ => e.to_string(),
    }
}

#[tool_router]
impl OwlTools {
    pub fn new(app: AppHandle) -> Self {
        Self {
            app,
            tool_router: Self::tool_router(),
        }
    }

    fn state(&self) -> tauri::State<'_, AppState> {
        self.app.state::<AppState>()
    }

    fn audit(&self, entry: AuditEntry) {
        self.state().audit.append(&entry);
        let _ = self.app.emit("mcp-activity", &entry);
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

    async fn connect(&self, host: &Host) -> Result<std::sync::Arc<SshConn>, String> {
        let state = self.state();
        let conn = state
            .pool
            .get_or_connect(&state.hosts, &host.id, &ConnectOptions::default())
            .await
            .map_err(|e| explain(&e))?;
        let _ = self.app.emit("host-connected", &host.id);
        Ok(conn)
    }

    #[tool(
        description = "List the servers saved in OwlSSH with their address, user, tags and whether they are connected right now."
    )]
    async fn list_servers(&self) -> Result<CallToolResult, ErrorData> {
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
        self.audit(AuditEntry::new("list_servers", None, ""));
        ok(serde_json::to_string_pretty(&list).unwrap_or_default())
    }

    #[tool(
        description = "Open the SSH connection to a server using the credentials saved in OwlSSH."
    )]
    async fn connect_server(
        &self,
        Parameters(ServerArg { server }): Parameters<ServerArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let host = match self.find(&server) {
            Ok(h) => h,
            Err(e) => return fail(e),
        };
        let mut entry = AuditEntry::new("connect_server", Some(&host.name), "");
        let result = self.connect(&host).await;
        if result.is_err() {
            entry.outcome = "error".into();
        }
        self.audit(entry);
        match result {
            Ok(_) => ok(format!("Connected to {}.", host.name)),
            Err(e) => fail(e),
        }
    }

    #[tool(
        description = "Run a shell command on a server and return stdout, stderr and the exit code. Commands that delete data, stop services or reboot wait for the user to approve them in OwlSSH."
    )]
    async fn run_command(
        &self,
        Parameters(args): Parameters<RunCommandArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let host = match self.find(&args.server) {
            Ok(h) => h,
            Err(e) => return fail(e),
        };
        let mut entry = AuditEntry::new("run_command", Some(&host.name), &args.command);

        if self.state().settings.get().mcp.confirm_dangerous && guard::is_dangerous(&args.command) {
            let approved = self
                .state()
                .confirmations
                .ask(&self.app, &host.name, "command", &args.command)
                .await;
            if !approved {
                entry.outcome = "denied".into();
                self.audit(entry);
                return fail(
                    "The user did not approve this command. Do not retry it without asking them.",
                );
            }
        }

        let conn = match self.connect(&host).await {
            Ok(c) => c,
            Err(e) => {
                entry.outcome = "error".into();
                self.audit(entry);
                return fail(e);
            }
        };
        let timeout = Duration::from_secs(args.timeout_secs.unwrap_or(60).clamp(1, 600));
        match exec::exec(&conn, &args.command, timeout).await {
            Ok(out) => {
                entry.exit_code = out.exit_code;
                self.audit(entry);
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
                ok(text)
            }
            Err(e) => {
                entry.outcome = "error".into();
                self.audit(entry);
                fail(e.to_string())
            }
        }
    }

    #[tool(
        description = "List a folder on a server: names, types, sizes, modification times and permissions."
    )]
    async fn list_directory(
        &self,
        Parameters(args): Parameters<PathArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let host = match self.find(&args.server) {
            Ok(h) => h,
            Err(e) => return fail(e),
        };
        self.audit(AuditEntry::new(
            "list_directory",
            Some(&host.name),
            &args.path,
        ));
        let result = async {
            let conn = self.connect(&host).await?;
            let sftp = self
                .state()
                .sftp
                .session(&host.id, &conn)
                .await
                .map_err(|e| e.to_string())?;
            sftp::list_remote(&sftp, &args.path)
                .await
                .map_err(|e| e.to_string())
        }
        .await;
        match result {
            Ok(listing) => ok(serde_json::to_string_pretty(&listing).unwrap_or_default()),
            Err(e) => fail(e),
        }
    }

    #[tool(description = "Read a text file from a server (up to 1 MB).")]
    async fn read_file(
        &self,
        Parameters(args): Parameters<PathArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let host = match self.find(&args.server) {
            Ok(h) => h,
            Err(e) => return fail(e),
        };
        self.audit(AuditEntry::new("read_file", Some(&host.name), &args.path));
        let result = async {
            let conn = self.connect(&host).await?;
            let sftp = self
                .state()
                .sftp
                .session(&host.id, &conn)
                .await
                .map_err(|e| e.to_string())?;
            let file = sftp
                .open(args.path.clone())
                .await
                .map_err(|e| e.to_string())?;
            let mut buf = Vec::new();
            file.take(MAX_READ + 1)
                .read_to_end(&mut buf)
                .await
                .map_err(|e| e.to_string())?;
            let cut = buf.len() as u64 > MAX_READ;
            buf.truncate(MAX_READ as usize);
            let mut text = String::from_utf8_lossy(&buf).into_owned();
            if cut {
                text.push_str("\n(file is longer than 1 MB, the rest was not read)");
            }
            Ok::<_, String>(text)
        }
        .await;
        match result {
            Ok(text) => ok(text),
            Err(e) => fail(e),
        }
    }

    #[tool(
        description = "Create or replace a text file on a server. The user approves every write in OwlSSH."
    )]
    async fn write_file(
        &self,
        Parameters(args): Parameters<WriteFileArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let host = match self.find(&args.server) {
            Ok(h) => h,
            Err(e) => return fail(e),
        };
        let mut entry = AuditEntry::new(
            "write_file",
            Some(&host.name),
            format!("{} ({} bytes)", args.path, args.content.len()),
        );
        if self.state().settings.get().mcp.confirm_dangerous {
            let preview: String = args.content.chars().take(2000).collect();
            let detail = format!("{}\n\n{}", args.path, preview);
            if !self
                .state()
                .confirmations
                .ask(&self.app, &host.name, "writeFile", &detail)
                .await
            {
                entry.outcome = "denied".into();
                self.audit(entry);
                return fail("The user did not approve writing this file.");
            }
        }
        let result = async {
            let conn = self.connect(&host).await?;
            let sftp = self
                .state()
                .sftp
                .session(&host.id, &conn)
                .await
                .map_err(|e| e.to_string())?;
            let mut file = sftp
                .create(args.path.clone())
                .await
                .map_err(|e| e.to_string())?;
            file.write_all(args.content.as_bytes())
                .await
                .map_err(|e| e.to_string())?;
            file.shutdown().await.map_err(|e| e.to_string())
        }
        .await;
        if result.is_err() {
            entry.outcome = "error".into();
        }
        self.audit(entry);
        match result {
            Ok(()) => ok(format!(
                "Wrote {} bytes to {}.",
                args.content.len(),
                args.path
            )),
            Err(e) => fail(e),
        }
    }

    #[tool(description = "Copy a file or folder from this computer to a server.")]
    async fn upload_file(
        &self,
        Parameters(args): Parameters<UploadArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let host = match self.find(&args.server) {
            Ok(h) => h,
            Err(e) => return fail(e),
        };
        let detail = format!("{} -> {}", args.local_path, args.remote_dir);
        let result = self
            .transfer(&host, |sftp, events, cancel| {
                let local = vec![args.local_path.clone()];
                let remote_dir = args.remote_dir.clone();
                async move { sftp::upload(&sftp, &local, &remote_dir, 0, &cancel, &events).await }
            })
            .await;
        self.finish_transfer("upload_file", &host, detail, result)
    }

    #[tool(description = "Copy a file or folder from a server to this computer.")]
    async fn download_file(
        &self,
        Parameters(args): Parameters<DownloadArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let host = match self.find(&args.server) {
            Ok(h) => h,
            Err(e) => return fail(e),
        };
        let detail = format!("{} -> {}", args.remote_path, args.local_dir);
        let result = self
            .transfer(&host, |sftp, events, cancel| {
                let remote = vec![args.remote_path.clone()];
                let local_dir = args.local_dir.clone();
                async move { sftp::download(&sftp, &remote, &local_dir, 0, &cancel, &events).await }
            })
            .await;
        self.finish_transfer("download_file", &host, detail, result)
    }

    #[tool(
        description = "Current CPU, memory, disk, uptime, load, top processes and Docker containers of a server."
    )]
    async fn server_metrics(
        &self,
        Parameters(ServerArg { server }): Parameters<ServerArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let host = match self.find(&server) {
            Ok(h) => h,
            Err(e) => return fail(e),
        };
        self.audit(AuditEntry::new("server_metrics", Some(&host.name), ""));
        let result = async {
            let conn = self.connect(&host).await?;
            metrics::collect(&conn).await.map_err(|e| e.to_string())
        }
        .await;
        match result {
            Ok(m) => ok(serde_json::to_string_pretty(&m).unwrap_or_default()),
            Err(e) => fail(e),
        }
    }
}

impl OwlTools {
    /// Runs a transfer to the end and returns a summary, or the error message.
    async fn transfer<F, Fut>(&self, host: &Host, start: F) -> Result<String, String>
    where
        F: FnOnce(
            std::sync::Arc<russh_sftp::client::SftpSession>,
            Channel<TransferEvent>,
            std::sync::Arc<AtomicBool>,
        ) -> Fut,
        Fut: std::future::Future<Output = ()>,
    {
        let conn = self.connect(host).await?;
        let sftp = self
            .state()
            .sftp
            .session(&host.id, &conn)
            .await
            .map_err(|e| e.to_string())?;
        let outcome = std::sync::Arc::new(std::sync::Mutex::new(None::<Result<String, String>>));
        let sink = outcome.clone();
        let totals = std::sync::Arc::new(std::sync::Mutex::new((0u32, 0u64)));
        let totals_sink = totals.clone();
        let events = Channel::new(move |body| {
            if let tauri::ipc::InvokeResponseBody::Json(json) = body {
                if let Ok(ev) = serde_json::from_str::<serde_json::Value>(&json) {
                    match ev["event"].as_str() {
                        Some("started") => {
                            *totals_sink.lock().unwrap() = (
                                ev["filesTotal"].as_u64().unwrap_or(0) as u32,
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
        start(sftp, events, std::sync::Arc::new(AtomicBool::new(false))).await;
        let result = outcome.lock().unwrap().take();
        result.unwrap_or_else(|| Err("transfer did not finish".into()))
    }

    fn finish_transfer(
        &self,
        tool: &str,
        host: &Host,
        detail: String,
        result: Result<String, String>,
    ) -> Result<CallToolResult, ErrorData> {
        let mut entry = AuditEntry::new(tool, Some(&host.name), detail);
        if result.is_err() {
            entry.outcome = "error".into();
        }
        self.audit(entry);
        match result {
            Ok(summary) => ok(summary),
            Err(e) => fail(e),
        }
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for OwlTools {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("owlssh", env!("CARGO_PKG_VERSION")))
            .with_instructions(
                "OwlSSH gives you the user's SSH servers. Start with list_servers. \
                 Prefer read-only commands; destructive commands and file writes wait \
                 for the user's approval in the OwlSSH window, and a denied request \
                 must not be retried without asking the user.",
            )
    }
}
