<!-- Find bar over a terminal: Enter goes to the next match, Shift+Enter to the previous one. -->
<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import type { ISearchOptions, SearchAddon } from "@xterm/addon-search";
  import { t } from "../i18n/index.svelte";
  import { settings } from "../stores/settings.svelte";
  import { searchColors } from "../terminal/themes";
  import Icon from "./Icon.svelte";

  let { addon, onclose }: { addon: SearchAddon; onclose: () => void } = $props();

  let input: HTMLInputElement;
  let query = $state("");
  let caseSensitive = $state(false);
  let regex = $state(false);
  let result = $state({ index: -1, count: 0 });
  let invalid = $state(false);

  onMount(() => {
    input.focus();
    const sub = addon.onDidChangeResults(({ resultIndex, resultCount }) => {
      result = { index: resultIndex, count: resultCount };
    });
    return () => sub.dispose();
  });
  onDestroy(() => addon.clearDecorations());

  function options(incremental = false): ISearchOptions {
    return { caseSensitive, regex, incremental, decorations: searchColors[settings.resolvedTheme] };
  }

  function validRegex(): boolean {
    if (!regex) return true;
    try {
      new RegExp(query);
      return true;
    } catch {
      return false;
    }
  }

  function find(direction: "next" | "previous", incremental = false) {
    invalid = !validRegex();
    if (!query || invalid) {
      addon.clearDecorations();
      result = { index: -1, count: 0 };
      return;
    }
    if (direction === "next") addon.findNext(query, options(incremental));
    else addon.findPrevious(query, options());
  }

  /** Focuses the field again when the shortcut is pressed while the bar is open. */
  export function focus() {
    input.focus();
    input.select();
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      onclose();
    } else if (e.key === "Enter") {
      e.preventDefault();
      find(e.shiftKey ? "previous" : "next");
    }
  }

  function toggle(which: "case" | "regex") {
    if (which === "case") caseSensitive = !caseSensitive;
    else regex = !regex;
    find("next", true);
    input.focus();
  }
</script>

<div class="search" role="search">
  <Icon name="search" size={14} />
  <input
    bind:this={input}
    class:invalid
    bind:value={query}
    placeholder={t("search.placeholder")}
    spellcheck="false"
    oninput={() => find("next", true)}
    {onkeydown}
  />
  <span class="count mono">
    {#if query && !invalid}
      {result.count ? `${result.index + 1}/${result.count}` : t("search.none")}
    {/if}
  </span>
  <button class="opt mono" class:on={caseSensitive} title={t("search.case")} onclick={() => toggle("case")}>Aa</button>
  <button class="opt mono" class:on={regex} title={t("search.regex")} onclick={() => toggle("regex")}>.*</button>
  <button class="icon" title={t("search.previous")} onclick={() => find("previous")}><Icon name="up" size={14} /></button>
  <button class="icon" title={t("search.next")} onclick={() => find("next")}><Icon name="down" size={14} /></button>
  <button class="icon" title={t("search.close")} onclick={onclose}><Icon name="x" size={14} /></button>
</div>

<style>
  .search {
    position: absolute;
    top: 10px;
    right: 18px;
    z-index: 10;
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 4px 4px 4px 10px;
    color: var(--text-faint);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    box-shadow: var(--shadow);
  }
  input {
    width: 200px;
    height: 26px;
    padding: 0 6px;
    border: 0;
    outline: 0;
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: 13px;
  }
  input.invalid {
    color: var(--danger);
  }
  .count {
    min-width: 46px;
    font-size: 11px;
    text-align: right;
    color: var(--text-faint);
  }
  button {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border: 0;
    border-radius: 5px;
    background: transparent;
    color: var(--text-dim);
  }
  button:hover {
    background: var(--hover);
    color: var(--text);
  }
  .opt {
    font-size: 11px;
  }
  .opt.on {
    color: var(--accent);
    background: var(--accent-soft);
  }
</style>
