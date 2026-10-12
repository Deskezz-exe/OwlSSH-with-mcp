<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { Terminal } from "@xterm/xterm";
  import { FitAddon } from "@xterm/addon-fit";
  import { WebglAddon } from "@xterm/addon-webgl";
  import { Unicode11Addon } from "@xterm/addon-unicode11";
  import { WebLinksAddon } from "@xterm/addon-web-links";
  import { SearchAddon } from "@xterm/addon-search";
  import { Channel } from "@tauri-apps/api/core";
  import { save } from "@tauri-apps/plugin-dialog";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import "@xterm/xterm/css/xterm.css";

  import { api, errorText, type TerminalExit } from "../api";
  import { t } from "../i18n/index.svelte";
  import { hosts } from "../stores/hosts.svelte";
  import { settings } from "../stores/settings.svelte";
  import { matches } from "../keybindings";
  import { terminalThemes } from "../terminal/themes";
  import { copyText, pasteText } from "../terminal/clipboard";
  import { openLink } from "../terminal/links";
  import ContextMenu from "../components/ContextMenu.svelte";
  import TerminalSearch from "../components/TerminalSearch.svelte";
  import { palette, terminalPaste } from "../stores/snippets.svelte";
  import { tabs } from "../stores/tabs.svelte";
  import { toast } from "../stores/toast.svelte";

  let { tabId, hostId, active }: { tabId: string; hostId: string; active: boolean } = $props();

  let container: HTMLDivElement;
  let term = $state.raw<Terminal>();
  let fit: FitAddon | undefined;
  let search = $state.raw<SearchAddon>();
  let searchBar = $state<ReturnType<typeof TerminalSearch>>();
  let searching = $state(false);
  let sessionId: number | null = null;
  let unlisten: UnlistenFn | undefined;
  let resizeObserver: ResizeObserver | undefined;

  let ended = $state<{ code: number | null; lost: boolean } | null>(null);
  /** Pending automatic reconnect after a dropped connection. */
  let retry = $state<{ attempt: number; left: number } | null>(null);
  let retryTimer = 0;
  /** Seconds to wait before each automatic attempt; then it gives up. */
  const RETRY_DELAYS = [2, 4, 8, 15, 30, 30, 30, 30, 30, 30];
  let failure = $state("");
  let menu = $state<{ x: number; y: number } | null>(null);
  /** Hint next to a hovered link. */
  let linkHint = $state<{ x: number; y: number } | null>(null);
  /** Visual bell in the tab you are looking at. */
  let flash = $state(false);
  let flashTimer = 0;

  const termSettings = $derived(settings.value?.terminal);
  const keys = $derived(settings.value?.keybindings);
  const themeName = $derived(settings.resolvedTheme);

  const MIN_FONT = 9;
  const MAX_FONT = 28;
  const DEFAULT_FONT = 14;
  let saveFontTimer = 0;

  /** The chosen font first, the bundled one as the fallback. */
  function fontStack(custom: string | undefined): string {
    const base = '"JetBrains Mono", "Cascadia Mono", Consolas, monospace';
    const name = custom?.trim().replace(/["\\]/g, "");
    return name ? `"${name}", ${base}` : base;
  }

  onMount(async () => {
    // Measure cells with the real font, not a fallback that is still loading.
    await document.fonts.load(`${termSettings?.fontSize ?? 14}px "JetBrains Mono"`).catch(() => {});

    term = new Terminal({
      fontFamily: fontStack(termSettings?.fontFamily),
      fontSize: termSettings?.fontSize ?? 14,
      lineHeight: 1.15,
      cursorBlink: termSettings?.cursorBlink ?? true,
      scrollback: termSettings?.scrollback ?? 10000,
      theme: terminalThemes[themeName],
      allowProposedApi: true,
      macOptionIsMeta: true,
      rightClickSelectsWord: false,
    });
    fit = new FitAddon();
    term.loadAddon(fit);
    // Unicode 11 widths keep emoji and CJK text aligned in prompts and TUIs.
    term.loadAddon(new Unicode11Addon());
    term.unicode.activeVersion = "11";
    // Ctrl+click opens a link, a plain click still selects text.
    term.loadAddon(
      new WebLinksAddon(
        (e, uri) => {
          if (e.ctrlKey || e.metaKey) openLink(uri);
        },
        {
          hover: (e) => (linkHint = { x: e.clientX, y: e.clientY }),
          leave: () => (linkHint = null),
        },
      ),
    );
    search = new SearchAddon({ highlightLimit: 2000 });
    term.loadAddon(search);
    term.open(container);
    loadWebgl(term);
    fit.fit();

    term.attachCustomKeyEventHandler(onKey);
    term.onData((data) => {
      if (sessionId !== null) api.terminalWrite(sessionId, data);
      // Enter on an ended session starts a new one.
      else if (data === "\r" && (ended || failure)) reconnect();
    });
    term.onBinary((data) => sessionId !== null && api.terminalWrite(sessionId, data));
    term.onResize(({ cols, rows }) => sessionId !== null && api.terminalResize(sessionId, cols, rows));
    term.onTitleChange((title) => tabs.setTitle(tabId, title));
    term.onBell(() => {
      tabs.ring(tabId);
      if (active) flashBell();
    });
    term.onSelectionChange(() => {
      if (settings.value?.terminal.copyOnSelect && term?.hasSelection()) copyText(term.getSelection());
    });

    // Refit when the panel changes size; one fit per frame at most.
    let frame = 0;
    resizeObserver = new ResizeObserver(() => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => active && fit?.fit());
    });
    resizeObserver.observe(container);
    container.addEventListener("wheel", onWheel, { passive: false, capture: true });

    unlisten = await listen<TerminalExit>("terminal-exit", ({ payload }) => {
      if (payload.id === sessionId) {
        sessionId = null;
        tabs.setLive(tabId, false);
        ended = { code: payload.exitCode, lost: payload.lost };
        hosts.load();
        if (payload.lost && settings.value?.terminal.autoReconnect) scheduleRetry(0);
      }
    });

    terminalPaste.set(tabId, (text) => {
      term?.paste(text);
      term?.focus();
    });

    await start();
  });

  onDestroy(() => {
    terminalPaste.delete(tabId);
    unlisten?.();
    resizeObserver?.disconnect();
    clearTimeout(saveFontTimer);
    clearInterval(retryTimer);
    clearTimeout(flashTimer);
    if (sessionId !== null) api.terminalClose(sessionId);
    term?.dispose();
  });

  /** WebGL draws much faster than the DOM renderer; fall back if the GPU context is lost. */
  function loadWebgl(terminal: Terminal) {
    try {
      const webgl = new WebglAddon();
      webgl.onContextLoss(() => webgl.dispose());
      terminal.loadAddon(webgl);
    } catch {
      // DOM renderer stays in use.
    }
  }

  /** Opens a shell. `again` keeps the old output and marks where the new session starts. */
  async function start(again = false) {
    if (!term) return;
    ended = null;
    failure = "";
    try {
      if (!(await hosts.connect(hostId))) {
        failure = hosts.errors[hostId] ?? t("status.error");
        return;
      }
      if (again) {
        // Leave whatever full-screen mode the old shell was in (vim, htop, mouse
        // tracking), then draw a divider so the old output stays readable.
        // 1049l also restores the saved cursor, so only send it from the alternate screen.
        if (term.buffer.active.type === "alternate") term.write("\x1b[?1049l");
        term.write("\x1b[?1000l\x1b[?1002l\x1b[?1003l\x1b[?1006l\x1b[?2004l\x1b[?1l\x1b[!p");
        term.write(`\r\n\x1b[2m── ${t("terminal.newSession")} ──\x1b[0m\r\n`);
      }
      const output = new Channel<ArrayBuffer>();
      output.onmessage = (data) => term?.write(new Uint8Array(data));
      sessionId = await api.terminalOpen(hostId, term.cols, term.rows, output);
      tabs.setLive(tabId, true);
      if (active) term.focus();
    } catch (e) {
      failure = errorText(e);
      hosts.load();
    }
  }

  /** Reconnects now; while auto-reconnecting, a failure schedules the next attempt. */
  async function reconnect() {
    const attempt = retry?.attempt;
    cancelRetry();
    await start(true);
    // A cancelled password prompt leaves the host idle: stop asking.
    if (attempt !== undefined && failure && hosts.stateOf(hostId) !== "idle") scheduleRetry(attempt + 1);
  }

  function scheduleRetry(attempt: number) {
    cancelRetry();
    if (attempt >= RETRY_DELAYS.length) return;
    retry = { attempt, left: RETRY_DELAYS[attempt] };
    retryTimer = window.setInterval(() => {
      if (!retry) return;
      retry.left -= 1;
      if (retry.left <= 0) reconnect();
    }, 1000);
  }

  function cancelRetry() {
    clearInterval(retryTimer);
    retry = null;
  }

  /** Copy and paste shortcuts from settings. Returning false keeps the keys from the shell. */
  function onKey(e: KeyboardEvent): boolean {
    if (e.type !== "keydown" || !keys) return true;
    // Ctrl+K belongs to the shell here (kill line); Ctrl+Shift+K opens the palette.
    if ((e.ctrlKey || e.metaKey) && e.code === "KeyK") {
      if (e.shiftKey) {
        e.preventDefault();
        palette.open = true;
        return false;
      }
      e.stopPropagation();
      return true;
    }
    if ((e.ctrlKey || e.metaKey) && e.shiftKey && !e.altKey && e.code === "KeyF") {
      e.preventDefault();
      openSearch();
      return false;
    }
    if ((e.ctrlKey || e.metaKey) && !e.altKey) {
      const step =
        e.code === "Equal" || e.code === "NumpadAdd" ? 1
        : e.code === "Minus" || e.code === "NumpadSubtract" ? -1
        : (e.code === "Digit0" || e.code === "Numpad0") && !e.shiftKey ? 0
        : null;
      if (step !== null) {
        e.preventDefault();
        zoom(step);
        return false;
      }
    }
    if (matches(e, keys.copy)) {
      // A plain Ctrl+C binding still interrupts the shell when nothing is selected.
      if (!term?.hasSelection()) return true;
      e.preventDefault();
      copySelection();
      return false;
    }
    if (matches(e, keys.paste)) {
      e.preventDefault();
      paste();
      return false;
    }
    return true;
  }

  /** Ctrl+wheel zooms like a browser; plain wheel scrolls as usual. */
  function onWheel(e: WheelEvent) {
    if (!(e.ctrlKey || e.metaKey) || e.deltaY === 0) return;
    e.preventDefault();
    e.stopPropagation();
    zoom(e.deltaY < 0 ? 1 : -1);
  }

  /** Changes the font size right away and saves it once the user stops. 0 resets it. */
  function zoom(step: number) {
    if (!term) return;
    const current = term.options.fontSize ?? DEFAULT_FONT;
    const size = step === 0 ? DEFAULT_FONT : Math.min(MAX_FONT, Math.max(MIN_FONT, current + step));
    if (size === current) return;
    term.options.fontSize = size;
    fit?.fit();
    clearTimeout(saveFontTimer);
    saveFontTimer = window.setTimeout(() => settings.update((x) => (x.terminal.fontSize = size)), 400);
  }

  /** The scrollback and the screen as plain text, without trailing blank lines. */
  function outputText(): string {
    const buffer = term!.buffer.normal;
    const lines: string[] = [];
    for (let i = 0; i < buffer.length; i++) {
      const line = buffer.getLine(i);
      if (!line) continue;
      // Wrapped rows continue the previous line.
      if (line.isWrapped && lines.length) lines[lines.length - 1] += line.translateToString(true);
      else lines.push(line.translateToString(true));
    }
    while (lines.length && !lines[lines.length - 1]) lines.pop();
    return lines.join("\n") + "\n";
  }

  async function saveOutput() {
    if (!term) return;
    const d = new Date();
    const pad = (n: number) => String(n).padStart(2, "0");
    const stamp = `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}-${pad(d.getHours())}${pad(d.getMinutes())}`;
    const name = `${hosts.byId(hostId)?.name ?? "terminal"}-${stamp}.txt`.replace(/[\\/:*?"<>|]/g, "_");
    try {
      const path = await save({ defaultPath: name, filters: [{ name: "Text", extensions: ["txt", "log"] }] });
      if (!path) return;
      await api.saveTerminalOutput(path, outputText());
      toast.show(t("terminal.saved"));
    } catch (e) {
      toast.show(errorText(e), "error");
    }
  }

  function flashBell() {
    flash = true;
    clearTimeout(flashTimer);
    flashTimer = window.setTimeout(() => (flash = false), 180);
  }

  function openSearch() {
    if (searching) searchBar?.focus();
    else searching = true;
  }

  function closeSearch() {
    searching = false;
    term?.focus();
  }

  function copySelection() {
    if (term?.hasSelection()) copyText(term.getSelection());
  }

  async function paste() {
    const text = await pasteText();
    // term.paste wraps the text in bracketed-paste markers when the shell asks for it.
    if (text) term?.paste(text);
    term?.focus();
  }

  function oncontextmenu(e: MouseEvent) {
    e.preventDefault();
    if (settings.value?.terminal.rightClick === "paste") {
      if (term?.hasSelection()) {
        copySelection();
        term.clearSelection();
      } else {
        paste();
      }
      return;
    }
    menu = { x: e.clientX, y: e.clientY };
  }

  // Become visible: refit to the panel and take keyboard focus.
  $effect(() => {
    if (active && term) {
      tabs.seen(tabId);
      requestAnimationFrame(() => {
        fit?.fit();
        term?.focus();
      });
    }
  });

  // Live settings changes.
  $effect(() => {
    if (!term || !termSettings) return;
    term.options.fontSize = termSettings.fontSize;
    term.options.fontFamily = fontStack(termSettings.fontFamily);
    term.options.cursorBlink = termSettings.cursorBlink;
    term.options.scrollback = termSettings.scrollback;
    fit?.fit();
  });

  $effect(() => {
    if (term) term.options.theme = terminalThemes[themeName];
  });
</script>

<div class="wrap">
  <div class="term" bind:this={container} {oncontextmenu} role="presentation"></div>
  {#if flash}<div class="flash"></div>{/if}

  {#if searching && search}
    <TerminalSearch bind:this={searchBar} addon={search} onclose={closeSearch} />
  {/if}

  {#if linkHint}
    <div class="link-hint" style:left="{linkHint.x + 12}px" style:top="{linkHint.y + 16}px">
      {t("terminal.openLink")}
    </div>
  {/if}

  {#if ended || failure}
    <div class="banner" class:error={!!failure}>
      <span>
        {#if failure}{failure}{:else if ended?.lost}{t("terminal.lost")}{:else if ended?.code !== null && ended?.code !== undefined}{t("terminal.closedCode", { code: ended.code })}{:else}{t("terminal.closed")}{/if}
      </span>
      {#if retry}
        <span class="retry">{t("terminal.retryIn", { seconds: retry.left })}</span>
        <button class="btn ghost" onclick={cancelRetry}>{t("common.cancel")}</button>
      {/if}
      <button class="btn primary" onclick={reconnect}>{t("terminal.reconnect")}</button>
    </div>
  {/if}
</div>

{#if menu}
  <ContextMenu
    x={menu.x}
    y={menu.y}
    onclose={() => {
      menu = null;
      term?.focus();
    }}
    items={[
      { label: `${t("terminal.copy")}  ·  ${keys?.copy ?? ""}`, action: copySelection, disabled: !term?.hasSelection() },
      { label: `${t("terminal.paste")}  ·  ${keys?.paste ?? ""}`, action: paste },
      { label: `${t("terminal.find")}  ·  Ctrl+Shift+F`, action: openSearch, separatorBefore: true },
      { label: t("terminal.selectAll"), action: () => term?.selectAll() },
      { label: t("terminal.saveOutput"), action: saveOutput },
      { label: t("terminal.clear"), action: () => term?.clear() },
    ]}
  />
{/if}

<style>
  .wrap {
    position: relative;
    height: 100%;
    background: var(--term-bg);
  }
  .term {
    position: absolute;
    inset: 8px 4px 4px 12px;
  }
  .term :global(.xterm-viewport) {
    background: transparent !important;
  }
  .flash {
    position: absolute;
    inset: 0;
    pointer-events: none;
    box-shadow: inset 0 0 0 2px var(--accent);
    background: var(--accent-soft);
  }
  .link-hint {
    position: fixed;
    z-index: 20;
    padding: 3px 8px;
    font-size: 12px;
    color: var(--text-dim);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 6px;
    box-shadow: var(--shadow);
    pointer-events: none;
  }
  .banner {
    position: absolute;
    left: 50%;
    bottom: 18px;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 14px;
    max-width: calc(100% - 32px);
    padding: 8px 8px 8px 16px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    box-shadow: var(--shadow);
  }
  .banner .retry {
    color: var(--text-faint);
    font-variant-numeric: tabular-nums;
  }
  .banner.error span:first-child {
    color: var(--danger);
  }
  .banner span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
