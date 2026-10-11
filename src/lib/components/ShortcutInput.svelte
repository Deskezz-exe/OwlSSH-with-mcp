<!-- Click, then press the new keys. Esc cancels, Backspace alone is ignored. -->
<script lang="ts">
  import { t } from "../i18n/index.svelte";
  import { comboFromEvent, hasModifier } from "../keybindings";

  let {
    value,
    onchange,
    validate,
  }: {
    value: string;
    onchange: (combo: string) => void;
    /** Returns an error message, or "" when the shortcut is acceptable. */
    validate: (combo: string) => string;
  } = $props();

  let recording = $state(false);
  let error = $state("");

  function onkeydown(e: KeyboardEvent) {
    if (!recording) return;
    e.preventDefault();
    e.stopPropagation();
    if (e.key === "Escape") {
      recording = false;
      error = "";
      return;
    }
    const combo = comboFromEvent(e);
    if (!combo) return;
    if (!hasModifier(combo)) {
      error = t("settings.shortcutNeedsModifier");
      return;
    }
    const problem = validate(combo);
    if (problem) {
      error = problem;
      return;
    }
    error = "";
    recording = false;
    onchange(combo);
  }
</script>

<div class="wrap">
  <button
    class="key mono"
    class:recording
    onclick={() => {
      recording = !recording;
      error = "";
    }}
    onblur={() => (recording = false)}
    {onkeydown}
  >
    {#if recording}
      <span class="hint">{t("settings.pressKeys")}</span>
    {:else}
      {#each value.split("+") as part, i}
        {#if i > 0}<span class="plus">+</span>{/if}<kbd>{part}</kbd>
      {/each}
    {/if}
  </button>
  {#if error}<p class="error" role="alert">{error}</p>{/if}
</div>

<style>
  .wrap {
    display: grid;
    gap: 4px;
    justify-items: end;
  }
  .key {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    min-width: 170px;
    height: 34px;
    padding: 0 10px;
    justify-content: center;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--bg);
    font-size: 12px;
  }
  .key:hover {
    border-color: var(--text-faint);
  }
  .key.recording {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-soft);
  }
  kbd {
    padding: 2px 6px;
    border-radius: 4px;
    background: var(--raised);
    border: 1px solid var(--border);
    border-bottom-width: 2px;
    font-family: inherit;
  }
  .plus {
    color: var(--text-faint);
  }
  .hint {
    color: var(--accent);
  }
  .error {
    margin: 0;
    font-size: 12px;
    color: var(--danger);
  }
</style>
