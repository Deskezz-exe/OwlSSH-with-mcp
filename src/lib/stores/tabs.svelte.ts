export type TabKind = "terminal" | "files";

export interface Tab {
  id: string;
  kind: TabKind;
  hostId: string;
  /** What the shell put in the window title, minus the user@host part. */
  title?: string;
  /** The shell rang the bell while the tab was in the background. */
  bell?: boolean;
  /** A shell is running in the tab. */
  live?: boolean;
}

/** Fixed screens that are not tabs. */
export type Screen = "dashboard" | "settings" | "mcp" | "snippets" | "tunnels";

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

  setLive(id: string, live: boolean) {
    const tab = this.list.find((t) => t.id === id);
    if (tab) tab.live = live;
  }

  /** The user is looking at the tab now. */
  seen(id: string) {
    const tab = this.list.find((t) => t.id === id);
    if (tab?.bell) tab.bell = false;
  }

  setTitle(id: string, title: string) {
    const tab = this.list.find((t) => t.id === id);
    // Prompts usually set "user@host: ~/dir"; the host is already on the tab.
    if (tab) tab.title = title.replace(/^[^@\s]+@[^:\s]+:\s*/, "").trim() || undefined;
  }

  ring(id: string) {
    const tab = this.list.find((t) => t.id === id);
    if (tab && this.active !== id) tab.bell = true;
  }
}

export const tabs = new TabsState();
