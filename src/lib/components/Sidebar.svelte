<script lang="ts">
  import { t } from "../i18n/index.svelte";
  import { hosts } from "../stores/hosts.svelte";
  import { tabs } from "../stores/tabs.svelte";
  import { shownAddress } from "../privacy";
  import OwlEyes from "./OwlEyes.svelte";
  import type { Host } from "../api";

  let {
    onadd,
    onhostmenu,
  }: {
    onadd: () => void;
    onhostmenu: (host: Host, e: MouseEvent) => void;
  } = $props();

  let query = $state("");

  const filtered = $derived.by(() => {
    const q = query.trim().toLowerCase();
    if (!q) return hosts.sorted;
    return hosts.sorted.filter(
      (h) =>
        h.name.toLowerCase().includes(q) ||
        h.hostname.toLowerCase().includes(q) ||
        h.tags.some((tag) => tag.toLowerCase().includes(q)),
    );
  });

  /** Brings up the host's terminal, connecting first if needed. */
  async function openTerminal(host: Host) {
    if (await hosts.connect(host.id)) tabs.focusOrOpen("terminal", host.id);
  }
</script>

<aside>
  <div class="brand">
    <img src="/logo.png" alt="" width="26" height="26" />
    <span>{t("app.name")}</span>
  </div>

  <nav>
    <button class="nav" class:active={tabs.active === "dashboard"} onclick={() => tabs.show("dashboard")}>
      <span class="glyph">▦</span>{t("nav.dashboard")}
    </button>
  </nav>

  <div class="section">
    <span>{t("nav.servers")}</span>
    <button class="btn ghost add" title={t("hosts.add")} aria-label={t("hosts.add")} onclick={onadd}>+</button>
  </div>

  <input class="input search" type="search" placeholder={t("nav.search")} bind:value={query} />

  <ul class="hosts">
    {#each filtered as host (host.id)}
      <li>
        <button
          class="host"
          title={`${host.username}@${shownAddress(host)}`}
          onclick={() => openTerminal(host)}
          oncontextmenu={(e) => {
            e.preventDefault();
            onhostmenu(host, e);
          }}
        >
          <OwlEyes state={hosts.stateOf(host.id)} size={16} />
          <span class="name">{host.name}</span>
          {#if host.favorite}<span class="star" aria-hidden="true">★</span>{/if}
        </button>
      </li>
    {:else}
      {#if query}
        <li class="none">{t("hosts.noMatches", { query })}</li>
      {/if}
    {/each}
  </ul>

  <button class="nav settings" class:active={tabs.active === "settings"} onclick={() => tabs.show("settings")}>
    <span class="glyph">⚙</span>{t("nav.settings")}
  </button>
</aside>

<style>
  aside {
    display: flex;
    flex-direction: column;
    min-height: 0;
    padding: 14px 10px 10px;
    background: var(--surface);
    border-right: 1px solid var(--border);
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 2px 8px 16px;
  }
  .brand img {
    border-radius: 6px;
  }
  .brand span {
    font-family: var(--font-display);
    font-weight: 700;
    font-size: 16px;
    letter-spacing: 0.02em;
  }
  .nav {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    height: 34px;
    padding: 0 10px;
    border: 0;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--text-dim);
    font-weight: 500;
    text-align: left;
  }
  .nav:hover {
    background: var(--hover);
    color: var(--text);
  }
  .nav.active {
    background: var(--accent-soft);
    color: var(--text);
  }
  .glyph {
    width: 16px;
    text-align: center;
    font-size: 15px;
  }
  .section {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 18px 4px 6px 10px;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-faint);
  }
  .add {
    width: 26px;
    height: 26px;
    padding: 0;
    font-size: 17px;
    color: var(--text-dim);
  }
  .search {
    height: 30px;
    margin-bottom: 6px;
    font-size: 13px;
  }
  .hosts {
    flex: 1;
    min-height: 0;
    overflow: auto;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .host {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    height: 32px;
    padding: 0 10px;
    border: 0;
    border-radius: var(--radius-sm);
    background: transparent;
    text-align: left;
  }
  .host:hover {
    background: var(--hover);
  }
  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .star {
    color: var(--accent);
    font-size: 11px;
  }
  .none {
    padding: 8px 10px;
    color: var(--text-faint);
    font-size: 13px;
  }
  .settings {
    margin-top: 8px;
  }
</style>
