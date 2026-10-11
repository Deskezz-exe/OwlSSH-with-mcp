import type { Host } from "./api";
import { settings } from "./stores/settings.svelte";

/**
 * The address to show for a host. In streamer mode every address becomes a
 * stable placeholder from the documentation range (203.0.113.0/24), so the
 * screen can be shared without leaking real infrastructure.
 */
export function shownAddress(host: Pick<Host, "id" | "hostname">): string {
  if (!settings.value?.streamerMode) return host.hostname;
  let hash = 0;
  for (const ch of host.id) hash = (hash * 31 + ch.charCodeAt(0)) >>> 0;
  return `203.0.113.${(hash % 254) + 1}`;
}
