<script lang="ts">
  import { getVersion } from "@tauri-apps/api/app";
  import { onMount } from "svelte";
  import { t, LOCALES } from "../i18n/index.svelte";
  import { settings } from "../stores/settings.svelte";
  import { sameCombo } from "../keybindings";
  import ShortcutInput from "../components/ShortcutInput.svelte";

  const s = $derived(settings.value!);
  let version = $state("");

  onMount(async () => {
    version = await getVersion().catch(() => "");
  });

  const themes = ["dark", "light", "system"] as const;
  const DEFAULT_KEYS = { copy: "Ctrl+Shift+C", paste: "Ctrl+Shift+V" };

  function validateShortcut(which: "copy" | "paste") {
    return (combo: string) => {
      const other = which === "copy" ? s.keybindings.paste : s.keybindings.copy;
      return sameCombo(combo, other) ? t("settings.shortcutConflict") : "";
    };
  }
</script>

<div class="page">
  <h1>{t("settings.title")}</h1>

  <section>
    <h2>{t("settings.general")}</h2>

    <div class="row">
      <span class="label">{t("settings.language")}</span>
      <div class="segmented">
        {#each LOCALES as locale}
          <button
            class:on={s.locale === locale.id}
            onclick={() => settings.update((x) => (x.locale = locale.id))}>{locale.label}</button
          >
        {/each}
      </div>
    </div>

    <div class="row">
      <span class="label">{t("settings.theme")}</span>
      <div class="segmented">
        {#each themes as theme}
          <button class:on={s.theme === theme} onclick={() => settings.update((x) => (x.theme = theme))}>
            {t(`settings.theme.${theme}`)}
          </button>
        {/each}
      </div>
    </div>

    <label class="row">
      <span class="label">
        {t("settings.streamer")}
        <small>{t("settings.streamerHint")}</small>
      </span>
      <input
        type="checkbox"
        class="switch"
        checked={s.streamerMode}
        onchange={(e) => settings.update((x) => (x.streamerMode = e.currentTarget.checked))}
      />
    </label>
  </section>

  <section>
    <h2>{t("settings.terminal")}</h2>

    <label class="row">
      <span class="label">{t("settings.fontSize")}</span>
      <input
        class="input number"
        type="number"
        min="9"
        max="28"
        value={s.terminal.fontSize}
        onchange={(e) =>
          settings.update((x) => (x.terminal.fontSize = Math.min(28, Math.max(9, Number(e.currentTarget.value) || 14))))}
      />
    </label>

    <label class="row">
      <span class="label">{t("settings.scrollback")}</span>
      <input
        class="input number"
        type="number"
        min="500"
        max="200000"
        step="500"
        value={s.terminal.scrollback}
        onchange={(e) =>
          settings.update(
            (x) => (x.terminal.scrollback = Math.min(200000, Math.max(500, Number(e.currentTarget.value) || 10000))),
          )}
      />
    </label>

    <label class="row">
      <span class="label">{t("settings.cursorBlink")}</span>
      <input
        type="checkbox"
        class="switch"
        checked={s.terminal.cursorBlink}
        onchange={(e) => settings.update((x) => (x.terminal.cursorBlink = e.currentTarget.checked))}
      />
    </label>

    <label class="row">
      <span class="label">{t("settings.copyOnSelect")}</span>
      <input
        type="checkbox"
        class="switch"
        checked={s.terminal.copyOnSelect}
        onchange={(e) => settings.update((x) => (x.terminal.copyOnSelect = e.currentTarget.checked))}
      />
    </label>

    <div class="row">
      <span class="label">{t("settings.rightClick")}</span>
      <div class="segmented">
        {#each ["menu", "paste"] as const as mode}
          <button
            class:on={s.terminal.rightClick === mode}
            onclick={() => settings.update((x) => (x.terminal.rightClick = mode))}
          >
            {t(`settings.rightClick.${mode}`)}
          </button>
        {/each}
      </div>
    </div>
  </section>

  <section>
    <h2>{t("settings.shortcuts")}</h2>
    <p class="hint">{t("settings.shortcutsHint")}</p>

    {#each ["copy", "paste"] as const as action}
      <div class="row">
        <span class="label">{t(`settings.${action}`)}</span>
        <div class="shortcut">
          <ShortcutInput
            value={s.keybindings[action]}
            validate={validateShortcut(action)}
            onchange={(combo) => settings.update((x) => (x.keybindings[action] = combo))}
          />
          {#if s.keybindings[action] !== DEFAULT_KEYS[action]}
            <button
              class="btn ghost reset"
              onclick={() => settings.update((x) => (x.keybindings[action] = DEFAULT_KEYS[action]))}
              >{t("settings.reset")}</button
            >
          {/if}
        </div>
      </div>
    {/each}
  </section>

  {#if version}<p class="version">OwlSSH {version}</p>{/if}
</div>

<style>
  .page {
    height: 100%;
    overflow: auto;
    padding: 22px 26px 40px;
  }
  .page > * {
    max-width: 680px;
  }
  h1 {
    margin: 0 0 20px;
    font-family: var(--font-display);
    font-weight: 500;
    font-size: 20px;
  }
  section {
    margin-bottom: 26px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 6px 18px;
  }
  h2 {
    margin: 12px 0 4px;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-faint);
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 20px;
    min-height: 54px;
    padding: 8px 0;
  }
  .row + .row {
    border-top: 1px solid var(--border);
  }
  .label {
    display: grid;
    gap: 2px;
    font-weight: 500;
  }
  small,
  .hint {
    font-weight: 400;
    font-size: 12px;
    color: var(--text-faint);
  }
  .hint {
    margin: 0 0 4px;
  }
  .number {
    width: 110px;
    text-align: right;
  }
  .segmented {
    display: flex;
    padding: 3px;
    gap: 3px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--bg);
  }
  .segmented button {
    min-width: 84px;
    height: 28px;
    padding: 0 12px;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: var(--text-dim);
    font-weight: 500;
  }
  .segmented button.on {
    background: var(--raised);
    color: var(--text);
    box-shadow: inset 0 -2px 0 var(--accent);
  }
  .switch {
    appearance: none;
    flex: none;
    width: 38px;
    height: 22px;
    margin: 0;
    border-radius: 11px;
    background: var(--border);
    position: relative;
    cursor: pointer;
    transition: background 0.15s;
  }
  .switch::after {
    content: "";
    position: absolute;
    top: 3px;
    left: 3px;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: var(--text);
    transition: transform 0.15s;
  }
  .switch:checked {
    background: var(--accent);
  }
  .switch:checked::after {
    transform: translateX(16px);
    background: var(--accent-text);
  }
  .shortcut {
    display: flex;
    align-items: flex-start;
    gap: 6px;
  }
  .reset {
    height: 34px;
    color: var(--text-dim);
  }
  .version {
    color: var(--text-faint);
    font-size: 12px;
  }
</style>
