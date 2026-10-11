<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { Channel } from "@tauri-apps/api/core";
  import { ask } from "@tauri-apps/plugin-dialog";
  import { t } from "../i18n/index.svelte";
  import { hosts } from "../stores/hosts.svelte";
  import { dialogs } from "../stores/dialogs.svelte";
  import { toast } from "../stores/toast.svelte";
  import { api, errorText, type Entry, type TransferEvent } from "../api";
  import { formatBytes } from "../format";
  import { shownAddress } from "../privacy";
  import { remoteJoin, localJoin } from "../paths";
  import { resolveConflicts } from "../conflicts";
  import FilePane from "../components/FilePane.svelte";
  import Icon, { iconForFile } from "../components/Icon.svelte";
  import ContextMenu, { type MenuItem } from "../components/ContextMenu.svelte";

  let { hostId, active }: { hostId: string; active: boolean } = $props();

  type Side = "local" | "remote";
  let localPane: FilePane | undefined = $state();
  let remotePane: FilePane | undefined = $state();
  let roots = $state<string[]>([]);
  let menu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);

  // Drag between panes (pointer based) and drops from the OS file manager.
  let drag = $state<{ from: Side; entries: Entry[]; x: number; y: number } | null>(null);
  let hover = $state<Side | null>(null);
  let unlistenDrop: (() => void) | undefined;

  interface Job {
    id: number;
    direction: "up" | "down";
    filesTotal: number;
    filesDone: number;
    bytesTotal: number;
    bytesDone: number;
    file: string;
    status: "running" | "done" | "failed" | "cancelled";
  }
  let jobs = $state<Job[]>([]);

  const host = $derived(hosts.byId(hostId));
  const pane = (side: Side) => (side === "local" ? localPane : remotePane);

  onMount(async () => {
    roots = await api.localRoots().catch(() => []);
    try {
      const { getCurrentWebview } = await import("@tauri-apps/api/webview");
      unlistenDrop = await getCurrentWebview().onDragDropEvent((event) => {
        if (!active) return;
        const p = event.payload;
        if (p.type === "over" || p.type === "enter") {
          hover = sideAt(p.position.x / devicePixelRatio, p.position.y / devicePixelRatio) === "remote" ? "remote" : null;
        } else if (p.type === "drop") {
          const x = p.position.x / devicePixelRatio;
          const y = p.position.y / devicePixelRatio;
          hover = null;
          if (sideAt(x, y) === "remote" && p.paths.length) {
            upload(p.paths.map((path) => ({ path, isDir: false })), folderAt(x, y) ?? undefined);
          }
        } else {
          hover = null;
        }
      });
    } catch {
      // Not running inside Tauri (browser dev mode).
    }
  });

  onDestroy(() => unlistenDrop?.());

  function sideAt(x: number, y: number): Side | null {
    const el = document.elementFromPoint(x, y)?.closest<HTMLElement>("[data-pane]");
    return (el?.dataset.pane as Side | undefined) ?? null;
  }

  function folderAt(x: number, y: number): string | null {
    return document.elementFromPoint(x, y)?.closest<HTMLElement>("[data-dir-path]")?.dataset.dirPath ?? null;
  }

  function startDrag(from: Side, entries: Entry[], e: MouseEvent) {
    drag = { from, entries, x: e.clientX, y: e.clientY };
  }

  function onmousemove(e: MouseEvent) {
    if (!drag) return;
    drag.x = e.clientX;
    drag.y = e.clientY;
    const side = sideAt(e.clientX, e.clientY);
    hover = side && side !== drag.from ? side : null;
  }

  function onmouseup(e: MouseEvent) {
    if (!drag) return;
    const { from, entries } = drag;
    const to = sideAt(e.clientX, e.clientY);
    const folder = folderAt(e.clientX, e.clientY);
    drag = null;
    hover = null;
    if (!to || to === from) return;
    const items = entries.map((x) => ({ path: x.path, isDir: x.kind === "dir" }));
    if (to === "remote") upload(items, folder ?? undefined);
    else download(items, folder ?? undefined);
  }

  function track(direction: Job["direction"], refreshSide: Side) {
    const channel = new Channel<TransferEvent>();
    channel.onmessage = (ev) => {
      if (ev.event === "started") {
        jobs.push({
          id: ev.job,
          direction,
          filesTotal: ev.filesTotal,
          filesDone: 0,
          bytesTotal: ev.bytesTotal,
          bytesDone: 0,
          file: "",
          status: "running",
        });
        return;
      }
      const j = jobs.find((x) => x.id === ev.job);
      if (!j) return;
      if (ev.event === "progress") {
        j.filesDone = ev.filesDone;
        j.bytesDone = ev.bytesDone;
        j.file = ev.file;
      } else {
        j.status = ev.event === "finished" ? "done" : ev.event === "failed" ? "failed" : "cancelled";
        if (ev.event === "finished") j.bytesDone = j.bytesTotal;
        if (ev.event === "failed") toast.show(t("files.transferFailed", { message: ev.message }), "error");
        pane(refreshSide)?.refresh();
        setTimeout(() => (jobs = jobs.filter((x) => x.id !== j.id)), ev.event === "failed" ? 8000 : 3500);
      }
    };
    return channel;
  }

  type Item = { path: string; isDir: boolean };

  async function upload(items: Item[], remoteDir?: string) {
    const dir = remoteDir ?? remotePane?.currentPath();
    if (!dir || !items.length) return;
    try {
      const sources = await resolveConflicts(items, "remote", (n) => remoteJoin(dir, n), (paths) =>
        api.sftpExists(hostId, paths),
      );
      if (sources) await api.sftpUpload(hostId, sources, dir, track("up", "remote"));
    } catch (e) {
      toast.show(errorText(e), "error");
    }
  }

  async function download(items: Item[], localDir?: string) {
    const dir = localDir ?? localPane?.currentPath();
    if (!dir || !items.length) return;
    try {
      const sources = await resolveConflicts(items, "local", (n) => localJoin(dir, n), (paths) => api.localExists(paths));
      if (sources) await api.sftpDownload(hostId, sources, dir, track("down", "local"));
    } catch (e) {
      toast.show(errorText(e), "error");
    }
  }

  function transfer(from: Side, entries?: Entry[]) {
    const list = entries ?? pane(from)?.selectedEntries() ?? [];
    const items = list.map((x) => ({ path: x.path, isDir: x.kind === "dir" }));
    if (from === "local") upload(items);
    else download(items);
  }

  async function newFolder(side: Side) {
    const dir = pane(side)?.currentPath();
    if (!dir) return;
    const name = await dialogs.askText({
      title: t("files.newFolder"),
      label: t("files.newFolderPrompt"),
      value: "",
      submit: t("files.newFolder"),
    });
    if (!name) return;
    try {
      if (side === "remote") await api.sftpMkdir(hostId, remoteJoin(dir, name));
      else await api.localMkdir(localJoin(dir, name));
      pane(side)?.refresh();
    } catch (e) {
      toast.show(errorText(e), "error");
    }
  }

  async function rename(side: Side) {
    const [entry] = pane(side)?.selectedEntries() ?? [];
    const dir = pane(side)?.currentPath();
    if (!entry || !dir || side !== "remote") return;
    const name = await dialogs.askText({
      title: t("files.rename"),
      label: t("files.renamePrompt"),
      value: entry.name,
      submit: t("files.rename"),
    });
    if (!name || name === entry.name) return;
    try {
      await api.sftpRename(hostId, entry.path, remoteJoin(dir, name));
      pane(side)?.refresh();
    } catch (e) {
      toast.show(errorText(e), "error");
    }
  }

  async function remove(side: Side) {
    const entries = pane(side)?.selectedEntries() ?? [];
    if (!entries.length) return;
    const question =
      entries.length === 1
        ? t("files.deleteConfirmOne", { name: entries[0].name })
        : t("files.deleteConfirm", { count: entries.length });
    if (!(await ask(question, { title: t("files.delete"), kind: "warning" }))) return;
    try {
      const paths = entries.map((x) => x.path);
      if (side === "remote") await api.sftpRemove(hostId, paths);
      else await api.localRemove(paths);
      pane(side)?.refresh();
    } catch (e) {
      toast.show(errorText(e), "error");
    }
  }

  function contextMenu(side: Side, entry: Entry | null, e: MouseEvent) {
    const items: MenuItem[] = [];
    if (entry) {
      items.push({
        label: side === "local" ? `${t("files.upload")} →` : `← ${t("files.download")}`,
        action: () => transfer(side),
      });
      if (side === "remote") items.push({ label: t("files.rename"), action: () => rename(side) });
      items.push({ label: t("files.delete"), action: () => remove(side), danger: true, separatorBefore: true });
    }
    items.push({ label: t("files.newFolder"), action: () => newFolder(side), separatorBefore: !!entry });
    items.push({ label: t("files.refresh"), action: () => pane(side)?.refresh() });
    menu = { x: e.clientX, y: e.clientY, items };
  }

  function keyAction(side: Side, action: "delete" | "rename" | "transfer") {
    if (action === "delete") remove(side);
    else if (action === "rename") rename(side);
    else transfer(side);
  }

  const percentOf = (j: Job) => (j.bytesTotal ? Math.round((j.bytesDone / j.bytesTotal) * 100) : 100);
  const fileName = (p: string) => p.split(/[\\/]/).pop() || "…";
</script>

<svelte:window {onmousemove} {onmouseup} />

<div class="files">
  <div class="panes">
    <FilePane
      bind:this={localPane}
      side="local"
      title={t("files.local")}
      load={(path) => api.localList(path)}
      {roots}
      dropTarget={hover === "local"}
      onopenfile={(entry) => transfer("local", [entry])}
      ondragout={(entries, e) => startDrag("local", entries, e)}
      oncontext={(entry, e) => contextMenu("local", entry, e)}
      onkeyaction={(action) => keyAction("local", action)}
      onnewfolder={() => newFolder("local")}
    />
    <div class="rail">
      <button class="round" title={t("files.upload")} aria-label={t("files.upload")} onclick={() => transfer("local")}>
        <Icon name="right" size={17} />
      </button>
      <button class="round" title={t("files.download")} aria-label={t("files.download")} onclick={() => transfer("remote")}>
        <Icon name="left" size={17} />
      </button>
    </div>
    <FilePane
      bind:this={remotePane}
      side="remote"
      title={host?.name ?? t("files.remote")}
      subtitle={host ? `${host.username}@${shownAddress(host)}` : ""}
      load={(path) => api.sftpList(hostId, path)}
      dropTarget={hover === "remote"}
      onopenfile={(entry) => transfer("remote", [entry])}
      ondragout={(entries, e) => startDrag("remote", entries, e)}
      oncontext={(entry, e) => contextMenu("remote", entry, e)}
      onkeyaction={(action) => keyAction("remote", action)}
      onnewfolder={() => newFolder("remote")}
    />
  </div>

  {#if jobs.length}
    <section class="jobs" aria-label={t("files.transfers")}>
      {#each jobs as job (job.id)}
        <div class="job {job.status}">
          <span class="dir" title={job.direction === "up" ? t("files.upload") : t("files.download")}>
            <Icon name={job.direction === "up" ? "right" : "left"} size={15} />
          </span>
          <div class="info">
            <div class="line">
              <span class="file">
                {#if job.status === "done"}<Icon name="check" size={14} /> {t("files.transferDone")}
                {:else if job.status === "cancelled"}{t("files.transferCancelled")}
                {:else}<Icon name={iconForFile(fileName(job.file))} size={14} /> {fileName(job.file)}{/if}
              </span>
              <span class="stats">
                {t("files.transferring", { done: job.filesDone, total: job.filesTotal })} ·
                {formatBytes(job.bytesDone)} / {formatBytes(job.bytesTotal)}
              </span>
            </div>
            <span class="track"><span class="fill" style:width="{percentOf(job)}%"></span></span>
          </div>
          <span class="pct mono">{percentOf(job)}%</span>
          {#if job.status === "running"}
            <button class="cancel" title={t("files.cancel")} aria-label={t("files.cancel")} onclick={() => api.sftpCancel(job.id)}>
              <Icon name="x" size={14} />
            </button>
          {/if}
        </div>
      {/each}
    </section>
  {/if}
</div>

{#if drag}
  <div class="drag-ghost" style:left="{drag.x + 14}px" style:top="{drag.y + 10}px">
    <Icon name={drag.entries.length === 1 && drag.entries[0].kind === "dir" ? "folder" : "file"} size={14} />
    {drag.entries.length === 1 ? drag.entries[0].name : t("files.dragCount", { count: drag.entries.length })}
  </div>
{/if}

{#if menu}
  <ContextMenu {...menu} onclose={() => (menu = null)} />
{/if}

<style>
  .files {
    display: flex;
    flex-direction: column;
    gap: 12px;
    height: 100%;
    padding: 14px;
  }
  .panes {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
    gap: 10px;
  }
  .rail {
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 10px;
  }
  .round {
    display: grid;
    place-items: center;
    width: 38px;
    height: 38px;
    border-radius: 50%;
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--text-dim);
    transition: background 0.12s, color 0.12s, border-color 0.12s, transform 0.12s;
  }
  .round:hover {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--accent-text);
    transform: scale(1.06);
  }
  .jobs {
    display: grid;
    gap: 8px;
    max-height: 30%;
    overflow: auto;
  }
  .job {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 12px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    animation: rise 0.18s ease-out;
  }
  .dir {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border-radius: 8px;
    background: var(--accent-soft);
    color: var(--accent);
  }
  .done .dir {
    background: color-mix(in srgb, var(--ok) 15%, transparent);
    color: var(--ok);
  }
  .failed .dir,
  .cancelled .dir {
    background: var(--danger-soft);
    color: var(--danger);
  }
  .info {
    flex: 1;
    min-width: 0;
    display: grid;
    gap: 6px;
  }
  .line {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    font-size: 12.5px;
  }
  .file {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 500;
  }
  .stats {
    flex: none;
    color: var(--text-dim);
  }
  .track {
    height: 5px;
    border-radius: 3px;
    background: var(--raised);
    overflow: hidden;
  }
  .fill {
    display: block;
    height: 100%;
    border-radius: 3px;
    background: var(--accent);
    transition: width 0.2s;
  }
  .done .fill {
    background: var(--ok);
  }
  .failed .fill,
  .cancelled .fill {
    background: var(--danger);
  }
  .pct {
    width: 40px;
    text-align: right;
    font-size: 12px;
    color: var(--text-dim);
  }
  .cancel {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border: 0;
    border-radius: 7px;
    background: transparent;
    color: var(--text-faint);
  }
  .cancel:hover {
    background: var(--danger-soft);
    color: var(--danger);
  }
  .drag-ghost {
    position: fixed;
    z-index: 80;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 12px;
    border-radius: 20px;
    background: var(--accent);
    color: var(--accent-text);
    font-size: 12.5px;
    font-weight: 500;
    pointer-events: none;
    box-shadow: var(--shadow);
  }
  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(6px);
    }
  }
</style>
