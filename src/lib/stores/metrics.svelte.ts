import { api, type Metrics } from "../api";
import { hosts } from "./hosts.svelte";

const INTERVAL_MS = 5000;

/**
 * Polls metrics for connected hosts while the window is visible.
 * One request per host at a time; a slow server never piles up requests.
 */
class MetricsState {
  byHost = $state<Record<string, Metrics | "unavailable">>({});
  private inFlight = new Set<string>();
  private timer: ReturnType<typeof setInterval> | undefined;

  start() {
    if (this.timer) return;
    this.tick();
    this.timer = setInterval(() => this.tick(), INTERVAL_MS);
    document.addEventListener("visibilitychange", () => {
      if (document.visibilityState === "visible") this.tick();
    });
    // Read a host as soon as it connects instead of waiting for the next tick.
    $effect.root(() => {
      $effect(() => {
        for (const [id, state] of Object.entries(hosts.status)) {
          if (state === "connected" && !(id in this.byHost)) this.fetch(id);
        }
      });
    });
  }

  /** Fetch right away, e.g. just after a host connected. */
  refresh(hostId: string) {
    this.fetch(hostId);
  }

  private tick() {
    if (document.visibilityState !== "visible") return;
    for (const host of hosts.list) {
      if (hosts.stateOf(host.id) === "connected") this.fetch(host.id);
      else delete this.byHost[host.id];
    }
  }

  private async fetch(hostId: string) {
    if (this.inFlight.has(hostId)) return;
    this.inFlight.add(hostId);
    try {
      const m = await api.hostMetrics(hostId);
      if (m) this.byHost[hostId] = m;
      else delete this.byHost[hostId];
    } catch {
      this.byHost[hostId] = "unavailable";
    } finally {
      this.inFlight.delete(hostId);
    }
  }
}

export const metrics = new MetricsState();
