<script lang="ts">
  import { t } from "../i18n/index.svelte";
  import { hosts } from "../stores/hosts.svelte";
  import type { Host } from "../api";
  import HostCard from "../components/HostCard.svelte";
  import HostMetrics from "../components/HostMetrics.svelte";

  let {
    onadd,
    onimport,
    onopen,
    onmenu,
  }: {
    onadd: () => void;
    onimport: () => void;
    onopen: (kind: "terminal" | "files", host: Host) => void;
    onmenu: (host: Host, e: MouseEvent) => void;
  } = $props();
</script>

<div class="dashboard">
  <header>
    <h1>{t("nav.dashboard")}</h1>
    <div class="actions">
      <button class="btn" onclick={onimport}>{t("hosts.import")}</button>
      <button class="btn primary" onclick={onadd}>+ {t("hosts.add")}</button>
    </div>
  </header>

  {#if hosts.list.length === 0}
    <div class="empty">
      <img src="/logo.png" alt="" width="72" height="72" />
      <h2>{t("hosts.empty.title")}</h2>
      <p>{t("hosts.empty.body")}</p>
      <div class="actions">
        <button class="btn primary" onclick={onadd}>+ {t("hosts.add")}</button>
        <button class="btn" onclick={onimport}>{t("hosts.import")}</button>
      </div>
    </div>
  {:else}
    <div class="grid">
      {#each hosts.sorted as host (host.id)}
        <HostCard {host} onopen={(kind) => onopen(kind, host)} onmenu={(e) => onmenu(host, e)}>
          <HostMetrics hostId={host.id} />
        </HostCard>
      {/each}
    </div>
  {/if}
</div>

<style>
  .dashboard {
    height: 100%;
    overflow: auto;
    padding: 22px 26px 32px;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 20px;
  }
  h1 {
    margin: 0;
    font-family: var(--font-display);
    font-weight: 500;
    font-size: 20px;
  }
  .actions {
    display: flex;
    gap: 8px;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(270px, 1fr));
    gap: 14px;
  }
  .empty {
    display: grid;
    justify-items: center;
    gap: 6px;
    max-width: 380px;
    margin: 12vh auto 0;
    text-align: center;
  }
  .empty img {
    border-radius: 16px;
    margin-bottom: 10px;
  }
  .empty h2 {
    margin: 0;
    font-family: var(--font-display);
    font-weight: 500;
    font-size: 17px;
  }
  .empty p {
    margin: 0 0 12px;
    color: var(--text-dim);
  }
</style>
