<!-- Approval dialog for risky actions requested by an AI assistant. -->
<script lang="ts">
  import { t } from "../i18n/index.svelte";
  import { mcp } from "../stores/mcp.svelte";
  import Modal from "./Modal.svelte";

  const request = $derived(mcp.pending[0]);
  const actionKey = {
    command: "mcp.askCommand",
    writeFile: "mcp.askWrite",
    delete: "mcp.askDelete",
    rename: "mcp.askRename",
    edit: "mcp.askEdit",
  } as const;
</script>

{#if request}
  <Modal
    title={request.client ? t("mcp.askBy", { client: request.client }) : t("mcp.askTitle")}
    onclose={() => mcp.answer(request, false)}
    width={560}
  >
    <div class="ask">
      <p class="what">{t(actionKey[request.action] ?? "mcp.askCommand", { server: request.server })}</p>
      <pre class="mono">{request.detail}</pre>
      <p class="hint">{t("mcp.askHint")}</p>
    </div>
    {#snippet footer()}
      <button class="btn" onclick={() => mcp.answer(request, false)}>{t("mcp.deny")}</button>
      <button class="btn danger strong" onclick={() => mcp.answer(request, true)}>{t("mcp.allow")}</button>
    {/snippet}
  </Modal>
{/if}

<style>
  .ask {
    display: grid;
    gap: 10px;
  }
  .what {
    margin: 0;
    font-weight: 500;
  }
  pre {
    margin: 0;
    max-height: 280px;
    overflow: auto;
    padding: 12px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border);
    border-left: 3px solid var(--danger);
    background: var(--bg);
    font-size: 12.5px;
    white-space: pre-wrap;
    word-break: break-all;
  }
  .hint {
    margin: 0;
    font-size: 12px;
    color: var(--text-faint);
  }
  .strong {
    border-color: var(--danger);
  }
</style>
