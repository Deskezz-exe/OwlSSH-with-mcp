<!-- Answers the questions the connect flow asks (see stores/prompt.svelte.ts). -->
<script lang="ts">
  import { t } from "../i18n/index.svelte";
  import { prompt } from "../stores/prompt.svelte";
  import Modal from "./Modal.svelte";

  let secret = $state("");
  let save = $state(true);

  const request = $derived(prompt.current);

  $effect(() => {
    if (request) {
      secret = "";
      save = true;
    }
  });

  function cancel() {
    prompt.answer(null);
  }

  function submit(e: SubmitEvent) {
    e.preventDefault();
    if (!request) return;
    if (request.kind === "password") prompt.answer({ kind: "password", password: secret, save });
    else if (request.kind === "passphrase") prompt.answer({ kind: "passphrase", passphrase: secret });
    else prompt.answer({ kind: "hostKey", trust: true });
  }
</script>

{#if request}
  {#if request.kind === "hostKey"}
    <Modal
      title={request.changed ? t("connect.hostKeyChangedTitle") : t("connect.hostKeyTitle")}
      onclose={cancel}
      width={520}
    >
      <form id="prompt-form" onsubmit={submit}>
        <p class:warning={request.changed}>
          {request.changed
            ? t("connect.hostKeyChangedBody", { target: request.target })
            : t("connect.hostKeyBody", { target: request.target })}
        </p>
        <div class="fingerprint">
          <span>{request.algorithm}</span>
          <code>{request.fingerprint}</code>
        </div>
      </form>
      {#snippet footer()}
        <button class="btn ghost" onclick={cancel}>{t("common.cancel")}</button>
        <button class="btn {request.changed ? 'danger' : 'primary'}" type="submit" form="prompt-form">
          {request.changed ? t("connect.trustChanged") : t("connect.trust")}
        </button>
      {/snippet}
    </Modal>
  {:else}
    <Modal
      title={request.kind === "password"
        ? t("connect.passwordTitle", { target: request.target })
        : t("connect.passphraseTitle")}
      onclose={cancel}
      width={420}
    >
      <form id="prompt-form" onsubmit={submit}>
        {#if request.kind === "passphrase"}
          <p>{t("connect.passphraseBody", { path: request.keyPath })}</p>
        {/if}
        <!-- svelte-ignore a11y_autofocus -->
        <input class="input" type="password" bind:value={secret} autofocus required autocomplete="off" />
        {#if request.kind === "password"}
          <label class="check">
            <input type="checkbox" bind:checked={save} />
            {t("connect.savePassword")}
          </label>
        {/if}
      </form>
      {#snippet footer()}
        <button class="btn ghost" onclick={cancel}>{t("common.cancel")}</button>
        <button class="btn primary" type="submit" form="prompt-form">{t("connect.submit")}</button>
      {/snippet}
    </Modal>
  {/if}
{/if}

<style>
  form {
    display: grid;
    gap: 12px;
  }
  p {
    margin: 0;
    color: var(--text-dim);
  }
  p.warning {
    color: var(--danger);
  }
  .fingerprint {
    display: grid;
    gap: 4px;
    padding: 10px 12px;
    border-radius: var(--radius-sm);
    background: var(--bg);
    border: 1px solid var(--border);
  }
  .fingerprint span {
    font-size: 11px;
    color: var(--text-faint);
  }
  code {
    font-family: var(--font-mono);
    font-size: 12.5px;
    word-break: break-all;
  }
  .check {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    color: var(--text-dim);
  }
</style>
