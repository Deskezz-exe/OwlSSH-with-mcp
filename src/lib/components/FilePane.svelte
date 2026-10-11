<script lang="ts">
  import { onMount, tick } from "svelte";
  import { i18n, t } from "../i18n/index.svelte";
  import { errorText, type Entry, type Listing } from "../api";
  import { formatBytes } from "../format";
  import Icon, { iconForFile } from "./Icon.svelte";

  let {
    side,
    title,
    subtitle = "",
    load,
    roots = [],
    dropTarget = false,
    onopenfile,
    ondragout,
    oncontext,
    onkeyaction,
    onnewfolder,
  }: {
    side: "local" | "remote";
    title: string;
    subtitle?: string;
    load: (path?: string) => Promise<Listing>;
    roots?: string[];
    /** Highlights the pane while something is dragged over it. */
    dropTarget?: boolean;
    onopenfile: (entry: Entry) => void;
    /** The user started dragging these entries out of the pane. */
    ondragout: (entries: Entry[], e: MouseEvent) => void;
    oncontext: (entry: Entry | null, e: MouseEvent) => void;
    onkeyaction: (action: "delete" | "rename" | "transfer") => void;
    onnewfolder: () => void;
  } = $props();

  type SortKey = "name" | "size" | "modified";

  let listing = $state<Listing | null>(null);
  let error = $state("");
  let loading = $state(false);
  let selected = $state<Set<string>>(new Set());
  let anchor: string | null = null;
  let list: HTMLDivElement | undefined = $state();
  let editingPath = $state(false);
  let pathInput = $state("");
  let pathField: HTMLInputElement | undefined = $state();
  let sortKey = $state<SortKey>("name");
  let sortAsc = $state(true);

  const sep = $derived(side === "remote" || !listing?.path.includes("\\") ? "/" : "\\");
  const dateFormat = $derived(new Intl.DateTimeFormat(i18n.locale, { dateStyle: "short", timeStyle: "short" }));

  /** Folders always first, then the chosen column. */
  const entries = $derived.by(() => {
    const items = [...(listing?.entries ?? [])];
    const dir = sortAsc ? 1 : -1;
    items.sort((a, b) => {
      const folders = Number(b.kind === "dir") - Number(a.kind === "dir");
      if (folders) return folders;
      let cmp = 0;
      if (sortKey === "size") cmp = a.size - b.size;
      else if (sortKey === "modified") cmp = (a.modified ?? 0) - (b.modified ?? 0);
      if (!cmp) cmp = a.name.localeCompare(b.name, undefined, { numeric: true, sensitivity: "base" });
      return cmp * dir;
    });
    return items;
  });

  /** Clickable pieces of the current path: label and the path it leads to. */
  const crumbs = $derived.by(() => {
    const path = listing?.path ?? "";
    if (!path) return [];
    if (sep === "/") {
      const parts = path.split("/").filter(Boolean);
      return [{ label: "/", path: "/" }, ...parts.map((p, i) => ({ label: p, path: "/" + parts.slice(0, i + 1).join("/") }))];
    }
    const parts = path.split("\\").filter(Boolean);
    return parts.map((p, i) => ({
      label: p,
      path: i === 0 ? `${p}\\` : parts.slice(0, i + 1).join("\\"),
    }));
  });

  const selectedSize = $derived(
    entries.filter((e) => selected.has(e.path) && e.kind === "file").reduce((n, e) => n + e.size, 0),
  );

  export function currentPath(): string | null {
    return listing?.path ?? null;
  }

  export function selectedEntries(): Entry[] {
    return entries.filter((e) => selected.has(e.path));
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
      const same = listing?.path === next.path;
      listing = next;
      if (!same) {
        selected = new Set();
        list?.scrollTo({ top: 0 });
      } else {
        selected = new Set([...selected].filter((p) => next.entries.some((e) => e.path === p)));
      }
    } catch (e) {
      error = errorText(e);
    } finally {
      loading = false;
      editingPath = false;
    }
  }

  /** Parent folder for `/a/b`, `C:\a\b` and the roots `/` and `C:\`. */
  function parentOf(path: string): string {
    const trimmed = path.length > 1 ? path.replace(/[\\/]+$/, "") : path;
    const i = trimmed.lastIndexOf(sep);
    if (i < 0) return path;
    const parent = trimmed.slice(0, i);
    if (sep === "/") return parent || "/";
    return parent.endsWith(":") ? `${parent}\\` : parent;
  }

  async function editPath() {
    pathInput = listing?.path ?? "";
    editingPath = true;
    await tick();
    pathField?.select();
  }

  function setSort(key: SortKey) {
    if (sortKey === key) sortAsc = !sortAsc;
    else {
      sortKey = key;
      sortAsc = key === "name";
    }
  }

  function open(entry: Entry) {
    if (entry.kind === "dir" || entry.kind === "symlink") go(entry.path);
    else onopenfile(entry);
  }

  function select(entry: Entry, e: MouseEvent) {
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
  let press: { x: number; y: number } | null = null;

  function onrowdown(entry: Entry, e: MouseEvent) {
    if (e.button !== 0) return;
    select(entry, e);
    press = { x: e.clientX, y: e.clientY };
  }

  function onwindowmove(e: MouseEvent) {
    if (!press || !(e.buttons & 1)) {
      press = null;
      return;
    }
    if (Math.hypot(e.clientX - press.x, e.clientY - press.y) > 5) {
      const items = selectedEntries();
      press = null;
      if (items.length) ondragout(items, e);
    }
  }

  function onrowclick(entry: Entry, e: MouseEvent) {
    if (!e.shiftKey && !e.ctrlKey && !e.metaKey) selected = new Set([entry.path]);
  }

  function onkeydown(e: KeyboardEvent) {
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

  const iconOf = (entry: Entry) =>
    entry.kind === "dir" ? "folder" : entry.kind === "symlink" ? "link" : iconForFile(entry.name);
</script>

<svelte:window onmousemove={onwindowmove} onmouseup={() => (press = null)} />

<section class="pane" class:drop={dropTarget} data-pane={side}>
  <header>
    <span class="badge"><Icon name={side === "local" ? "laptop" : "server"} size={15} /></span>
    <div class="titles">
      <span class="title">{title}</span>
      {#if subtitle}<span class="subtitle mono">{subtitle}</span>{/if}
    </div>
    <div class="tools">
      <button class="icon-btn" title={t("files.up")} aria-label={t("files.up")} onclick={() => listing && go(parentOf(listing.path))}>
        <Icon name="up" />
      </button>
      <button class="icon-btn" title={t("files.refresh")} aria-label={t("files.refresh")} onclick={refresh}>
        <Icon name="refresh" />
      </button>
      <button class="icon-btn" title={t("files.newFolder")} aria-label={t("files.newFolder")} onclick={onnewfolder}>
        <Icon name="folderPlus" />
      </button>
    </div>
  </header>

  <div class="pathbar">
    {#if roots.length > 1}
      <select class="drive" title={t("files.drives")} onchange={(e) => go(e.currentTarget.value)}>
        {#each roots as root}
          <option value={root} selected={listing?.path.toUpperCase().startsWith(root.toUpperCase())}>{root.replace("\\", "")}</option>
        {/each}
      </select>
    {/if}
    {#if editingPath}
      <form class="path-edit" onsubmit={(e) => { e.preventDefault(); go(pathInput); }}>
        <input
          class="input mono"
          bind:this={pathField}
          bind:value={pathInput}
          spellcheck="false"
          onblur={() => (editingPath = false)}
          onkeydown={(e) => e.key === "Escape" && (editingPath = false)}
        />
      </form>
    {:else}
      <div class="crumbs" role="navigation" ondblclick={editPath}>
        {#each crumbs as crumb, i}
          {#if i === 0 && sep === "\\" && roots.length > 1}
            <!-- The drive picker already shows the drive. -->
          {:else}
          {#if i > 1 || (i === 1 && !(sep === "/" || roots.length > 1))}<span class="sep"><Icon name="chevron" size={12} /></span>{/if}
          <button class="crumb" class:last={i === crumbs.length - 1} onclick={() => go(crumb.path)}>{crumb.label}</button>
          {/if}
        {/each}
        <button class="crumb-edit" title={t("files.editPath")} aria-label={t("files.editPath")} onclick={editPath}>
          <Icon name="pencil" size={13} />
        </button>
      </div>
    {/if}
  </div>

  <div class="head" class:remote={side === "remote"}>
    {#each [["name", "files.name"], ["size", "files.size"], ["modified", "files.modified"]] as const as [key, label]}
      <button class="sort" class:num={key === "size"} class:active={sortKey === key} onclick={() => setSort(key)}>
        {t(label)}
        {#if sortKey === key}<Icon name={sortAsc ? "sortUp" : "sortDown"} size={10} stroke={0} />{/if}
      </button>
    {/each}
    {#if side === "remote"}<span class="perm-head">{t("files.permissions")}</span>{/if}
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
      <div class="message error">{error}</div>
    {:else if listing && entries.length === 0}
      <div class="message"><Icon name="folder" size={30} stroke={1.3} /><span>{t("files.empty")}</span></div>
    {/if}
    {#each entries as entry, index (entry.path)}
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
        <span class="name">
          <span class="kind {iconOf(entry)}"><Icon name={iconOf(entry)} size={16} /></span>
          <span class="label" title={entry.name}>{entry.name}</span>
        </span>
        <span class="num mono">{entry.kind === "file" ? formatBytes(entry.size) : ""}</span>
        <span class="date">{entry.modified ? dateFormat.format(entry.modified * 1000) : ""}</span>
        {#if side === "remote"}<span class="perm mono">{entry.permissions ?? ""}</span>{/if}
      </div>
    {/each}
    {#if dropTarget}<div class="drop-hint">{t("files.dropHint")}</div>{/if}
  </div>

  <footer>
    <span>{t("files.items", { count: listing?.entries.length ?? 0 })}</span>
    {#if selected.size}
      <span class="sel">
        {t("files.selected", { count: selected.size })}{selectedSize ? ` · ${formatBytes(selectedSize)}` : ""}
      </span>
    {/if}
  </footer>
</section>

<style>
  .pane {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 12px;
    overflow: hidden;
    transition: border-color 0.15s, box-shadow 0.15s;
  }
  .pane.drop {
    border-color: var(--accent);
    box-shadow: 0 0 0 4px var(--accent-soft);
  }
  header {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 12px 10px 8px 14px;
  }
  .badge {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border-radius: 8px;
    background: var(--accent-soft);
    color: var(--accent);
  }
  .titles {
    flex: 1;
    min-width: 0;
    display: grid;
  }
  .title {
    font-family: var(--font-display);
    font-weight: 500;
    font-size: 12.5px;
  }
  .subtitle {
    font-size: 11px;
    color: var(--text-faint);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tools {
    display: flex;
    gap: 2px;
  }
  .icon-btn {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border: 0;
    border-radius: 7px;
    background: transparent;
    color: var(--text-dim);
  }
  .icon-btn:hover {
    background: var(--hover);
    color: var(--text);
  }
  .pathbar {
    display: flex;
    gap: 6px;
    margin: 0 10px 10px;
  }
  .drive {
    height: 32px;
    padding: 0 6px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--input);
    font-family: var(--font-mono);
    font-size: 12px;
  }
  .crumbs {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 1px;
    height: 32px;
    padding: 0 4px 0 6px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--input);
    overflow-x: auto;
    scrollbar-width: none;
    white-space: nowrap;
  }
  .crumb {
    flex: none;
    padding: 3px 6px;
    border: 0;
    border-radius: 5px;
    background: transparent;
    color: var(--text-dim);
    font-family: var(--font-mono);
    font-size: 12px;
  }
  .crumb:hover {
    background: var(--hover);
    color: var(--text);
  }
  .crumb.last {
    color: var(--text);
    font-weight: 700;
  }
  .sep {
    flex: none;
    color: var(--text-faint);
  }
  .crumb-edit {
    flex: none;
    margin-left: auto;
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border: 0;
    border-radius: 5px;
    background: transparent;
    color: var(--text-faint);
  }
  .crumb-edit:hover {
    color: var(--text);
    background: var(--hover);
  }
  .path-edit {
    flex: 1;
    min-width: 0;
  }
  .path-edit .input {
    height: 32px;
    border-radius: 8px;
    font-size: 12px;
  }
  .head,
  .row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 76px 118px;
    gap: 10px;
    align-items: center;
  }
  .head.remote,
  .remote .row {
    grid-template-columns: minmax(0, 1fr) 76px 118px 80px;
  }
  .head {
    height: 30px;
    padding: 0 16px;
    border-top: 1px solid var(--border);
    border-bottom: 1px solid var(--border);
    background: var(--raised);
  }
  .sort,
  .perm-head {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 0;
    border: 0;
    background: none;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    color: var(--text-faint);
  }
  .sort.num {
    justify-content: flex-end;
  }
  .sort:hover,
  .sort.active {
    color: var(--text);
  }
  .sort :global(svg) {
    fill: currentColor;
  }
  .list {
    position: relative;
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 6px;
    outline: none;
  }
  .list.loading {
    opacity: 0.55;
  }
  .row {
    height: 32px;
    padding: 0 10px;
    border-radius: 7px;
    font-size: 13px;
    cursor: default;
    user-select: none;
  }
  .row:hover {
    background: var(--hover);
  }
  .row.selected {
    background: var(--accent-soft);
    box-shadow: inset 2px 0 0 var(--accent);
  }
  .list:focus-visible .row.selected {
    box-shadow: inset 2px 0 0 var(--accent), inset 0 0 0 1px var(--accent-line);
  }
  .name {
    display: flex;
    align-items: center;
    gap: 9px;
    min-width: 0;
  }
  .label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .kind {
    color: var(--text-faint);
  }
  .kind.folder {
    color: var(--accent);
  }
  .kind.code {
    color: #6d9cf2;
  }
  .kind.image {
    color: #c38ae8;
  }
  .kind.archive {
    color: var(--warn);
  }
  .dir .label {
    font-weight: 500;
  }
  .num {
    text-align: right;
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
    color: var(--text-faint);
  }
  .message {
    display: grid;
    justify-items: center;
    gap: 8px;
    margin: 40px 12px;
    color: var(--text-faint);
    text-align: center;
  }
  .message.error {
    color: var(--danger);
  }
  .drop-hint {
    position: sticky;
    bottom: 8px;
    margin: 8px auto 0;
    width: fit-content;
    padding: 7px 14px;
    border-radius: 20px;
    background: var(--accent);
    color: var(--accent-text);
    font-weight: 500;
    font-size: 12.5px;
    pointer-events: none;
  }
  footer {
    display: flex;
    justify-content: space-between;
    gap: 10px;
    padding: 7px 14px;
    border-top: 1px solid var(--border);
    font-size: 11.5px;
    color: var(--text-faint);
  }
  .sel {
    color: var(--accent);
  }
</style>
