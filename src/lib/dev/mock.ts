// Browser-only stand-in for the Rust backend, used by `npm run dev` outside
// Tauri so the UI can be developed and checked in a normal browser.
// Never bundled into the desktop app (see main.ts).
import { mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import type { Channel } from "@tauri-apps/api/core";
import type { Host, Settings } from "../api";

const hosts: Host[] = [
  host("1", "web-prod", "198.51.100.24", "deploy", ["prod", "web"], true),
  host("2", "db-primary", "198.51.100.31", "postgres", ["prod", "db"]),
  host("3", "staging", "staging.example.com", "ubuntu", ["staging"]),
  host("4", "home-nas", "192.168.1.20", "admin", ["home"]),
];

function host(id: string, name: string, hostname: string, username: string, tags: string[], favorite = false): Host {
  return {
    id,
    name,
    hostname,
    port: 22,
    username,
    auth: "password",
    keyPath: null,
    hasPassword: true,
    tags,
    favorite,
    jumpHostId: null,
    sortOrder: Number(id),
  };
}

let settings: Settings = {
  locale: "en",
  theme: "dark",
  terminal: { fontSize: 14, scrollback: 10000, cursorBlink: true, copyOnSelect: false, rightClick: "menu" },
  keybindings: { copy: "Ctrl+Shift+C", paste: "Ctrl+Shift+V" },
  streamerMode: false,
  mcp: { enabled: true, port: 47821, confirmDangerous: true, token: "3f9c2a7be14d4c0f9a51d6e2b8c07a44" },
};

const connected = new Set<string>();
const terminals = new Map<number, Channel<ArrayBuffer>>();
let nextTerminal = 1;
const encoder = new TextEncoder();
const send = (ch: Channel<ArrayBuffer>, text: string) =>
  ch.onmessage(encoder.encode(text).buffer as ArrayBuffer);
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

function listing(path: string, sep: string, dirs: string[], files: string[]) {
  const join = (name: string) => (path.endsWith(sep) ? path + name : path + sep + name);
  const now = Math.floor(Date.now() / 1000);
  return {
    path,
    entries: [
      ...dirs.map((name, i) => ({ name, path: join(name), kind: "dir", size: 0, modified: now - i * 86400, permissions: "rwxr-xr-x" })),
      ...files.map((name, i) => ({ name, path: join(name), kind: "file", size: 1500 * (i + 1) ** 5, modified: now - i * 3600, permissions: "rw-r--r--" })),
    ],
  };
}

export function installMock() {
  mockIPC(
    async (cmd, args) => {
      const a = (args ?? {}) as Record<string, any>;
      switch (cmd) {
        case "list_hosts":
          return hosts;
        case "connected_hosts":
          return [...connected];
        case "save_host": {
          const input = a.input;
          const existing = hosts.find((h) => h.id === input.id);
          const saved: Host = {
            ...(existing ?? host(String(Date.now()), "", "", "", [])),
            ...input,
            id: existing?.id ?? String(Date.now()),
            hasPassword: input.password ? true : (existing?.hasPassword ?? false),
          };
          if (existing) Object.assign(existing, saved);
          else hosts.push(saved);
          return saved;
        }
        case "delete_host":
          hosts.splice(hosts.findIndex((h) => h.id === a.id), 1);
          return null;
        case "set_favorite": {
          const h = hosts.find((h) => h.id === a.id)!;
          h.favorite = a.favorite;
          return h;
        }
        case "reorder_hosts":
          a.ids.forEach((id: string, i: number) => {
            const h = hosts.find((h) => h.id === id);
            if (h) h.sortOrder = i;
          });
          return null;
        case "preview_ssh_config":
          return [
            { alias: "bastion", hostname: "203.0.113.5", port: 22, user: "ops", identityFile: "~/.ssh/id_ed25519", proxyJump: null, alreadyAdded: false },
            { alias: "web-prod", hostname: "198.51.100.24", port: 22, user: "deploy", identityFile: null, proxyJump: null, alreadyAdded: true },
          ];
        case "import_ssh_config":
          return [];
        case "connect_host":
          await sleep(700);
          if (a.hostId === "3" && !a.options?.trustFingerprint) {
            throw {
              kind: "hostKeyUnknown",
              message: "unknown host key",
              details: { hostId: "3", hostname: "staging.example.com", port: 22, fingerprint: "SHA256:q2Yd0kN5mX8bVwz3oP1cR7tLhE4jU6sFa9GiKnB0", algorithm: "ssh-ed25519" },
            };
          }
          connected.add(a.hostId);
          return null;
        case "disconnect_host":
          connected.delete(a.hostId);
          return null;
        case "host_metrics": {
          if (!connected.has(a.hostId)) return null;
          await sleep(450);
          const r = (min: number, max: number) => min + Math.random() * (max - min);
          const busy = a.hostId === "2";
          return {
            cpuPercent: busy ? r(70, 96) : r(3, 35),
            cpus: busy ? 8 : 2,
            load: [r(0.1, 2), r(0.1, 2), r(0.1, 2)],
            memTotalKb: 8_000_000,
            memUsedKb: busy ? 6_900_000 : r(1_500_000, 3_500_000),
            swapTotalKb: 2_000_000,
            swapUsedKb: 0,
            diskTotalKb: 80_000_000,
            diskUsedKb: busy ? 61_000_000 : 23_000_000,
            uptimeSecs: 1_234_567,
            os: "Ubuntu 24.04.1 LTS",
            processes: [
              { pid: 1234, cpu: 31.5, mem: 4.1, name: busy ? "postgres" : "node" },
              { pid: 812, cpu: 6.2, mem: 10.4, name: "dockerd" },
              { pid: 77, cpu: 1.1, mem: 0.6, name: "sshd" },
            ],
            docker: a.hostId === "1" ? { running: 4, total: 6 } : null,
          };
        }
        case "terminal_open": {
          const id = nextTerminal++;
          terminals.set(id, a.output);
          const h = hosts.find((h) => h.id === a.hostId)!;
          setTimeout(() =>
            send(a.output, `Welcome to Ubuntu 24.04 LTS\r\n\r\n\x1b[32m${h.username}@${h.name}\x1b[0m:\x1b[34m~\x1b[0m$ `),
          );
          return id;
        }
        case "terminal_write": {
          const ch = terminals.get(a.id);
          if (ch) send(ch, a.data === "\r" ? "\r\n$ " : a.data);
          return null;
        }
        case "terminal_resize":
          return null;
        case "terminal_close":
          terminals.delete(a.id);
          await emit("terminal-exit", { id: a.id, exitCode: 0 });
          return null;
        case "local_roots":
          return ["C:\\", "D:\\"];
        case "local_list":
          return listing(a.path ?? String.raw`C:\Users\dev`,"\\", ["Documents", "Downloads", "projects"], ["notes.txt", "deploy.sh", "backup.tar.gz"]);
        case "sftp_list":
          return listing(a.path && a.path !== "." ? a.path : "/home/deploy", "/", [".ssh", "app", "logs"], [".bashrc", "docker-compose.yml", "README.md"]);
        case "sftp_upload":
        case "sftp_download": {
          const job = Date.now() % 100000;
          const ch = a.events;
          const total = 48_000_000;
          ch.onmessage({ event: "started", job, filesTotal: 3, bytesTotal: total });
          let done = 0;
          const timer = setInterval(() => {
            done = Math.min(total, done + 6_000_000);
            ch.onmessage({ event: "progress", job, file: "backup.tar.gz", filesDone: Math.floor((done / total) * 3), bytesDone: done });
            if (done >= total) {
              clearInterval(timer);
              ch.onmessage({ event: "finished", job });
            }
          }, 300);
          return job;
        }
        case "mcp_status":
          return { running: settings.mcp.enabled, port: settings.mcp.port, url: `http://127.0.0.1:${settings.mcp.port}/mcp`, error: null };
        case "mcp_activity":
          return [
            { time: Date.now() - 60000, tool: "run_command", server: "web-prod", detail: "docker ps --format '{{.Names}}'", outcome: "ok", exitCode: 0 },
            { time: Date.now() - 50000, tool: "run_command", server: "web-prod", detail: "rm -rf /var/log/old", outcome: "denied", exitCode: null },
            { time: Date.now() - 40000, tool: "read_file", server: "db-primary", detail: "/etc/postgresql/16/main/postgresql.conf", outcome: "ok", exitCode: null },
          ];
        case "mcp_confirm_answer":
          return null;
        case "list_snippets":
          return [
            { id: "s1", name: "Disk usage", command: "df -h", description: "" },
            { id: "s2", name: "Restart a service", command: "sudo systemctl restart {{service}}", description: "" },
          ];
        case "save_snippet":
          return { ...a.input, id: a.input.id ?? String(Date.now()) };
        case "delete_snippet":
          return null;
        case "broadcast_command":
          for (const id of a.hostIds) {
            setTimeout(() => a.results.onmessage({ hostId: id, stdout: "Filesystem  Size  Used Avail Use% Mounted on\n/dev/vda1    40G   12G   26G  32% /", stderr: "", exitCode: 0, error: null }), 400 + Math.random() * 900);
          }
          return null;
        case "get_settings":
          return settings;
        case "save_settings":
          settings = a.settings;
          return settings;
        default:
          console.warn("mock: unhandled command", cmd, a);
          return null;
      }
    },
    { shouldMockEvents: true },
  );
}
