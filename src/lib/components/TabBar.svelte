<script lang="ts">
  import { t } from "../i18n/index.svelte";
  import { hosts } from "../stores/hosts.svelte";
  import { tabs, type Tab } from "../stores/tabs.svelte";
  import { settings } from "../stores/settings.svelte";
  import OwlEyes from "./OwlEyes.svelte";

  let { onclose }: { onclose: (tab: Tab) => void } = $props();

  const glyph = { terminal: "❯_", files: "⇄" } as const;
</script>

{#if tabs.list.length}
  <div class="bar" role="tablist">
    {#each tabs.list as tab (tab.id)}
      {@const host = hosts.byId(tab.hostId)}
      <!-- Shell titles can name real machines and paths, so streamer mode hides them. -->
      {@const sub = settings.value?.streamerMode ? "" : (tab.title ?? "")}
      <div
        class="tab"
        class:active={tabs.active === tab.id}
        class:bell={tab.bell}
        role="tab"
        tabindex="0"
        aria-selected={tabs.active === tab.id}
        onclick={() => tabs.show(tab.id)}
        onkeydown={(e) => e.key === "Enter" && tabs.show(tab.id)}
        onauxclick={(e) => e.button === 1 && onclose(tab)}
      >
        <span class="glyph mono" title={t(tab.kind === "terminal" ? "tabs.terminal" : "tabs.files")}>
          {glyph[tab.kind]}
        </span>
        <span class="title" title={sub ? `${host?.name ?? ""} · ${sub}` : undefined}>
          {host?.name ?? "…"}{#if sub}<span class="sub">· {sub}</span>{/if}
        </span>
        {#if tab.bell}<span class="bell-dot" title={t("tabs.bell")}></span>{/if}
        <OwlEyes state={hosts.stateOf(tab.hostId)} size={14} />
        <button
          class="close"
          aria-label={t("tabs.close")}
          onclick={(e) => {
            e.stopPropagation();
            onclose(tab);
          }}>✕</button
        >
      </div>
    {/each}
  </div>
{/if}

<style>
  .bar {
    display: flex;
    gap: 2px;
    padding: 6px 8px 0;
    overflow-x: auto;
    background: var(--surface);
    border-bottom: 1px solid var(--border);
    scrollbar-width: none;
  }
  .tab {
    display: flex;
    align-items: center;
    gap: 8px;
    max-width: 280px;
    height: 34px;
    padding: 0 6px 0 12px;
    border: 1px solid transparent;
    border-bottom: 0;
    border-radius: var(--radius) var(--radius) 0 0;
    color: var(--text-dim);
    cursor: pointer;
    user-select: none;
  }
  .tab:hover {
    color: var(--text);
    background: var(--hover);
  }
  .tab.active {
    color: var(--text);
    background: var(--bg);
    border-color: var(--border);
    margin-bottom: -1px;
    box-shadow: inset 0 2px 0 var(--accent);
  }
  .glyph {
    font-size: 11px;
    color: var(--accent);
  }
  /* Stays until the tab is opened, so a bell that rang while you were away is not missed. */
  .tab.bell {
    color: var(--text);
    background: var(--accent-soft);
  }
  .tab.bell .glyph {
    animation: ring 1.2s ease-in-out infinite;
  }
  .bell-dot {
    flex: none;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-soft);
  }
  @keyframes ring {
    50% {
      opacity: 0.25;
    }
  }
  .sub {
    margin-left: 5px;
    color: var(--text-faint);
  }
  .title {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .close {
    width: 22px;
    height: 22px;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: var(--text-faint);
    font-size: 11px;
  }
  .close:hover {
    background: var(--hover);
    color: var(--text);
  }
</style>
