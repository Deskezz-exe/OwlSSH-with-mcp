<script lang="ts">
  import { onMount } from "svelte";
  import { i18n, t } from "../i18n/index.svelte";
  import { errorText, type Entry, type Listing } from "../api";
  import { formatBytes } from "../format";

  let {
    side,
    title,
    load,
    roots = [],
    dropTarget = false,
    onopenfile,
    ondragout,
    oncontext,
    onkeyaction,
  }: {
    side: "local" | "remote";
    title: string;
    load: (path?: string) => Promise<Listing>;
    roots?: string[];
    /** Highlights the pane while something is dragged over it. */
    dropTarget?: boolean;
    onopenfile: (entry: Entry) => void;
    /** The user started dragging these entries out of the pane. */
    ondragout: (entries: Entry[], e: MouseEvent) => void;
    oncontext: (entry: Entry | null, e: MouseEvent) => void;
    onkeyaction: (action: "delete" | "rename" | "transfer") => void;
  } = $props();

  let listing = $state<Listing | null>(null);
  let pathInput = $state("");
  let error = $state("");
  let loading = $state(false);
  let selected = $state<Set<string>>(new Set());
  let anchor: string | null = null;
  let list: HTMLDivElement | undefined = $state();

  const dateFormat = $derived(
    new Intl.DateTimeFormat(i18n.locale, { dateStyle: "short", timeStyle: "short" }),
  );

  export function currentPath(): string | null {
    return listing?.path ?? null;
  }

  export function selectedEntries(): Entry[] {
    return listing?.entries.filter((e) => selected.has(e.path)) ?? [];
  }

  export async function refresh() {
    await go(listing?.path);
  }

  onMount(() => go());

  async function go(path?: string) {
    loading = true;
    error = "";
    try {
      const next = await load(path);
      const keep = listing?.path === next.path;
      listing = next;
      pathInput = next.path;
      if (!keep) {
        selected = new Set();
        list?.scrollTo({ top: 0 });
      } else {
        selected = new Set([...selected].filter((p) => next.entries.some((e) => e.path === p)));
      }
    } catch (e) {
      error = errorText(e);
      if (listing) pathInput = listing.path;
    } finally {
      loading = false;
    }
  }

  /** Parent folder for `/a/b`, `C:\a\b` and the roots `/` and `C:\`. */
  function parentOf(path: string): string {
    const sep = side === "remote" || !path.includes("\\") ? "/" : "\\";
    const trimmed = path.length > 1 ? path.replace(/[\\/]+$/, "") : path;
    const i = trimmed.lastIndexOf(sep);
    if (i < 0) return path;
    const parent = trimmed.slice(0, i);
    if (sep === "/") return parent || "/";
    return parent.endsWith(":") ? `${parent}\\` : parent;
  }

  function open(entry: Entry) {
    if (entry.kind === "dir" || entry.kind === "symlink") go(entry.path);
    else onopenfile(entry);
  }

  function select(entry: Entry, e: MouseEvent) {
    const entries = listing?.entries ?? [];
    if (e.shiftKey && anchor) {
      const a = entries.findIndex((x) => x.path === anchor);
      const b = entries.findIndex((x) => x.path === entry.path);
      const [from, to] = a < b ? [a, b] : [b, a];
      selected = new Set(entries.slice(from, to + 1).map((x) => x.path));
      return;
    }
    if (e.ctrlKey || e.metaKey) {
      const next = new Set(selected);
      if (next.has(entry.path)) next.delete(entry.path);
      else next.add(entry.path);
      selected = next;
    } else if (!selected.has(entry.path)) {
      selected = new Set([entry.path]);
    }
    anchor = entry.path;
  }

  // Start a drag after the pointer moves a few pixels with the button held.
  let press: { x: number; y: number; entry: Entry } | null = null;

  function onrowdown(entry: Entry, e: MouseEvent) {
    if (e.button !== 0) return;
    select(entry, e);
    press = { x: e.clientX, y: e.clientY, entry };
  }

  function onwindowmove(e: MouseEvent) {
    if (!press || !(e.buttons & 1)) {
      press = null;
      return;
    }
    if (Math.hypot(e.clientX - press.x, e.clientY - press.y) > 5) {
      const entries = selectedEntries();
      press = null;
      if (entries.length) ondragout(entries, e);
    }
  }

  function onrowclick(entry: Entry, e: MouseEvent) {
    // A plain click on an already-selected row narrows the selection to it.
    if (!e.shiftKey && !e.ctrlKey && !e.metaKey) selected = new Set([entry.path]);
  }

  function onkeydown(e: KeyboardEvent) {
    const entries = listing?.entries ?? [];
    if (e.key === "Backspace" && listing) {
      e.preventDefault();
      go(parentOf(listing.path));
    } else if (e.key === "Enter") {
      const [first] = selectedEntries();
      if (first) open(first);
    } else if (e.key === "Delete") {
      if (selected.size) onkeyaction("delete");
    } else if (e.key === "F2") {
      if (selected.size === 1) onkeyaction("rename");
    } else if (e.key === "F5") {
      e.preventDefault();
      if (selected.size) onkeyaction("transfer");
      else refresh();
    } else if ((e.ctrlKey || e.metaKey) && e.code === "KeyA") {
      e.preventDefault();
      selected = new Set(entries.map((x) => x.path));
    } else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const current = entries.findIndex((x) => x.path === anchor);
      const next = Math.max(0, Math.min(entries.length - 1, current + (e.key === "ArrowDown" ? 1 : -1)));
      if (entries[next]) {
        anchor = entries[next].path;
        selected = new Set([anchor]);
        list?.querySelector(`[data-index="${next}"]`)?.scrollIntoView({ block: "nearest" });
      }
    }
  }

  const icon = (entry: Entry) => (entry.kind === "dir" ? "▸" : entry.kind === "symlink" ? "↪" : "·");
</script>

<svelte:window onmousemove={onwindowmove} onmouseup={() => (press = null)} />

<section class="pane" class:drop={dropTarget} data-pane={side}>
  <header>
    <span class="title">{title}</span>
    {#if selected.size > 1}<span class="count">{t("files.selected", { count: selected.size })}</span>{/if}
  </header>

  <div class="toolbar">
    <button class="btn ghost icon" title={t("files.up")} aria-label={t("files.up")} onclick={() => listing && go(parentOf(listing.path))}>←</button>
    {#if roots.length > 1}
      <select class="input roots" title={t("files.drives")} onchange={(e) => go(e.currentTarget.value)}>
        {#each roots as root}
          <option value={root} selected={listing?.path.startsWith(root)}>{root}</option>
        {/each}
      </select>
    {/if}
    <form class="path" onsubmit={(e) => { e.preventDefault(); go(pathInput); }}>
      <input class="input mono" bind:value={pathInput} spellcheck="false" />
    </form>
    <button class="btn ghost icon" title={t("files.refresh")} aria-label={t("files.refresh")} onclick={refresh}>↻</button>
  </div>

  <div class="head">
    <span>{t("files.name")}</span>
    <span class="num">{t("files.size")}</span>
    <span>{t("files.modified")}</span>
    {#if side === "remote"}<span>{t("files.permissions")}</span>{/if}
  </div>

  <div
    class="list"
    class:remote={side === "remote"}
    class:loading
    bind:this={list}
    tabindex="0"
    role="listbox"
    aria-multiselectable="true"
    aria-label={title}
    {onkeydown}
    oncontextmenu={(e) => {
      e.preventDefault();
      if ((e.target as HTMLElement).closest(".row") === null) {
        selected = new Set();
        oncontext(null, e);
      }
    }}
    onmousedown={(e) => {
      if (e.target === e.currentTarget) selected = new Set();
    }}
  >
    {#if error}
      <p class="message error">{error}</p>
    {:else if listing && listing.entries.length === 0}
      <p class="message">{t("files.empty")}</p>
    {/if}
    {#each listing?.entries ?? [] as entry, index (entry.path)}
      <!-- Keyboard selection is handled by the list (arrows, Enter, Delete, F2). -->
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <div
        class="row"
        class:selected={selected.has(entry.path)}
        class:dir={entry.kind === "dir"}
        data-index={index}
        data-dir-path={entry.kind === "dir" ? entry.path : undefined}
        role="option"
        aria-selected={selected.has(entry.path)}
        tabindex="-1"
        onmousedown={(e) => onrowdown(entry, e)}
        onclick={(e) => onrowclick(entry, e)}
        ondblclick={() => open(entry)}
        oncontextmenu={(e) => {
          e.preventDefault();
          if (!selected.has(entry.path)) selected = new Set([entry.path]);
          oncontext(entry, e);
        }}
      >
        <span class="name"><span class="kind">{icon(entry)}</span>{entry.name}</span>
        <span class="num mono">{entry.kind === "file" ? formatBytes(entry.size) : ""}</span>
        <span class="date">{entry.modified ? dateFormat.format(entry.modified * 1000) : ""}</span>
        {#if side === "remote"}<span class="perm mono">{entry.permissions ?? ""}</span>{/if}
      </div>
    {/each}
    {#if dropTarget}<div class="drop-hint">{t("files.dropHint")}</div>{/if}
  </div>
</section>

<style>
  .pane {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    overflow: hidden;
    transition: border-color 0.12s, box-shadow 0.12s;
  }
  .pane.drop {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-soft);
  }
  header {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    padding: 10px 12px 4px;
  }
  .title {
    font-family: var(--font-display);
    font-weight: 500;
    font-size: 12.5px;
  }
  .count {
    font-size: 12px;
    color: var(--accent);
  }
  .toolbar {
    display: flex;
    gap: 4px;
    padding: 4px 8px 8px;
  }
  .icon {
    width: 32px;
    padding: 0;
    font-size: 16px;
  }
  .roots {
    width: 70px;
    height: 32px;
    padding: 0 4px;
  }
  .path {
    flex: 1;
    min-width: 0;
  }
  .path .input {
    height: 32px;
    font-size: 12.5px;
  }
  .head,
  .row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 80px 128px;
    gap: 10px;
    align-items: center;
    padding: 0 12px;
  }
  .remote .row,
  .pane:has(.remote) .head {
    grid-template-columns: minmax(0, 1fr) 80px 128px 82px;
  }
  .head {
    height: 28px;
    border-top: 1px solid var(--border);
    border-bottom: 1px solid var(--border);
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-faint);
  }
  .num {
    text-align: right;
  }
  .list {
    position: relative;
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 4px;
    outline: none;
  }
  .list.loading {
    opacity: 0.6;
  }
  .row {
    height: 28px;
    padding: 0 8px;
    border-radius: var(--radius-sm);
    font-size: 13px;
    cursor: default;
    user-select: none;
  }
  .row:hover {
    background: var(--hover);
  }
  .row.selected {
    background: var(--accent-soft);
  }
  .list:focus-visible .row.selected {
    box-shadow: inset 0 0 0 1px var(--accent);
  }
  .name {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .kind {
    flex: none;
    width: 12px;
    text-align: center;
    color: var(--text-faint);
  }
  .dir .kind {
    color: var(--accent);
  }
  .dir .name {
    font-weight: 500;
  }
  .num,
  .date,
  .perm {
    font-size: 12px;
    color: var(--text-dim);
    white-space: nowrap;
  }
  .perm {
    font-size: 11.5px;
  }
  .message {
    margin: 16px 12px;
    color: var(--text-faint);
  }
  .message.error {
    color: var(--danger);
  }
  .drop-hint {
    position: sticky;
    bottom: 8px;
    margin: 8px auto 0;
    width: fit-content;
    padding: 6px 12px;
    border-radius: var(--radius-sm);
    background: var(--accent);
    color: var(--accent-text);
    font-weight: 500;
    font-size: 12.5px;
    pointer-events: none;
  }
</style>
