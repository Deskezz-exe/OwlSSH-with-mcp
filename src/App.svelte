<script lang="ts">
  import { onMount } from "svelte";
  import { ask } from "@tauri-apps/plugin-dialog";
  import { t } from "./lib/i18n/index.svelte";
  import { settings } from "./lib/stores/settings.svelte";
  import { hosts } from "./lib/stores/hosts.svelte";
  import { tabs, type Tab, type TabKind } from "./lib/stores/tabs.svelte";
  import { toast } from "./lib/stores/toast.svelte";
  import { metrics } from "./lib/stores/metrics.svelte";
  import { mcp } from "./lib/stores/mcp.svelte";
  import { errorText, type Host } from "./lib/api";
  import Sidebar from "./lib/components/Sidebar.svelte";
  import TabBar from "./lib/components/TabBar.svelte";
  import ContextMenu, { type MenuItem } from "./lib/components/ContextMenu.svelte";
  import HostEditor from "./lib/components/HostEditor.svelte";
  import ImportDialog from "./lib/components/ImportDialog.svelte";
  import ConnectPrompt from "./lib/components/ConnectPrompt.svelte";
  import Toasts from "./lib/components/Toasts.svelte";
  import Dashboard from "./lib/views/Dashboard.svelte";
  import TerminalView from "./lib/views/TerminalView.svelte";
  import SettingsView from "./lib/views/SettingsView.svelte";
  import FilesView from "./lib/views/FilesView.svelte";
  import TextPrompt from "./lib/components/TextPrompt.svelte";
  import McpView from "./lib/views/McpView.svelte";
  import McpConfirm from "./lib/components/McpConfirm.svelte";

  let ready = $state(false);
  let menu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);
  /** undefined: closed, null: new host, Host: editing. */
  let editing = $state<Host | null | undefined>(undefined);
  let importing = $state(false);

  onMount(async () => {
    await settings.load();
    await hosts.load();
    metrics.start();
    mcp.start();
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
        { label: t("hosts.edit"), action: () => (editing = host) },
        { label: t("hosts.delete"), action: () => remove(host), danger: true },
      ],
    };
  }

  async function openTab(kind: TabKind, host: Host) {
    if (await hosts.connect(host.id)) tabs.open(kind, host.id);
  }

  async function disconnect(host: Host) {
    tabs.closeHost(host.id);
    await hosts.disconnect(host.id);
  }

  async function remove(host: Host) {
    const ok = await ask(t("hosts.deleteConfirm", { name: host.name }), {
      title: t("hosts.delete"),
      kind: "warning",
    });
    if (!ok) return;
    try {
      tabs.closeHost(host.id);
      await hosts.remove(host.id);
    } catch (e) {
      toast.show(errorText(e), "error");
    }
  }

  function closeTab(tab: Tab) {
    tabs.close(tab.id);
  }
</script>

{#if ready}
  <div class="shell">
    <Sidebar onadd={() => (editing = null)} onhostmenu={hostMenu} />
    <main>
      <TabBar onclose={closeTab} />
      <section class="content">
        {#if tabs.active === "dashboard"}
          <Dashboard
            onadd={() => (editing = null)}
            onimport={() => (importing = true)}
            onopen={openTab}
            onmenu={hostMenu}
          />
        {:else if tabs.active === "settings"}
          <SettingsView />
        {:else if tabs.active === "mcp"}
          <McpView />
        {/if}
        <!-- Tabs stay mounted while hidden so their sessions keep running. -->
        {#each tabs.list as tab (tab.id)}
          <div class="pane" class:hidden={tabs.active !== tab.id}>
            {#if tab.kind === "terminal"}
              <TerminalView hostId={tab.hostId} active={tabs.active === tab.id} />
            {:else}
              <FilesView hostId={tab.hostId} active={tabs.active === tab.id} />
            {/if}
          </div>
        {/each}
      </section>
      <footer class="statusbar">
        <span>{t("status.connectedCount", { count: hosts.connectedCount })}</span>
        <button class="mcp-status" class:on={mcp.status?.running} onclick={() => tabs.show("mcp")}>
          <span class="dot"></span>{t("mcp.status")}{mcp.status?.running ? ` · 127.0.0.1:${mcp.status.port}` : ""}
        </button>
      </footer>
    </main>
  </div>
{/if}

{#if menu}
  <ContextMenu {...menu} onclose={() => (menu = null)} />
{/if}
{#if editing !== undefined}
  <HostEditor host={editing} onclose={() => (editing = undefined)} />
{/if}
{#if importing}
  <ImportDialog
    onclose={() => (importing = false)}
    ondone={(count) => toast.show(t("import.done", { count }))}
  />
{/if}
<ConnectPrompt />
<TextPrompt />
<McpConfirm />
<Toasts />

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
  .pane {
    position: absolute;
    inset: 0;
  }
  .pane.hidden {
    display: none;
  }
  .mcp-status {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-left: auto;
    padding: 0 6px;
    border: 0;
    background: transparent;
    color: inherit;
    font-size: 12px;
  }
  .mcp-status:hover {
    color: var(--text);
  }
  .mcp-status .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--text-faint);
  }
  .mcp-status.on .dot {
    background: var(--ok);
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
