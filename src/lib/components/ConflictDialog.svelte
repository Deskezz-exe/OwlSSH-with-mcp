<!-- Asked before a transfer would overwrite something: replace, rename or cancel. -->
<script lang="ts">
  import { tick } from "svelte";
  import { t } from "../i18n/index.svelte";
  import { dialogs } from "../stores/dialogs.svelte";
  import Modal from "./Modal.svelte";
  import Icon, { iconForFile } from "./Icon.svelte";

  const request = $derived(dialogs.conflict);
  let renaming = $state(false);
  let name = $state("");
  let all = $state(false);
  let taken = $state(false);
  let field: HTMLInputElement | undefined = $state();

  $effect(() => {
    if (request) {
      renaming = false;
      all = false;
      taken = false;
      name = request.suggestion;
    }
  });

  async function startRename() {
    renaming = true;
    await tick();
    const dot = name.lastIndexOf(".");
    field?.focus();
    field?.setSelectionRange(0, dot > 0 && !request?.isDir ? dot : name.length);
  }

  async function submitRename(e: SubmitEvent) {
    e.preventDefault();
    const trimmed = name.trim();
    if (!request || !trimmed) return;
    if (trimmed === request.name || (await request.isTaken(trimmed))) {
      taken = true;
      return;
    }
    dialogs.answerConflict({ action: "rename", name: trimmed, all: false });
  }
</script>

{#if request}
  <Modal title={t("conflict.title")} onclose={() => dialogs.answerConflict({ action: "cancel", all })} width={470}>
    <div class="body">
      <div class="file">
        <span class="icon" class:dir={request.isDir}>
          <Icon name={request.isDir ? "folder" : iconForFile(request.name)} size={22} />
        </span>
        <div>
          <p class="name">{request.name}</p>
          <p class="where">
            {request.side === "remote"
              ? t("conflict.onServer", { name: request.name })
              : t("conflict.onLocal", { name: request.name })}
          </p>
        </div>
      </div>
      {#if request.isDir}<p class="note">{t("conflict.folderNote")}</p>{/if}

      {#if renaming}
        <form id="rename-form" class="field" onsubmit={submitRename}>
          <span>{t("conflict.newName")}</span>
          <input class="input" bind:this={field} bind:value={name} oninput={() => (taken = false)} spellcheck="false" />
          {#if taken}<small class="taken">{t("conflict.taken")}</small>{/if}
        </form>
      {:else if request.remaining > 0}
        <label class="all">
          <input type="checkbox" bind:checked={all} />
          {t("conflict.applyAll", { count: request.remaining })}
        </label>
      {/if}
    </div>

    {#snippet footer()}
      {#if renaming}
        <button class="btn ghost" onclick={() => (renaming = false)}>{t("conflict.back")}</button>
        <button class="btn primary" type="submit" form="rename-form">{t("conflict.saveAs")}</button>
      {:else}
        <button class="btn ghost" onclick={() => dialogs.answerConflict({ action: "cancel", all })}>{t("conflict.cancel")}</button>
        <button
          class="btn"
          onclick={() =>
            all ? dialogs.answerConflict({ action: "rename", name: request.suggestion, all: true }) : startRename()}
        >
          {t("conflict.rename")}
        </button>
        <button class="btn primary" onclick={() => dialogs.answerConflict({ action: "replace", all })}>{t("conflict.replace")}</button>
      {/if}
    {/snippet}
  </Modal>
{/if}

<style>
  .body {
    display: grid;
    gap: 14px;
  }
  .file {
    display: flex;
    gap: 12px;
    align-items: center;
    padding: 12px;
    border-radius: var(--radius);
    background: var(--raised);
    border: 1px solid var(--border);
  }
  .icon {
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    border-radius: 9px;
    background: var(--surface);
    color: var(--text-dim);
  }
  .icon.dir {
    color: var(--accent);
  }
  .name {
    margin: 0;
    font-weight: 600;
    word-break: break-all;
  }
  .where {
    margin: 2px 0 0;
    font-size: 12.5px;
    color: var(--text-dim);
  }
  .note {
    margin: 0;
    font-size: 12.5px;
    color: var(--warn);
  }
  .all {
    display: flex;
    gap: 8px;
    align-items: center;
    font-size: 13px;
    color: var(--text-dim);
  }
  .taken {
    color: var(--danger);
    font-size: 12px;
  }
</style>
