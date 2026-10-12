# Changelog

All notable changes to OwlSSH. Versions follow [Semantic Versioning](https://semver.org).

## [0.2.0] - 2026-10-12

Port forwarding, one-click key login, a tray icon and a much more capable terminal.

- Tunnels: forward a local port to anything the server can reach, or a server port back
  to this computer, and start or stop them from their own screen
- Set up key login on a server in one click: OwlSSH installs your key, checks it and
  switches the server over
- Keeps running in the tray when the window is closed, so sessions and the MCP server stay up;
  a second launch opens the running window
- Terminal: find in output (`Ctrl+Shift+F`), `Ctrl+click` links, font zoom with `Ctrl+wheel`,
  any installed font, Unicode 11 widths for emoji and CJK text
- Terminal: reconnects by itself when the connection drops and keeps the old output
- Terminal: the shell title on the tab, a mark on tabs that rang the bell, a question before
  closing a running session, save output to a file

## [0.1.0] - 2026-10-11

First public release: a desktop SSH client with a live server dashboard, a two-pane
file manager and a built-in MCP server for AI assistants.

- Low-latency terminal tabs; copy and paste shortcuts that work on any keyboard layout
- Live CPU, RAM and disk load for every connected server
- File transfers with drag and drop, progress, and replace or rename on name conflicts
- MCP server with approval for risky actions, an optional full access mode and a journal
- English and Russian interface, dark and light themes

The full feature list is in the [README](https://github.com/Deskezz-exe/OwlSSH-with-mcp#features).
