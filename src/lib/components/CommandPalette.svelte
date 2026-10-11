<!-- Ctrl+K: jump to a server, an open tab, a screen, or paste a snippet. -->
<script lang="ts">
  import { t } from "../i18n/index.svelte";
  import { hosts } from "../stores/hosts.svelte";
  import { tabs } from "../stores/tabs.svelte";
  import { palette, snippets, terminalPaste } from "../stores/snippets.svelte";
  import { shownAddress } from "../privacy";

  interface Item {
    group: string;
    label: string;
    hint: string;
    run: () => void;
  }

  let query = $state("");
  let index = $state(0);
  let input: HTMLInputElement | undefined = $state();

  $effect(() => {
    if (palette.open) {
      query = "";
      index = 0;
      requestAnimationFrame(() => input?.focus());
    }
  });

  /** Subsequence match: every typed character in order. Lower score is better. */
  function score(text: string, q: string): number | null {
    if (!q) return 0;
    const hay = text.toLowerCase();
    const direct = hay.indexOf(q);
    if (direct >= 0) return direct;
    let pos = -1;
    let gaps = 0;
    for (const ch of q) {
      const next = hay.indexOf(ch, pos + 1);
      if (next < 0) return null;
      gaps += next - pos - 1;
      pos = next;
    }
    return 100 + gaps;
  }

  const activeTerminal = $derived(tabs.list.find((x) => x.id === tabs.active && x.kind === "terminal"));

  const items = $derived.by(() => {
    const q = query.trim().toLowerCase();
    const all: Item[] = [];
    for (const tab of tabs.list) {
      const host = hosts.byId(tab.hostId);
      all.push({
        group: t("palette.tabs"),
        label: `${tab.kind === "terminal" ? "❯_" : "⇄"}  ${host?.name ?? ""}`,
        hint: t(tab.kind === "terminal" ? "tabs.terminal" : "tabs.files"),
        run: () => tabs.show(tab.id),
      });
    }
    for (const host of hosts.sorted) {
      all.push({
        group: t("palette.hosts"),
        label: host.name,
        hint: `${host.username}@${shownAddress(host)}`,
        run: async () => {
          if (await hosts.connect(host.id)) tabs.focusOrOpen("terminal", host.id);
        },
      });
    }
    for (const s of snippets.list) {
      const paste = activeTerminal && terminalPaste.get(activeTerminal.id);
      all.push({
        group: t("palette.snippets"),
        label: s.name,
        hint: paste ? t("palette.paste") : s.command,
        run: () => (paste ? paste(s.command) : tabs.show("snippets")),
      });
    }
    const screens: [string, string][] = [
      ["dashboard", t("nav.dashboard")],
      ["snippets", t("nav.snippets")],
      ["mcp", t("nav.mcp")],
      ["settings", t("nav.settings")],
    ];
    for (const [id, label] of screens) {
      all.push({ group: t("palette.screens"), label, hint: "", run: () => tabs.show(id) });
    }

    return all
      .map((item) => ({ item, s: score(`${item.label} ${item.hint}`, q) }))
      .filter((x) => x.s !== null)
      .sort((a, b) => (q ? (a.s as number) - (b.s as number) : 0))
      .map((x) => x.item)
      .slice(0, 40);
  });

  function choose(item: Item | undefined) {
    if (!item) return;
    palette.open = false;
    item.run();
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Escape") palette.open = false;
    else if (e.key === "ArrowDown") {
      e.preventDefault();
      index = Math.min(items.length - 1, index + 1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      index = Math.max(0, index - 1);
    } else if (e.key === "Enter") {
      e.preventDefault();
      choose(items[index]);
    }
  }
</script>

{#if palette.open}
  <div class="backdrop" role="presentation" onmousedown={(e) => e.target === e.currentTarget && (palette.open = false)}>
    <div class="palette" role="dialog" aria-modal="true" aria-label={t("palette.placeholder")}>
      <input
        bind:this={input}
        bind:value={query}
        oninput={() => (index = 0)}
        {onkeydown}
        placeholder={t("palette.placeholder")}
        spellcheck="false"
      />
      <ul role="listbox">
        {#each items as item, i}
          {#if i === 0 || items[i - 1].group !== item.group}
            <li class="group">{item.group}</li>
          {/if}
          <li>
            <button
              class:active={i === index}
              role="option"
              aria-selected={i === index}
              onmouseenter={() => (index = i)}
              onclick={() => choose(item)}
            >
              <span class="label">{item.label}</span>
              <span class="hint mono">{item.hint}</span>
            </button>
          </li>
        {:else}
          <li class="none">{t("palette.empty")}</li>
        {/each}
      </ul>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 65;
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding-top: 12vh;
    background: rgba(5, 7, 12, 0.45);
  }
  .palette {
    width: min(620px, calc(100vw - 32px));
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 12px;
    box-shadow: var(--shadow);
    overflow: hidden;
  }
  input {
    width: 100%;
    height: 50px;
    padding: 0 18px;
    border: 0;
    border-bottom: 1px solid var(--border);
    background: transparent;
    font-size: 15px;
    outline: none;
  }
  ul {
    max-height: 52vh;
    overflow: auto;
    margin: 0;
    padding: 6px;
    list-style: none;
  }
  .group {
    padding: 8px 10px 4px;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-faint);
  }
  button {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    width: 100%;
    padding: 8px 10px;
    border: 0;
    border-radius: var(--radius-sm);
    background: transparent;
    text-align: left;
  }
  button.active {
    background: var(--accent-soft);
  }
  .label {
    font-weight: 500;
    white-space: nowrap;
  }
  .hint {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 12px;
    color: var(--text-dim);
  }
  .none {
    padding: 14px 10px;
    color: var(--text-faint);
  }
</style>
