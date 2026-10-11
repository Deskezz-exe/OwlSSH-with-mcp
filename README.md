<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="assets/logo-light.png">
  <img src="assets/logo-dark.png" alt="OwlSSH logo" width="140">
</picture>

# OwlSSH

**A fast desktop SSH client with a live server dashboard, a two-pane file manager and a built-in MCP server, so your AI assistant can work on the same servers you do.**

[![CI](https://github.com/Deskezz-exe/OwlSSH-with-mcp/actions/workflows/ci.yml/badge.svg)](https://github.com/Deskezz-exe/OwlSSH-with-mcp/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
![Tauri 2](https://img.shields.io/badge/Tauri-2-24C8DB?logo=tauri&logoColor=white)
![Rust](https://img.shields.io/badge/Rust-stable-orange?logo=rust)
![Svelte 5](https://img.shields.io/badge/Svelte-5-FF3E00?logo=svelte&logoColor=white)

English interface by default, Russian available in settings.

</div>

---

## Features

**Terminal**
- Tabs with real PTY sessions, xterm.js with the WebGL renderer
- Low input latency: `TCP_NODELAY` on the SSH socket, keystrokes sent from async
  commands that never block the UI thread, output streamed as raw bytes and
  merged only when packets are already waiting
- Copy and paste shortcuts you can change in settings (`Ctrl+Shift+C` /
  `Ctrl+Shift+V` by default). They match the physical key, so they keep working
  on a Russian or any other keyboard layout
- Copy on select, right click to paste or open a menu, bracketed paste

**Dashboard**
- A card per server with live CPU, RAM and disk bars, uptime, load, OS,
  top processes and Docker containers
- One POSIX probe per refresh over the existing connection; nothing to install
  on the server
- Favorites, tags, search, jump hosts

**Files**
- Two panes: this computer and the server
- Drag files between panes or straight from Explorer / Finder onto the server
- Folders transfer recursively, with progress and cancel
- New folder, rename, delete with confirmation, keyboard shortcuts

**Snippets and search**
- Saved commands with `{{placeholders}}`, run on one server or many in parallel
- `Ctrl+K` palette for servers, open tabs, screens and snippets

**Security**
- Passwords live in the OS keychain (Windows Credential Manager, macOS Keychain,
  Secret Service) and never reach the UI
- Host keys checked against `~/.ssh/known_hosts` and an app known_hosts file;
  new and changed keys are shown with their SHA256 fingerprint before you trust them
- Password, key file (with passphrase prompt) and SSH agent / Pageant auth
- Streamer mode replaces real addresses on screen

## AI assistants (MCP)

OwlSSH runs a [Model Context Protocol](https://modelcontextprotocol.io) server on
`127.0.0.1`. Claude Code, Claude Desktop, Cursor and other MCP clients can then use
your servers through the connections the app already has.

| Tool | What it does |
|---|---|
| `list_servers` | Saved servers, their address, tags and connection state |
| `connect_server` | Connects with the credentials saved in OwlSSH |
| `run_command` | Runs a command, returns stdout, stderr and the exit code |
| `list_directory` | Lists a folder with sizes, dates and permissions |
| `read_file` | Reads a text file (up to 1 MB) |
| `write_file` | Creates or replaces a file, always after your approval |
| `upload_file` / `download_file` | Copies files or folders between this computer and a server |
| `server_metrics` | CPU, memory, disk, load, top processes, Docker |

Safety model:
- The endpoint listens on loopback only and requires a bearer token generated on first start.
- Commands that delete data, stop services, reboot or wipe disks, and every file write,
  wait in an approval dialog in the app. The assistant cannot approve its own request,
  and a request nobody answers is denied after three minutes.
- There is no delete tool at all.
- Every call is written to an audit log you can read on the *AI assistants* screen.

Connect Claude Code with the command shown on that screen:

```bash
claude mcp add --transport http owlssh http://127.0.0.1:47821/mcp --header "Authorization: Bearer <token>"
```

## Tech stack

| Layer | Tools |
|---|---|
| Desktop shell | Tauri 2 |
| Backend | Rust, Tokio, russh, russh-sftp, keyring |
| AI integration | rmcp (streamable HTTP), axum |
| Frontend | Svelte 5, TypeScript, Vite, xterm.js |

The Rust side has unit tests and end-to-end tests that start an in-process SSH
server to check host key trust, password auth, jump hosts, exec and the PTY path.

## Development

Requirements: Rust (stable), Node.js 20+, and the
[Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your OS.

```bash
npm install
npm run tauri dev
```

`npm run dev` alone starts the UI in a browser with a mocked backend, which is handy
for working on the interface.

Checks and release build:

```bash
npm run check
cd src-tauri && cargo test && cargo clippy --all-targets -- -D warnings
npm run tauri build
```

## License

[MIT](LICENSE)
