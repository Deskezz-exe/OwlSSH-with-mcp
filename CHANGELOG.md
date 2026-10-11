# Changelog

All notable changes to OwlSSH. Versions follow [Semantic Versioning](https://semver.org).

## [0.1.0] - 2026-10-11

First public release.

### Terminal
- Tabs with real PTY sessions on xterm.js with the WebGL renderer
- Low input latency: `TCP_NODELAY` on the SSH socket, async input commands, raw byte output
- Copy and paste shortcuts you can change (`Ctrl+Shift+C` / `Ctrl+Shift+V` by default),
  matched by physical key so they work on any keyboard layout
- Copy on select, right click to paste or open a menu

### Servers
- Password (kept in the OS keychain), key file and SSH agent / Pageant sign-in
- Host key checks with SHA256 fingerprints for new and changed keys
- Jump hosts, tags, favorites, import from `~/.ssh/config`
- Live dashboard: CPU, RAM, disk, uptime, load, busiest processes, Docker containers

### Files
- Two-pane file manager with breadcrumbs, sorting and file type icons
- Drag and drop between panes or from the system file manager
- Folder transfers with progress and cancel
- Replace, rename or cancel when a name already exists at the destination

### AI assistants (MCP)
- MCP server on 127.0.0.1 with a bearer token
- Tools to list servers, run commands, read, write and copy files, read metrics
- Optional full access mode to delete, rename, create folders and edit files
- Risky actions wait for approval in the app
- Journal with the assistant name, status filters and pages

### Other
- Snippets with parameters, run on many servers at once
- `Ctrl+K` command palette
- English and Russian interface, dark and light themes, streamer mode
