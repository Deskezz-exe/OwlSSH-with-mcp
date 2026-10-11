<script lang="ts">
  import { onMount } from "svelte";
  import { t } from "./lib/i18n/index.svelte";
  import { settings } from "./lib/stores/settings.svelte";
  import { hosts } from "./lib/stores/hosts.svelte";
  import { tabs, type Tab } from "./lib/stores/tabs.svelte";
  import type { Host } from "./lib/api";
  import Sidebar from "./lib/components/Sidebar.svelte";
  import TabBar from "./lib/components/TabBar.svelte";
  import ContextMenu, { type MenuItem } from "./lib/components/ContextMenu.svelte";

  let ready = $state(false);
  let menu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);

  onMount(async () => {
    await settings.load();
    await hosts.load();
    ready = true;
  });

  function hostMenu(host: Host, e: MouseEvent) {
    const connected = hosts.stateOf(host.id) === "connected";
    menu = {
      x: e.clientX,
      y: e.clientY,
      items: [
        { label: t("hosts.openTerminal"), action: () => openTab("terminal", host) },
        { label: t("hosts.openFiles"), action: () => openTab("files", host) },
        {
          label: connected ? t("hosts.disconnect") : t("hosts.connect"),
          action: () => (connected ? disconnect(host) : hosts.connect(host.id)),
        },
        {
          label: host.favorite ? t("hosts.unfavorite") : t("hosts.favorite"),
          action: () => hosts.toggleFavorite(host),
          separatorBefore: true,
        },
      ],
    };
  }

  async function openTab(kind: "terminal" | "files", host: Host) {
    if (await hosts.connect(host.id)) tabs.open(kind, host.id);
  }

  async function disconnect(host: Host) {
    tabs.closeHost(host.id);
    await hosts.disconnect(host.id);
  }

  function closeTab(tab: Tab) {
    tabs.close(tab.id);
  }
</script>

{#if ready}
  <div class="shell">
    <Sidebar onadd={() => {}} onhostmenu={hostMenu} />
    <main>
      <TabBar onclose={closeTab} />
      <section class="content">
        {#if tabs.active === "dashboard"}
          <div class="placeholder">{t("nav.dashboard")}</div>
        {:else if tabs.active === "settings"}
          <div class="placeholder">{t("settings.title")}</div>
        {:else}
          <div class="placeholder">{tabs.active}</div>
        {/if}
      </section>
      <footer class="statusbar">
        <span>{t("status.connectedCount", { count: hosts.connectedCount })}</span>
      </footer>
    </main>
  </div>
{/if}

{#if menu}
  <ContextMenu {...menu} onclose={() => (menu = null)} />
{/if}

<style>
  .shell {
    display: grid;
    grid-template-columns: 236px 1fr;
    height: 100vh;
  }
  main {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }
  .content {
    position: relative;
    flex: 1;
    min-height: 0;
  }
  .placeholder {
    display: grid;
    place-items: center;
    height: 100%;
    color: var(--text-faint);
  }
  .statusbar {
    display: flex;
    align-items: center;
    gap: 16px;
    height: 26px;
    padding: 0 12px;
    font-size: 12px;
    color: var(--text-faint);
    background: var(--surface);
    border-top: 1px solid var(--border);
  }
</style>
