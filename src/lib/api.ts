// Typed wrappers around the Rust commands. Shapes mirror the serde structs.
import { invoke, Channel } from "@tauri-apps/api/core";
import { t, type MessageKey } from "./i18n/index.svelte";

export type AuthKind = "password" | "key" | "agent";

export interface Host {
  id: string;
  name: string;
  hostname: string;
  port: number;
  username: string;
  auth: AuthKind;
  keyPath: string | null;
  hasPassword: boolean;
  tags: string[];
  favorite: boolean;
  jumpHostId: string | null;
  sortOrder: number;
}

export interface HostInput {
  id: string | null;
  name: string;
  hostname: string;
  port: number;
  username: string;
  auth: AuthKind;
  keyPath: string | null;
  /** null keeps the stored password, "" removes it. */
  password: string | null;
  tags: string[];
  favorite: boolean;
  jumpHostId: string | null;
}

export interface ImportCandidate {
  alias: string;
  hostname: string;
  port: number;
  user: string | null;
  identityFile: string | null;
  proxyJump: string | null;
  alreadyAdded: boolean;
}

export interface ConnectOptions {
  password?: string;
  savePassword?: boolean;
  passphrase?: string;
  trustFingerprint?: string;
}

export interface ExecOutput {
  stdout: string;
  stderr: string;
  exitCode: number | null;
  truncated: boolean;
}

export interface Settings {
  locale: string;
  theme: "dark" | "light" | "system";
  terminal: {
    fontSize: number;
    /** Empty: the bundled JetBrains Mono. */
    fontFamily: string;
    scrollback: number;
    cursorBlink: boolean;
    copyOnSelect: boolean;
    confirmClose: boolean;
    autoReconnect: boolean;
    rightClick: "menu" | "paste";
  };
  keybindings: { copy: string; paste: string };
  streamerMode: boolean;
  mcp: { enabled: boolean; port: number; confirmDangerous: boolean; fullAccess: boolean; token: string };
}

export interface Metrics {
  cpuPercent: number | null;
  cpus: number | null;
  load: [number, number, number] | null;
  memTotalKb: number | null;
  memUsedKb: number | null;
  swapTotalKb: number | null;
  swapUsedKb: number | null;
  diskTotalKb: number | null;
  diskUsedKb: number | null;
  uptimeSecs: number | null;
  os: string | null;
  processes: { pid: number; cpu: number; mem: number; name: string }[];
  docker: { running: number; total: number } | null;
}

export type EntryKind = "dir" | "file" | "symlink" | "other";

export interface Entry {
  name: string;
  path: string;
  kind: EntryKind;
  size: number;
  modified: number | null;
  permissions: string | null;
}

export interface Listing {
  path: string;
  entries: Entry[];
}

/** A file or folder to copy; `name` renames it at the destination. */
export interface TransferSource {
  path: string;
  name?: string | null;
}

export type TransferEvent =
  | { event: "started"; job: number; filesTotal: number; bytesTotal: number }
  | { event: "progress"; job: number; file: string; filesDone: number; bytesDone: number }
  | { event: "finished"; job: number }
  | { event: "failed"; job: number; message: string }
  | { event: "cancelled"; job: number };

export interface McpStatus {
  running: boolean;
  port: number;
  url: string;
  error: string | null;
}

export interface AuditEntry {
  time: number;
  tool: string;
  client: string | null;
  server: string | null;
  detail: string;
  outcome: "ok" | "error" | "denied";
  exitCode: number | null;
}

export type AuditOutcome = "ok" | "error" | "denied";

export interface AuditPage {
  entries: AuditEntry[];
  total: number;
  clients: string[];
}

export interface ConfirmRequest {
  id: number;
  client: string;
  server: string;
  action: "command" | "writeFile" | "delete" | "rename" | "edit";
  detail: string;
}

export interface Snippet {
  id: string;
  name: string;
  command: string;
  description: string;
}

export interface HostRun {
  hostId: string;
  stdout: string;
  stderr: string;
  exitCode: number | null;
  error: string | null;
}

export interface TerminalExit {
  id: number;
  exitCode: number | null;
  /** The connection dropped; reconnecting makes sense. */
  lost: boolean;
}

/** The error object every command rejects with. */
export interface AppError {
  kind: string;
  message: string;
  details: Record<string, unknown> | null;
}

export function isAppError(e: unknown): e is AppError {
  return typeof e === "object" && e !== null && "kind" in e && "message" in e;
}

/** A readable, translated description of any error a command threw. */
export function errorText(e: unknown): string {
  if (isAppError(e)) {
    const key = `error.${e.kind}` as MessageKey;
    const title = t(key) === key ? t("error.unknown") : t(key);
    return e.message ? `${title}: ${e.message}` : title;
  }
  return e instanceof Error ? e.message : String(e);
}

export const api = {
  listHosts: () => invoke<Host[]>("list_hosts"),
  saveHost: (input: HostInput) => invoke<Host>("save_host", { input }),
  deleteHost: (id: string) => invoke<void>("delete_host", { id }),
  setFavorite: (id: string, favorite: boolean) => invoke<Host>("set_favorite", { id, favorite }),
  reorderHosts: (ids: string[]) => invoke<void>("reorder_hosts", { ids }),
  previewSshConfig: () => invoke<ImportCandidate[]>("preview_ssh_config"),
  importSshConfig: (aliases: string[]) => invoke<Host[]>("import_ssh_config", { aliases }),

  connectHost: (hostId: string, options?: ConnectOptions) =>
    invoke<void>("connect_host", { hostId, options: options ?? null }),
  disconnectHost: (hostId: string) => invoke<void>("disconnect_host", { hostId }),
  connectedHosts: () => invoke<string[]>("connected_hosts"),
  runCommand: (hostId: string, command: string) =>
    invoke<ExecOutput>("run_command", { hostId, command }),
  hostMetrics: (hostId: string) => invoke<Metrics | null>("host_metrics", { hostId }),

  terminalOpen: (hostId: string, cols: number, rows: number, output: Channel<ArrayBuffer>) =>
    invoke<number>("terminal_open", { hostId, cols, rows, output }),
  terminalWrite: (id: number, data: string) => invoke<void>("terminal_write", { id, data }),
  terminalResize: (id: number, cols: number, rows: number) =>
    invoke<void>("terminal_resize", { id, cols, rows }),
  terminalClose: (id: number) => invoke<void>("terminal_close", { id }),
  saveTerminalOutput: (path: string, text: string) => invoke<void>("save_terminal_output", { path, text }),

  sftpList: (hostId: string, path?: string) => invoke<Listing>("sftp_list", { hostId, path: path ?? null }),
  sftpMkdir: (hostId: string, path: string) => invoke<void>("sftp_mkdir", { hostId, path }),
  sftpRename: (hostId: string, from: string, to: string) => invoke<void>("sftp_rename", { hostId, from, to }),
  sftpRemove: (hostId: string, paths: string[]) => invoke<void>("sftp_remove", { hostId, paths }),
  sftpUpload: (hostId: string, sources: TransferSource[], remoteDir: string, events: Channel<TransferEvent>) =>
    invoke<number>("sftp_upload", { hostId, sources, remoteDir, events }),
  sftpDownload: (hostId: string, sources: TransferSource[], localDir: string, events: Channel<TransferEvent>) =>
    invoke<number>("sftp_download", { hostId, sources, localDir, events }),
  sftpExists: (hostId: string, paths: string[]) => invoke<boolean[]>("sftp_exists", { hostId, paths }),
  localExists: (paths: string[]) => invoke<boolean[]>("local_exists", { paths }),
  sftpCancel: (job: number) => invoke<void>("sftp_cancel", { job }),
  localList: (path?: string) => invoke<Listing>("local_list", { path: path ?? null }),
  localRoots: () => invoke<string[]>("local_roots"),
  localMkdir: (path: string) => invoke<void>("local_mkdir", { path }),
  localRemove: (paths: string[]) => invoke<void>("local_remove", { paths }),

  mcpStatus: () => invoke<McpStatus>("mcp_status"),
  mcpActivity: (query: { offset: number; limit: number; outcome: AuditOutcome | null; client: string | null }) =>
    invoke<AuditPage>("mcp_activity", { query }),
  mcpConfirmAnswer: (id: number, approve: boolean) => invoke<void>("mcp_confirm_answer", { id, approve }),

  listSnippets: () => invoke<Snippet[]>("list_snippets"),
  saveSnippet: (input: Omit<Snippet, "id"> & { id: string | null }) => invoke<Snippet>("save_snippet", { input }),
  deleteSnippet: (id: string) => invoke<void>("delete_snippet", { id }),
  broadcastCommand: (hostIds: string[], command: string, results: Channel<HostRun>) =>
    invoke<void>("broadcast_command", { hostIds, command, results }),

  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (settings: Settings) => invoke<Settings>("save_settings", { settings }),
};
