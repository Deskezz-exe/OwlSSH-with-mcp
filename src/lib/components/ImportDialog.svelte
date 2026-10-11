<script lang="ts">
  import { onMount } from "svelte";
  import { t } from "../i18n/index.svelte";
  import { hosts } from "../stores/hosts.svelte";
  import { api, errorText, type ImportCandidate } from "../api";
  import Modal from "./Modal.svelte";

  let { onclose, ondone }: { onclose: () => void; ondone: (count: number) => void } = $props();

  let candidates = $state<ImportCandidate[] | null>(null);
  let selected = $state<Record<string, boolean>>({});
  let error = $state("");
  let busy = $state(false);

  const available = $derived((candidates ?? []).filter((c) => !c.alreadyAdded));
  const count = $derived(available.filter((c) => selected[c.alias]).length);

  onMount(async () => {
    try {
      candidates = await api.previewSshConfig();
      for (const c of candidates) selected[c.alias] = !c.alreadyAdded;
    } catch (e) {
      error = errorText(e);
      candidates = [];
    }
  });

  function toggleAll() {
    const all = count < available.length;
    for (const c of available) selected[c.alias] = all;
  }

  async function submit() {
    busy = true;
    try {
      const n = await hosts.importAliases(available.filter((c) => selected[c.alias]).map((c) => c.alias));
      ondone(n);
      onclose();
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = false;
    }
  }
</script>

<Modal title={t("import.title")} {onclose} width={520}>
  {#if candidates === null}
    <p class="muted">…</p>
  {:else if candidates.length === 0}
    <p class="muted">{t("import.empty")}</p>
  {:else}
    {#if available.length > 1}
      <button class="btn ghost all" onclick={toggleAll}>{t("import.selectAll")}</button>
    {/if}
    <ul>
      {#each candidates as c (c.alias)}
        <li class:done={c.alreadyAdded}>
          <label>
            <input type="checkbox" bind:checked={selected[c.alias]} disabled={c.alreadyAdded} />
            <span class="alias">{c.alias}</span>
            <span class="target mono">{c.user ? `${c.user}@` : ""}{c.hostname}{c.port !== 22 ? `:${c.port}` : ""}</span>
            {#if c.alreadyAdded}<span class="tag">{t("import.already")}</span>{/if}
          </label>
        </li>
      {/each}
    </ul>
  {/if}
  {#if error}<p class="error" role="alert">{error}</p>{/if}

  {#snippet footer()}
    <button class="btn ghost" onclick={onclose}>{t("common.cancel")}</button>
    <button class="btn primary" disabled={!count || busy} onclick={submit}>{t("import.submit", { count })}</button>
  {/snippet}
</Modal>

<style>
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    overflow: hidden;
  }
  li + li {
    border-top: 1px solid var(--border);
  }
  label {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 9px 12px;
    cursor: pointer;
  }
  .done label {
    cursor: default;
    opacity: 0.6;
  }
  .alias {
    font-weight: 500;
  }
  .target {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--text-dim);
    font-size: 12px;
  }
  .tag {
    font-size: 11px;
    color: var(--text-faint);
  }
  .all {
    margin: 0 0 8px -8px;
    height: 28px;
  }
  .muted {
    color: var(--text-dim);
  }
  .error {
    color: var(--danger);
    font-size: 13px;
  }
</style>
