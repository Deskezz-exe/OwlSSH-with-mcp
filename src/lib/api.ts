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
    scrollback: number;
    cursorBlink: boolean;
    copyOnSelect: boolean;
    rightClick: "menu" | "paste";
  };
  keybindings: { copy: string; paste: string };
  streamerMode: boolean;
  mcp: { enabled: boolean; port: number; confirmDangerous: boolean };
}

export interface TerminalExit {
  id: number;
  exitCode: number | null;
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

  terminalOpen: (hostId: string, cols: number, rows: number, output: Channel<ArrayBuffer>) =>
    invoke<number>("terminal_open", { hostId, cols, rows, output }),
  terminalWrite: (id: number, data: string) => invoke<void>("terminal_write", { id, data }),
  terminalResize: (id: number, cols: number, rows: number) =>
    invoke<void>("terminal_resize", { id, cols, rows }),
  terminalClose: (id: number) => invoke<void>("terminal_close", { id }),

  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (settings: Settings) => invoke<Settings>("save_settings", { settings }),
};
