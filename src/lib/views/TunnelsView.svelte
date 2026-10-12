<script lang="ts">
  import { ask } from "@tauri-apps/plugin-dialog";
  import { t } from "../i18n/index.svelte";
  import { hosts } from "../stores/hosts.svelte";
  import { tunnels } from "../stores/tunnels.svelte";
  import { toast } from "../stores/toast.svelte";
  import { errorText, type Tunnel } from "../api";
  import { copyText } from "../terminal/clipboard";
  import { openLink } from "../terminal/links";
  import TunnelEditor from "../components/TunnelEditor.svelte";

  /** undefined: closed, null: new tunnel. */
  let editing = $state<Tunnel | null | undefined>(undefined);
  let busy = $state<Record<string, boolean>>({});

  async function toggle(tunnel: Tunnel) {
    busy[tunnel.id] = true;
    try {
      if (tunnels.status[tunnel.id]?.running) await tunnels.stop(tunnel.id);
      else await tunnels.start(tunnel);
    } catch (e) {
      toast.show(errorText(e), "error", 8000);
    } finally {
      busy[tunnel.id] = false;
    }
  }

  async function remove(tunnel: Tunnel) {
    const ok = await ask(t("tunnels.deleteConfirm", { name: title(tunnel) }), {
      title: t("tunnels.delete"),
      kind: "warning",
      okLabel: t("tunnels.delete"),
      cancelLabel: t("common.cancel"),
    });
    if (ok) await tunnels.remove(tunnel.id);
  }

  function title(tunnel: Tunnel): string {
    return tunnel.name || `${tunnel.targetHost}:${tunnel.targetPort}`;
  }

  /** The address to use on this computer, for local tunnels. */
  function localAddress(tunnel: Tunnel): string {
    const host = tunnel.bindHost === "0.0.0.0" ? "127.0.0.1" : tunnel.bindHost;
    return `${host}:${tunnels.status[tunnel.id]?.port ?? tunnel.bindPort}`;
  }

  const looksLikeWeb = (port: number) => [80, 443, 3000, 5173, 8000, 8080, 8443, 9000].includes(port);
</script>

<div class="page">
  <header>
    <div>
      <h1>{t("tunnels.title")}</h1>
      <p class="intro">{t("tunnels.intro")}</p>
    </div>
    <button class="btn primary" disabled={!hosts.list.length} onclick={() => (editing = null)}>+ {t("tunnels.new")}</button>
  </header>

  {#if tunnels.list.length}
    <ul class="list">
      {#each tunnels.list as tunnel (tunnel.id)}
        {@const status = tunnels.status[tunnel.id]}
        {@const host = hosts.byId(tunnel.hostId)}
        <li class="item" class:running={status?.running} class:failed={!!status?.error}>
          <span class="dot" title={status?.running ? t("tunnels.running") : t("tunnels.stopped")}></span>
          <div class="main">
            <div class="head">
              <span class="name">{title(tunnel)}</span>
              <span class="badge">{t(`tunnels.kind.${tunnel.kind}`)}</span>
              <span class="host">{host?.name ?? "?"}</span>
            </div>
            <div class="route mono">
              {#if tunnel.kind === "local"}
                {tunnel.bindHost}:{status?.port ?? tunnel.bindPort} → {host?.name ?? "?"} → {tunnel.targetHost}:{tunnel.targetPort}
              {:else}
                {host?.name ?? "?"} {tunnel.bindHost}:{status?.port ?? tunnel.bindPort} → {t("tunnels.thisComputer")} → {tunnel.targetHost}:{tunnel.targetPort}
              {/if}
            </div>
            {#if status?.error}<div class="error">{status.error}</div>{/if}
          </div>
          <div class="actions">
            {#if status?.running && tunnel.kind === "local"}
              {#if looksLikeWeb(tunnel.targetPort)}
                <button class="btn ghost" onclick={() => openLink(`http://${localAddress(tunnel)}`)}>{t("tunnels.open")}</button>
              {/if}
              <button
                class="btn ghost"
                onclick={() => {
                  copyText(localAddress(tunnel));
                  toast.show(t("tunnels.copied", { address: localAddress(tunnel) }));
                }}>{t("tunnels.copy")}</button
              >
            {/if}
            <button class="btn ghost" onclick={() => (editing = tunnel)}>{t("tunnels.editShort")}</button>
            <button class="btn ghost danger-text" onclick={() => remove(tunnel)}>{t("tunnels.delete")}</button>
            <button class="btn" class:primary={!status?.running} disabled={busy[tunnel.id]} onclick={() => toggle(tunnel)}>
              {status?.running ? t("tunnels.stop") : t("tunnels.start")}
            </button>
          </div>
        </li>
      {/each}
    </ul>
  {:else}
    <div class="empty">
      <p class="empty-title">{t("tunnels.emptyTitle")}</p>
      <p>{t("tunnels.emptyBody")}</p>
    </div>
  {/if}
</div>

{#if editing !== undefined}
  <TunnelEditor tunnel={editing} onclose={() => (editing = undefined)} />
{/if}

<style>
  .page {
    height: 100%;
    overflow: auto;
    padding: 22px 26px 24px;
  }
  header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 20px;
    margin-bottom: 18px;
  }
  h1 {
    margin: 0 0 6px;
    font-family: var(--font-display);
    font-weight: 500;
    font-size: 20px;
  }
  .intro {
    margin: 0;
    max-width: 640px;
    color: var(--text-dim);
  }
  .list {
    display: grid;
    gap: 8px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 12px 14px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
  }
  .item.running {
    border-color: var(--accent-line);
  }
  .dot {
    flex: none;
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: var(--text-faint);
  }
  .running .dot {
    background: var(--ok);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--ok) 22%, transparent);
  }
  .failed .dot {
    background: var(--danger);
  }
  .main {
    flex: 1;
    min-width: 0;
    display: grid;
    gap: 4px;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }
  .name {
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .badge {
    padding: 1px 7px;
    border-radius: 9px;
    font-size: 11px;
    color: var(--accent);
    background: var(--accent-soft);
  }
  .host {
    font-size: 12px;
    color: var(--text-faint);
  }
  .route {
    font-size: 12px;
    color: var(--text-dim);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .error {
    font-size: 12px;
    color: var(--danger);
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .danger-text {
    color: var(--danger);
  }
  .empty {
    padding: 40px 20px;
    text-align: center;
    color: var(--text-dim);
    background: var(--surface);
    border: 1px dashed var(--border-strong);
    border-radius: 10px;
  }
  .empty p {
    margin: 0 auto;
    max-width: 480px;
  }
  .empty-title {
    margin-bottom: 6px !important;
    font-weight: 500;
    color: var(--text);
  }
</style>
