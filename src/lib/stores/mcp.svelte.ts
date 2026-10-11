import { listen } from "@tauri-apps/api/event";
import { api, type AuditEntry, type ConfirmRequest, type McpStatus } from "../api";
import { hosts } from "./hosts.svelte";

class McpState {
  status = $state<McpStatus | null>(null);
  activity = $state<AuditEntry[]>([]);
  /** Approval requests from assistants, oldest first. */
  pending = $state<ConfirmRequest[]>([]);

  async start() {
    await this.refresh();
    this.activity = await api.mcpActivity(200).catch(() => []);
    await listen<AuditEntry>("mcp-activity", ({ payload }) => {
      this.activity = [payload, ...this.activity].slice(0, 200);
    });
    await listen<ConfirmRequest>("mcp-confirm", ({ payload }) => {
      this.pending.push(payload);
    });
    await listen<number>("mcp-confirm-closed", ({ payload }) => {
      this.pending = this.pending.filter((r) => r.id !== payload);
    });
    // An assistant connected a host: show it as connected in the UI too.
    await listen<string>("host-connected", ({ payload }) => {
      hosts.status[payload] = "connected";
    });
  }

  async refresh() {
    this.status = await api.mcpStatus().catch(() => null);
  }

  async answer(request: ConfirmRequest, approve: boolean) {
    this.pending = this.pending.filter((r) => r.id !== request.id);
    await api.mcpConfirmAnswer(request.id, approve);
  }
}

export const mcp = new McpState();
