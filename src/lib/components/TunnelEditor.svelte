<script lang="ts">
  import { untrack } from "svelte";
  import { t } from "../i18n/index.svelte";
  import { hosts } from "../stores/hosts.svelte";
  import { tunnels } from "../stores/tunnels.svelte";
  import { errorText, type Tunnel, type TunnelKind } from "../api";
  import Modal from "./Modal.svelte";

  let { tunnel = null, onclose }: { tunnel?: Tunnel | null; onclose: () => void } = $props();

  const initial = untrack(() => tunnel);
  let hostId = $state(initial?.hostId ?? hosts.sorted[0]?.id ?? "");
  let kind = $state<TunnelKind>(initial?.kind ?? "local");
  let name = $state(initial?.name ?? "");
  let bindHost = $state(initial?.bindHost ?? "127.0.0.1");
  let bindPort = $state<number | null>(initial?.bindPort ?? null);
  let targetHost = $state(initial?.targetHost ?? "localhost");
  let targetPort = $state<number | null>(initial?.targetPort ?? null);
  let error = $state("");
  let saving = $state(false);

  const hostName = $derived(hosts.byId(hostId)?.name ?? "…");
  const kinds: TunnelKind[] = ["local", "remote"];

  // The usual case: the same port on both ends.
  function onTargetPort() {
    if (bindPort === null && targetPort) bindPort = targetPort;
  }

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    saving = true;
    error = "";
    try {
      await tunnels.save({
        id: initial?.id ?? null,
        hostId,
        name,
        kind,
        bindHost,
        bindPort: Number(bindPort),
        targetHost,
        targetPort: Number(targetPort),
      });
      onclose();
    } catch (err) {
      error = errorText(err);
    } finally {
      saving = false;
    }
  }
</script>

<Modal title={initial ? t("tunnels.edit") : t("tunnels.new")} {onclose} width={540}>
  <form id="tunnel-form" onsubmit={submit}>
    <div class="row">
      <label class="field grow">
        <span>{t("tunnels.server")}</span>
        <select class="input" bind:value={hostId} required>
          {#each hosts.sorted as h (h.id)}
            <option value={h.id}>{h.name}</option>
          {/each}
        </select>
      </label>
      <label class="field grow">
        <span>{t("tunnels.name")}</span>
        <input class="input" bind:value={name} placeholder={t("tunnels.namePlaceholder")} />
      </label>
    </div>

    <div class="field">
      <span>{t("tunnels.kind")}</span>
      <div class="segmented wide" role="radiogroup">
        {#each kinds as k}
          <button
            type="button"
            role="radio"
            aria-checked={kind === k}
            class:on={kind === k}
            onclick={() => {
              kind = k;
              if (!initial) targetHost = k === "local" ? "localhost" : "127.0.0.1";
            }}>{t(`tunnels.kind.${k}`)}</button
          >
        {/each}
      </div>
      <small>{t(`tunnels.kindHint.${kind}`, { server: hostName })}</small>
    </div>

    <div class="field">
      <span>{kind === "local" ? t("tunnels.listenHere") : t("tunnels.listenServer", { server: hostName })}</span>
      <div class="row tight">
        <input class="input mono grow" bind:value={bindHost} required spellcheck="false" />
        <span class="colon">:</span>
        <input class="input mono port" type="number" min="1" max="65535" bind:value={bindPort} required placeholder="8080" />
      </div>
    </div>

    <div class="field">
      <span>{kind === "local" ? t("tunnels.targetServer", { server: hostName }) : t("tunnels.targetHere")}</span>
      <div class="row tight">
        <input class="input mono grow" bind:value={targetHost} required spellcheck="false" />
        <span class="colon">:</span>
        <input
          class="input mono port"
          type="number"
          min="1"
          max="65535"
          bind:value={targetPort}
          onchange={onTargetPort}
          required
          placeholder="80"
        />
      </div>
    </div>

    <p class="route mono">
      {#if kind === "local"}
        {bindHost}:{bindPort ?? "…"} <span>→ {hostName} →</span> {targetHost}:{targetPort ?? "…"}
      {:else}
        {hostName} {bindHost}:{bindPort ?? "…"} <span>→ {t("tunnels.thisComputer")} →</span> {targetHost}:{targetPort ?? "…"}
      {/if}
    </p>
    {#if kind === "remote" && bindHost !== "127.0.0.1" && bindHost !== "localhost"}
      <p class="hint">{t("tunnels.gatewayHint")}</p>
    {/if}

    {#if error}<p class="error" role="alert">{error}</p>{/if}
  </form>

  {#snippet footer()}
    <button class="btn ghost" onclick={onclose}>{t("common.cancel")}</button>
    <button class="btn primary" type="submit" form="tunnel-form" disabled={saving || !hostId}>{t("tunnels.save")}</button>
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
    align-items: center;
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
    width: 110px;
  }
  .colon {
    color: var(--text-faint);
  }
  small,
  .hint {
    margin: 0;
    font-size: 12px;
    color: var(--text-faint);
  }
  .route {
    margin: 0;
    padding: 9px 12px;
    font-size: 12.5px;
    border-radius: var(--radius-sm);
    background: var(--accent-soft);
    color: var(--text);
  }
  .route span {
    color: var(--text-dim);
  }
  .error {
    margin: 0;
    padding: 8px 10px;
    border-radius: var(--radius-sm);
    background: var(--danger-soft);
    color: var(--danger);
  }
</style>
