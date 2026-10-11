<script lang="ts">
  import { t } from "../i18n/index.svelte";
  import { dialogs } from "../stores/dialogs.svelte";
  import Modal from "./Modal.svelte";

  let value = $state("");
  const request = $derived(dialogs.text);

  $effect(() => {
    if (request) value = request.value;
  });

  function submit(e: SubmitEvent) {
    e.preventDefault();
    const trimmed = value.trim();
    if (trimmed) dialogs.answer(trimmed);
  }

  /** Select the name without its extension, like file managers do. */
  function selectName(input: HTMLInputElement) {
    requestAnimationFrame(() => {
      input.focus();
      const dot = input.value.lastIndexOf(".");
      input.setSelectionRange(0, dot > 0 ? dot : input.value.length);
    });
  }
</script>

{#if request}
  <Modal title={request.title} onclose={() => dialogs.answer(null)} width={400}>
    <form id="text-form" onsubmit={submit}>
      <label class="field">
        <span>{request.label}</span>
        <input class="input" bind:value use:selectName spellcheck="false" />
      </label>
    </form>
    {#snippet footer()}
      <button class="btn ghost" onclick={() => dialogs.answer(null)}>{t("common.cancel")}</button>
      <button class="btn primary" type="submit" form="text-form">{request.submit}</button>
    {/snippet}
  </Modal>
{/if}
