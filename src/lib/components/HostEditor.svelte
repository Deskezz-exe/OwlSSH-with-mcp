<script lang="ts">
  import { untrack } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { t } from "../i18n/index.svelte";
  import { hosts } from "../stores/hosts.svelte";
  import { errorText, type AuthKind, type Host } from "../api";
  import Modal from "./Modal.svelte";

  let { host = null, onclose }: { host?: Host | null; onclose: () => void } = $props();

  // A snapshot of the host is the form's starting point; edits stay local until saved.
  const initial = untrack(() => host);
  let name = $state(initial?.name ?? "");
  let hostname = $state(initial?.hostname ?? "");
  let port = $state(initial?.port ?? 22);
  let username = $state(initial?.username ?? "root");
  let auth = $state<AuthKind>(initial?.auth ?? "password");
  let password = $state("");
  let forgetPassword = $state(false);
  let keyPath = $state(initial?.keyPath ?? "");
  let tags = $state(initial?.tags.join(", ") ?? "");
  let jumpHostId = $state(initial?.jumpHostId ?? "");
  let error = $state("");
  let saving = $state(false);

  const jumpOptions = $derived(hosts.sorted.filter((h) => h.id !== initial?.id));
  const authKinds: AuthKind[] = ["password", "key", "agent"];

  async function browseKey() {
    const picked = await open({ multiple: false, directory: false });
    if (typeof picked === "string") keyPath = picked;
  }

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    saving = true;
    error = "";
    try {
      await hosts.save({
        id: initial?.id ?? null,
        name,
        hostname,
        port: Number(port),
        username,
        auth,
        keyPath: auth === "key" ? keyPath : null,
        password: forgetPassword ? "" : password ? password : null,
        tags: tags.split(","),
        favorite: initial?.favorite ?? false,
        jumpHostId: jumpHostId || null,
      });
      onclose();
    } catch (err) {
      error = errorText(err);
    } finally {
      saving = false;
    }
  }
</script>

<Modal title={initial ? t("editor.titleEdit") : t("editor.titleNew")} {onclose} width={500}>
  <form id="host-form" onsubmit={submit}>
    <label class="field">
      <span>{t("editor.name")}</span>
      <input class="input" bind:value={name} placeholder={t("editor.namePlaceholder")} />
    </label>

    <div class="row">
      <label class="field grow">
        <span>{t("editor.hostname")}</span>
        <!-- svelte-ignore a11y_autofocus -->
        <input
          class="input mono"
          bind:value={hostname}
          placeholder={t("editor.hostnamePlaceholder")}
          required
          autofocus={!initial}
          spellcheck="false"
        />
      </label>
      <label class="field port">
        <span>{t("editor.port")}</span>
        <input class="input mono" type="number" min="1" max="65535" bind:value={port} required />
      </label>
    </div>

    <label class="field">
      <span>{t("editor.username")}</span>
      <input class="input mono" bind:value={username} required spellcheck="false" />
    </label>

    <div class="field">
      <span>{t("editor.auth")}</span>
      <div class="segmented wide" role="radiogroup">
        {#each authKinds as kind}
          <button
            type="button"
            role="radio"
            aria-checked={auth === kind}
            class:on={auth === kind}
            onclick={() => (auth = kind)}>{t(`editor.auth.${kind}`)}</button
          >
        {/each}
      </div>
    </div>

    {#if auth === "password"}
      <label class="field">
        <span>{t("editor.password")}</span>
        <input
          class="input"
          type="password"
          bind:value={password}
          disabled={forgetPassword}
          autocomplete="new-password"
          placeholder={initial?.hasPassword ? "••••••••" : ""}
        />
        <small>{initial?.hasPassword ? t("editor.passwordSaved") : t("editor.passwordHint")}</small>
      </label>
      {#if initial?.hasPassword}
        <label class="check">
          <input type="checkbox" bind:checked={forgetPassword} />
          {t("editor.removePassword")}
        </label>
      {/if}
    {:else if auth === "key"}
      <div class="field">
        <span>{t("editor.keyPath")}</span>
        <div class="row tight">
          <input class="input mono grow" bind:value={keyPath} placeholder="~/.ssh/id_ed25519" required />
          <button type="button" class="btn" onclick={browseKey}>{t("editor.browse")}</button>
        </div>
      </div>
    {:else}
      <p class="hint">{t("editor.agentHint")}</p>
    {/if}

    <div class="row">
      <label class="field grow">
        <span>{t("editor.tags")}</span>
        <input class="input" bind:value={tags} placeholder={t("editor.tagsPlaceholder")} />
      </label>
      <label class="field grow">
        <span>{t("editor.jump")}</span>
        <select class="input" bind:value={jumpHostId}>
          <option value="">{t("editor.jumpNone")}</option>
          {#each jumpOptions as option (option.id)}
            <option value={option.id}>{option.name}</option>
          {/each}
        </select>
      </label>
    </div>

    {#if error}<p class="error" role="alert">{error}</p>{/if}
  </form>

  {#snippet footer()}
    <button class="btn ghost" onclick={onclose}>{t("editor.cancel")}</button>
    <button class="btn primary" type="submit" form="host-form" disabled={saving}>{t("editor.save")}</button>
  {/snippet}
</Modal>

<style>
  form {
    display: grid;
    gap: 14px;
  }
  .row {
    display: flex;
    gap: 10px;
  }
  .row.tight {
    gap: 6px;
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  .wide {
    display: flex;
  }
  .port {
    width: 96px;
  }
  small,
  .hint {
    margin: 0;
    font-size: 12px;
    color: var(--text-faint);
  }
  .check {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    color: var(--text-dim);
  }
  .error {
    margin: 0;
    padding: 8px 10px;
    border-radius: var(--radius-sm);
    background: var(--danger-soft);
    color: var(--danger);
    font-size: 13px;
  }
</style>
