import { api, isAppError, errorText, type ConnectOptions, type Host, type HostInput } from "../api";
import { prompt } from "./prompt.svelte";

export type ConnState = "idle" | "connecting" | "connected" | "error";

class HostsState {
  list = $state<Host[]>([]);
  status = $state<Record<string, ConnState>>({});
  errors = $state<Record<string, string>>({});

  /** Favorites first, then the manual order. */
  sorted = $derived(
    [...this.list].sort(
      (a, b) => Number(b.favorite) - Number(a.favorite) || a.sortOrder - b.sortOrder,
    ),
  );

  connectedCount = $derived(Object.values(this.status).filter((s) => s === "connected").length);

  async load() {
    this.list = await api.listHosts();
    for (const id of await api.connectedHosts()) this.status[id] = "connected";
  }

  byId(id: string | null | undefined): Host | undefined {
    return this.list.find((h) => h.id === id);
  }

  stateOf(id: string): ConnState {
    return this.status[id] ?? "idle";
  }

  async save(input: HostInput): Promise<Host> {
    const host = await api.saveHost(input);
    const i = this.list.findIndex((h) => h.id === host.id);
    if (i >= 0) this.list[i] = host;
    else this.list.push(host);
    return host;
  }

  async remove(id: string) {
    await this.disconnect(id);
    await api.deleteHost(id);
    this.list = (await api.listHosts()) ?? this.list.filter((h) => h.id !== id);
    delete this.status[id];
  }

  async toggleFavorite(host: Host) {
    const updated = await api.setFavorite(host.id, !host.favorite);
    const i = this.list.findIndex((h) => h.id === host.id);
    if (i >= 0) this.list[i] = updated;
  }

  async reorder(ids: string[]) {
    await api.reorderHosts(ids);
    this.list = await api.listHosts();
  }

  async importAliases(aliases: string[]): Promise<number> {
    const created = await api.importSshConfig(aliases);
    this.list = await api.listHosts();
    return created.length;
  }

  /**
   * Connects, asking the user for whatever is missing: a password, a key
   * passphrase, or trust in an unknown host key. Returns false if the user
   * cancelled or the connection failed (the error is kept in `errors`).
   */
  async connect(id: string): Promise<boolean> {
    if (this.stateOf(id) === "connected") return true;
    this.status[id] = "connecting";
    delete this.errors[id];
    const options: ConnectOptions = {};

    for (let attempt = 0; attempt < 6; attempt++) {
      try {
        await api.connectHost(id, options);
        this.status[id] = "connected";
        if (options.savePassword) await this.load();
        return true;
      } catch (e) {
        const answered = await this.resolveError(e, options);
        if (!answered) {
          this.status[id] = answered === null ? "idle" : "error";
          if (answered === false) this.errors[id] = errorText(e);
          return false;
        }
      }
    }
    this.status[id] = "error";
    return false;
  }

  /** true: retry with updated options; null: user cancelled; false: real error. */
  private async resolveError(e: unknown, options: ConnectOptions): Promise<boolean | null> {
    if (!isAppError(e)) return false;
    const d = (e.details ?? {}) as Record<string, string>;
    const target = (hostId: string) => {
      const h = this.byId(hostId);
      return h ? `${h.username}@${h.hostname}` : String(d.hostname ?? "");
    };

    switch (e.kind) {
      case "needPassword":
      case "auth": {
        if (e.kind === "auth" && options.password === undefined && !this.hasPasswordAuth(d.hostId)) {
          return false;
        }
        const a = await prompt.ask({ kind: "password", target: target(d.hostId ?? "") });
        if (!a || a.kind !== "password") return null;
        options.password = a.password;
        options.savePassword = a.save;
        return true;
      }
      case "needPassphrase": {
        const a = await prompt.ask({ kind: "passphrase", keyPath: d.keyPath });
        if (!a || a.kind !== "passphrase") return null;
        options.passphrase = a.passphrase;
        return true;
      }
      case "hostKeyUnknown":
      case "hostKeyChanged": {
        const a = await prompt.ask({
          kind: "hostKey",
          changed: e.kind === "hostKeyChanged",
          target: `${d.hostname}:${d.port}`,
          fingerprint: d.fingerprint,
          algorithm: d.algorithm,
        });
        if (!a) return null;
        options.trustFingerprint = d.fingerprint;
        return true;
      }
      default:
        return false;
    }
  }

  private hasPasswordAuth(hostId: string | undefined): boolean {
    return this.byId(hostId)?.auth === "password";
  }

  async disconnect(id: string) {
    if (this.stateOf(id) === "idle") return;
    await api.disconnectHost(id);
    this.status[id] = "idle";
  }
}

export const hosts = new HostsState();
