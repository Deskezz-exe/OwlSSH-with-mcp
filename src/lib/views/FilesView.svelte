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
  import { remoteJoin, localJoin } from "../paths";
  import FilePane from "../components/FilePane.svelte";
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
          const side = sideAt(p.position.x / devicePixelRatio, p.position.y / devicePixelRatio);
          hover = null;
          if (side === "remote" && p.paths.length) upload(p.paths);
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
    const paths = entries.map((x) => x.path);
    if (to === "remote") upload(paths, folder ?? undefined);
    else download(paths, folder ?? undefined);
  }

  function track(direction: Job["direction"], refreshSide: Side) {
    const channel = new Channel<TransferEvent>();
    let job: Job | undefined;
    channel.onmessage = (ev) => {
      if (ev.event === "started") {
        job = {
          id: ev.job,
          direction,
          filesTotal: ev.filesTotal,
          filesDone: 0,
          bytesTotal: ev.bytesTotal,
          bytesDone: 0,
          file: "",
          status: "running",
        };
        jobs.push(job);
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
        if (ev.event === "failed") toast.show(t("files.transferFailed", { message: ev.message }), "error");
        pane(refreshSide)?.refresh();
        setTimeout(() => (jobs = jobs.filter((x) => x.id !== j.id)), ev.event === "failed" ? 8000 : 3000);
      }
    };
    return channel;
  }

  async function upload(localPaths: string[], remoteDir?: string) {
    const dir = remoteDir ?? remotePane?.currentPath();
    if (!dir) return;
    try {
      await api.sftpUpload(hostId, localPaths, dir, track("up", "remote"));
    } catch (e) {
      toast.show(errorText(e), "error");
    }
  }

  async function download(remotePaths: string[], localDir?: string) {
    const dir = localDir ?? localPane?.currentPath();
    if (!dir) return;
    try {
      await api.sftpDownload(hostId, remotePaths, dir, track("down", "local"));
    } catch (e) {
      toast.show(errorText(e), "error");
    }
  }

  function transfer(from: Side, entries?: Entry[]) {
    const items = entries ?? pane(from)?.selectedEntries() ?? [];
    if (!items.length) return;
    const paths = items.map((x) => x.path);
    if (from === "local") upload(paths);
    else download(paths);
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
    />
    <div class="middle">
      <button class="btn" title={t("files.upload")} aria-label={t("files.upload")} onclick={() => transfer("local")}>→</button>
      <button class="btn" title={t("files.download")} aria-label={t("files.download")} onclick={() => transfer("remote")}>←</button>
    </div>
    <FilePane
      bind:this={remotePane}
      side="remote"
      title={`${t("files.remote")} · ${host?.name ?? ""}`}
      load={(path) => api.sftpList(hostId, path)}
      dropTarget={hover === "remote"}
      onopenfile={(entry) => transfer("remote", [entry])}
      ondragout={(entries, e) => startDrag("remote", entries, e)}
      oncontext={(entry, e) => contextMenu("remote", entry, e)}
      onkeyaction={(action) => keyAction("remote", action)}
    />
  </div>

  {#if jobs.length}
    <ul class="jobs">
      {#each jobs as job (job.id)}
        <li class={job.status}>
          <span class="dir mono">{job.direction === "up" ? "→" : "←"}</span>
          <div class="info">
            <div class="line">
              <span class="file">
                {#if job.status === "done"}{t("files.transferDone")}
                {:else if job.status === "cancelled"}{t("files.transferCancelled")}
                {:else}{job.file.split(/[\\/]/).pop() || "…"}{/if}
              </span>
              <span class="stats">
                {t("files.transferring", { done: job.filesDone, total: job.filesTotal })} ·
                {formatBytes(job.bytesDone)} / {formatBytes(job.bytesTotal)}
              </span>
            </div>
            <span class="track"><span class="fill" style:width="{percentOf(job)}%"></span></span>
          </div>
          {#if job.status === "running"}
            <button class="btn ghost" onclick={() => api.sftpCancel(job.id)}>{t("files.cancel")}</button>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</div>

{#if drag}
  <div class="ghost" style:left="{drag.x + 14}px" style:top="{drag.y + 10}px">
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
    gap: 10px;
    height: 100%;
    padding: 12px;
  }
  .panes {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
    gap: 8px;
  }
  .middle {
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 6px;
  }
  .middle .btn {
    width: 34px;
    padding: 0;
    font-size: 15px;
  }
  .jobs {
    display: grid;
    gap: 6px;
    margin: 0;
    padding: 0;
    list-style: none;
    max-height: 30%;
    overflow: auto;
  }
  .jobs li {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 12px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }
  .dir {
    color: var(--accent);
  }
  .info {
    flex: 1;
    min-width: 0;
    display: grid;
    gap: 5px;
  }
  .line {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    font-size: 12.5px;
  }
  .file {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .stats {
    flex: none;
    color: var(--text-dim);
  }
  .track {
    height: 4px;
    border-radius: 2px;
    background: var(--raised);
    overflow: hidden;
  }
  .fill {
    display: block;
    height: 100%;
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
  .ghost {
    position: fixed;
    z-index: 80;
    padding: 5px 10px;
    border-radius: var(--radius-sm);
    background: var(--accent);
    color: var(--accent-text);
    font-size: 12.5px;
    font-weight: 500;
    pointer-events: none;
    box-shadow: var(--shadow);
  }
</style>
