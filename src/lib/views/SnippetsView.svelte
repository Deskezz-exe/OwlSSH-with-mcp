<script lang="ts">
  import { ask } from "@tauri-apps/plugin-dialog";
  import { t } from "../i18n/index.svelte";
  import { snippets } from "../stores/snippets.svelte";
  import { toast } from "../stores/toast.svelte";
  import { errorText, type Snippet } from "../api";
  import { paramsOf } from "../snippetParams";
  import RunSnippet from "../components/RunSnippet.svelte";

  let selectedId = $state<string | null>(snippets.list[0]?.id ?? null);
  let name = $state("");
  let command = $state("");
  let description = $state("");
  let running = $state<Snippet | null>(null);

  const selected = $derived(snippets.list.find((s) => s.id === selectedId) ?? null);
  const params = $derived(paramsOf(command));
  const dirty = $derived(
    !selected ? name !== "" || command !== "" : name !== selected.name || command !== selected.command || description !== selected.description,
  );

  // Load the chosen snippet into the editor.
  $effect(() => {
    name = selected?.name ?? "";
    command = selected?.command ?? "";
    description = selected?.description ?? "";
  });

  async function save(e: SubmitEvent) {
    e.preventDefault();
    try {
      const saved = await snippets.save({ id: selected?.id ?? null, name, command, description });
      selectedId = saved.id;
    } catch (err) {
      toast.show(errorText(err), "error");
    }
  }

  async function remove() {
    if (!selected) return;
    if (!(await ask(t("snippets.deleteConfirm", { name: selected.name }), { kind: "warning" }))) return;
    await snippets.remove(selected.id);
    selectedId = snippets.list[0]?.id ?? null;
  }
</script>

<div class="page">
  <header>
    <div>
      <h1>{t("snippets.title")}</h1>
      <p class="intro">{t("snippets.intro")}</p>
    </div>
    <button class="btn primary" onclick={() => (selectedId = null)}>+ {t("snippets.new")}</button>
  </header>

  <div class="layout">
    <ul class="list">
      {#each snippets.list as s (s.id)}
        <li>
          <button class="item" class:active={s.id === selectedId} onclick={() => (selectedId = s.id)}>
            <span class="name">{s.name}</span>
            <span class="cmd mono">{s.command}</span>
          </button>
        </li>
      {:else}
        <li class="empty">{t("snippets.empty")}</li>
      {/each}
    </ul>

    <form class="editor" onsubmit={save}>
      <label class="field">
        <span>{t("snippets.name")}</span>
        <input class="input" bind:value={name} required />
      </label>
      <label class="field">
        <span>{t("snippets.command")}</span>
        <textarea class="input mono code" bind:value={command} required spellcheck="false" rows="5"></textarea>
      </label>
      {#if params.length}
        <p class="params">{#each params as p}<code class="mono">{p}</code>{/each}</p>
      {/if}
      <label class="field">
        <span>{t("snippets.description")}</span>
        <input class="input" bind:value={description} />
      </label>
      <div class="actions">
        {#if selected}
          <button type="button" class="btn danger" onclick={remove}>{t("snippets.delete")}</button>
        {/if}
        <span class="spacer"></span>
        <button type="submit" class="btn" disabled={!dirty}>{t("snippets.save")}</button>
        {#if selected}
          <button type="button" class="btn primary" onclick={() => (running = selected)}>▶ {t("snippets.run")}</button>
        {/if}
      </div>
      <p class="hint">{t("snippets.pasteHint")}</p>
    </form>
  </div>
</div>

{#if running}
  <RunSnippet snippet={running} onclose={() => (running = null)} />
{/if}

<style>
  .page {
    height: 100%;
    display: flex;
    flex-direction: column;
    padding: 22px 26px 24px;
    min-height: 0;
  }
  header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 20px;
    margin-bottom: 18px;
  }
  h1 {
    margin: 0 0 6px;
    font-family: var(--font-display);
    font-weight: 500;
    font-size: 20px;
  }
  .intro {
    margin: 0;
    max-width: 620px;
    color: var(--text-dim);
  }
  .layout {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(220px, 320px) minmax(0, 1fr);
    gap: 14px;
  }
  .list {
    margin: 0;
    padding: 6px;
    list-style: none;
    overflow: auto;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
  }
  .item {
    display: grid;
    gap: 2px;
    width: 100%;
    padding: 8px 10px;
    border: 0;
    border-radius: var(--radius-sm);
    background: transparent;
    text-align: left;
  }
  .item:hover {
    background: var(--hover);
  }
  .item.active {
    background: var(--accent-soft);
  }
  .name {
    font-weight: 500;
  }
  .cmd {
    font-size: 11.5px;
    color: var(--text-dim);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .empty {
    padding: 10px;
    color: var(--text-faint);
  }
  .editor {
    display: grid;
    align-content: start;
    gap: 14px;
    padding: 16px 18px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    overflow: auto;
  }
  .code {
    height: auto;
    padding: 8px 10px;
    resize: vertical;
    font-size: 13px;
    line-height: 1.5;
  }
  .params {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin: -6px 0 0;
  }
  .params code {
    padding: 1px 8px;
    border-radius: 10px;
    background: var(--accent-soft);
    color: var(--accent);
    font-size: 12px;
  }
  .actions {
    display: flex;
    gap: 8px;
  }
  .spacer {
    flex: 1;
  }
  .hint {
    margin: 0;
    font-size: 12px;
    color: var(--text-faint);
  }
</style>
