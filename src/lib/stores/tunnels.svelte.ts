import { listen } from "@tauri-apps/api/event";
import { api, type Tunnel, type TunnelInput, type TunnelStatus } from "../api";
import { hosts } from "./hosts.svelte";

class TunnelsState {
  list = $state<Tunnel[]>([]);
  status = $state<Record<string, TunnelStatus>>({});
  private listening = false;

  runningCount = $derived(Object.values(this.status).filter((s) => s.running).length);

  async load() {
    this.list = await api.listTunnels().catch(() => []);
    await this.refresh();
    if (!this.listening) {
      this.listening = true;
      await listen("tunnels-changed", () => this.refresh());
    }
  }

  async refresh() {
    const all = await api.tunnelStatuses().catch(() => []);
    this.status = Object.fromEntries(all.map((s) => [s.id, s]));
  }

  async save(input: TunnelInput): Promise<Tunnel> {
    const saved = await api.saveTunnel(input);
    const i = this.list.findIndex((t) => t.id === saved.id);
    if (i >= 0) this.list[i] = saved;
    else this.list.push(saved);
    return saved;
  }

  async remove(id: string) {
    await api.deleteTunnel(id);
    this.list = this.list.filter((t) => t.id !== id);
  }

  /** Connects the host first so password and host key questions get asked. */
  async start(tunnel: Tunnel): Promise<boolean> {
    if (!(await hosts.connect(tunnel.hostId))) return false;
    await api.startTunnel(tunnel.id);
    return true;
  }

  async stop(id: string) {
    await api.stopTunnel(id);
  }
}

export const tunnels = new TunnelsState();
