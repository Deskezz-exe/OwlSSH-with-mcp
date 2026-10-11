export type TabKind = "terminal" | "files";

export interface Tab {
  id: string;
  kind: TabKind;
  hostId: string;
}

/** Fixed screens that are not tabs. */
export type Screen = "dashboard" | "settings" | "mcp";

let counter = 0;

class TabsState {
  list = $state<Tab[]>([]);
  /** A tab id or a fixed screen. */
  active = $state<string>("dashboard");

  open(kind: TabKind, hostId: string): Tab {
    const tab = { id: `${kind}-${++counter}`, kind, hostId };
    this.list.push(tab);
    this.active = tab.id;
    return tab;
  }

  /** Focuses an existing tab of this kind for the host, or opens one. */
  focusOrOpen(kind: TabKind, hostId: string) {
    const existing = this.list.find((t) => t.kind === kind && t.hostId === hostId);
    if (existing) this.active = existing.id;
    else this.open(kind, hostId);
  }

  close(id: string) {
    const i = this.list.findIndex((t) => t.id === id);
    if (i < 0) return;
    this.list.splice(i, 1);
    if (this.active === id) {
      this.active = this.list[Math.min(i, this.list.length - 1)]?.id ?? "dashboard";
    }
  }

  closeHost(hostId: string) {
    for (const tab of this.list.filter((t) => t.hostId === hostId)) this.close(tab.id);
  }

  show(screen: Screen | string) {
    this.active = screen;
  }
}

export const tabs = new TabsState();
