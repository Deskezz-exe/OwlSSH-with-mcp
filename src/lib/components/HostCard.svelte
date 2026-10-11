<script lang="ts">
  import type { Snippet } from "svelte";
  import { t } from "../i18n/index.svelte";
  import { hosts } from "../stores/hosts.svelte";
  import { shownAddress } from "../privacy";
  import type { Host } from "../api";
  import OwlEyes from "./OwlEyes.svelte";

  let {
    host,
    onopen,
    onmenu,
    children,
  }: {
    host: Host;
    onopen: (kind: "terminal" | "files") => void;
    onmenu: (e: MouseEvent) => void;
    children?: Snippet;
  } = $props();

  const state = $derived(hosts.stateOf(host.id));
  const jump = $derived(hosts.byId(host.jumpHostId));
  const error = $derived(hosts.errors[host.id]);
</script>

<article
  class="card"
  class:connected={state === "connected"}
  oncontextmenu={(e) => {
    e.preventDefault();
    onmenu(e);
  }}
>
  <header>
    <OwlEyes {state} />
    <h3 title={host.name}>{host.name}</h3>
    <button
      class="star"
      class:on={host.favorite}
      aria-label={host.favorite ? t("hosts.unfavorite") : t("hosts.favorite")}
      title={host.favorite ? t("hosts.unfavorite") : t("hosts.favorite")}
      onclick={() => hosts.toggleFavorite(host)}>{host.favorite ? "★" : "☆"}</button
    >
    <button class="more" aria-label="⋯" onclick={onmenu}>⋯</button>
  </header>

  <p class="address mono">
    {host.username}@{shownAddress(host)}{host.port !== 22 ? `:${host.port}` : ""}
    {#if jump}<span class="via">{t("hosts.via", { name: jump.name })}</span>{/if}
  </p>

  {#if host.tags.length}
    <ul class="tags">
      {#each host.tags as tag}<li>{tag}</li>{/each}
    </ul>
  {/if}

  {#if children}{@render children()}{/if}

  {#if error && state === "error"}
    <p class="error" title={error}>{error}</p>
  {/if}

  <footer>
    <button class="btn" onclick={() => onopen("terminal")}><span class="mono glyph">❯_</span>{t("tabs.terminal")}</button>
    <button class="btn" onclick={() => onopen("files")}><span class="mono glyph">⇄</span>{t("tabs.files")}</button>
  </footer>
</article>

<style>
  .card {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
    padding: 14px 14px 12px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    transition: border-color 0.15s;
  }
  .card.connected {
    border-color: var(--border-strong);
  }
  .card:hover {
    border-color: var(--accent-line);
  }
  header {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  h3 {
    flex: 1;
    min-width: 0;
    margin: 0;
    font-size: 15px;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .star,
  .more {
    width: 26px;
    height: 26px;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: var(--text-faint);
    font-size: 15px;
  }
  .star:hover,
  .more:hover {
    background: var(--hover);
    color: var(--text);
  }
  .star.on {
    color: var(--accent);
  }
  .address {
    margin: 0;
    font-size: 12px;
    color: var(--text-dim);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .via {
    margin-left: 6px;
    font-family: var(--font-body);
    color: var(--text-faint);
  }
  .tags {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .tags li {
    padding: 1px 7px;
    border-radius: 10px;
    background: var(--raised);
    border: 1px solid var(--border);
    font-size: 11px;
    color: var(--text-dim);
  }
  .error {
    margin: 0;
    font-size: 12px;
    color: var(--danger);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  footer {
    display: flex;
    gap: 6px;
    margin-top: auto;
    padding-top: 4px;
  }
  footer .btn {
    flex: 1;
    height: 30px;
    font-size: 13px;
  }
  .glyph {
    font-size: 11px;
    color: var(--accent);
  }
</style>
