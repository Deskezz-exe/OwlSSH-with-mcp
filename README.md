<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="assets/logo-light.png">
  <img src="assets/logo-dark.png" alt="OwlSSH logo" width="140">
</picture>

# OwlSSH

**A fast desktop SSH client with a live server dashboard, a two-pane SFTP manager and a built-in MCP server, so your AI assistant can work on the same servers you do.**

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
![Tauri 2](https://img.shields.io/badge/Tauri-2-24C8DB?logo=tauri&logoColor=white)
![Rust](https://img.shields.io/badge/Rust-stable-orange?logo=rust)
![Svelte 5](https://img.shields.io/badge/Svelte-5-FF3E00?logo=svelte&logoColor=white)

</div>

---

> Work in progress. The roadmap below shows what already works and what is next.

## Why

Most SSH clients are either heavy Electron apps or bare terminals. OwlSSH aims for the middle:
a native, lightweight app (Rust + Tauri) that keeps the terminal snappy, shows the health of
every server at a glance, and lets AI assistants such as Claude act on your servers through the
[Model Context Protocol](https://modelcontextprotocol.io) with every command logged and
dangerous ones waiting for your confirmation.

## Roadmap

- [ ] Server profiles, passwords in the OS keychain, `~/.ssh/config` import
- [ ] SSH engine on `russh`: host key pinning, connection pool, keys and agents
- [ ] Low-latency terminal tabs (xterm.js + WebGL, binary streaming)
- [ ] Rebindable copy/paste (default `Ctrl+Shift+C` / `Ctrl+Shift+V`), copy on select
- [ ] Live dashboard: CPU, RAM, disk, uptime, top processes, Docker containers
- [ ] Two-pane SFTP with drag and drop, multi-select and progress
- [ ] English and Russian UI, switchable in settings
- [ ] Dark and light themes
- [ ] MCP server: run commands, browse and transfer files, audit log
- [ ] Snippets with parameters, broadcast to many hosts
- [ ] Quick search (`Ctrl+K`), streamer mode that hides real IPs
- [ ] One-click SSH key setup, port forwarding, jump hosts

## Tech stack

| Layer | Tools |
|---|---|
| Desktop shell | Tauri 2 |
| Backend | Rust, Tokio, russh, russh-sftp |
| Frontend | Svelte 5, TypeScript, Vite, xterm.js |
| AI integration | MCP over streamable HTTP (rmcp) |

## Development

Requirements: Rust (stable), Node.js 20+, and the
[Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your OS.

```bash
npm install
npm run tauri dev
```

Release build:

```bash
npm run tauri build
```

## License

[MIT](LICENSE)
