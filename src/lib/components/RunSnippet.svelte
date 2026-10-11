<script lang="ts">
  import { untrack } from "svelte";
  import { Channel } from "@tauri-apps/api/core";
  import { t } from "../i18n/index.svelte";
  import { hosts } from "../stores/hosts.svelte";
  import { api, type HostRun, type Snippet } from "../api";
  import { fillParams, paramsOf } from "../snippetParams";
  import Modal from "./Modal.svelte";

  let { snippet, onclose }: { snippet: Snippet; onclose: () => void } = $props();

  const command = untrack(() => snippet.command);
  const params = paramsOf(command);
  let values = $state<Record<string, string>>(Object.fromEntries(params.map((p) => [p, ""])));
  let chosen = $state<Record<string, boolean>>(
    Object.fromEntries(hosts.list.map((h) => [h.id, hosts.stateOf(h.id) === "connected"])),
  );
  let results = $state<Record<string, HostRun | "running">>({});
  let started = $state(false);

  const finalCommand = $derived(fillParams(command, values));
  const selected = $derived(hosts.sorted.filter((h) => chosen[h.id]));
  const ready = $derived(selected.length > 0 && params.every((p) => values[p].trim() !== ""));

  function selectAll(onlyConnected: boolean) {
    for (const h of hosts.list) chosen[h.id] = !onlyConnected || hosts.stateOf(h.id) === "connected";
  }

  async function run(e: SubmitEvent) {
    e.preventDefault();
    if (!ready) return;
    started = true;
    results = Object.fromEntries(selected.map((h) => [h.id, "running" as const]));
    const channel = new Channel<HostRun>();
    channel.onmessage = (run) => {
      results[run.hostId] = run;
      if (!run.error) hosts.status[run.hostId] = "connected";
    };
    await api.broadcastCommand(
      selected.map((h) => h.id),
      finalCommand,
      channel,
    );
  }
</script>

<Modal title={t("snippets.runOn", { name: snippet.name })} {onclose} width={640}>
  {#if !started}
    <form id="run-form" onsubmit={run}>
      {#if params.length}
        <fieldset>
          <legend>{t("snippets.params")}</legend>
          {#each params as name, i}
            <label class="field">
              <span class="mono">{name}</span>
              <!-- svelte-ignore a11y_autofocus -->
              <input class="input mono" bind:value={values[name]} autofocus={i === 0} spellcheck="false" />
            </label>
          {/each}
        </fieldset>
      {/if}

      <fieldset>
        <legend>
          {t("snippets.servers")}
          <span class="quick">
            <button type="button" class="link" onclick={() => selectAll(true)}>{t("snippets.selectConnected")}</button>
            <button type="button" class="link" onclick={() => selectAll(false)}>{t("snippets.selectAll")}</button>
          </span>
        </legend>
        <div class="hosts">
          {#each hosts.sorted as host (host.id)}
            <label class="check">
              <input type="checkbox" bind:checked={chosen[host.id]} />
              {host.name}
            </label>
          {/each}
        </div>
      </fieldset>

      <div class="preview">
        <span>{t("snippets.preview")}</span>
        <pre class="mono">{finalCommand}</pre>
      </div>
    </form>
  {:else}
    <ul class="results">
      {#each Object.entries(results) as [hostId, result] (hostId)}
        {@const host = hosts.byId(hostId)}
        <li>
          <details open={result !== "running" && (!!result.error || result.exitCode !== 0)}>
            <summary>
              <span class="name">{host?.name}</span>
              {#if result === "running"}
                <span class="badge">{t("snippets.waiting")}</span>
              {:else if result.error}
                <span class="badge bad">{t("status.error")}</span>
              {:else}
                <span class="badge" class:bad={result.exitCode !== 0}>{t("snippets.exit", { code: result.exitCode ?? "?" })}</span>
              {/if}
            </summary>
            {#if result !== "running"}
              <pre class="mono out">{result.error ?? (result.stdout + (result.stderr ? `\n${result.stderr}` : "")).trim()}</pre>
            {/if}
          </details>
        </li>
      {/each}
    </ul>
  {/if}

  {#snippet footer()}
    <button class="btn ghost" onclick={onclose}>{t("common.close")}</button>
    {#if !started}
      <button class="btn primary" type="submit" form="run-form" disabled={!ready}>
        {t("snippets.start", { count: selected.length })}
      </button>
    {/if}
  {/snippet}
</Modal>

<style>
  form {
    display: grid;
    gap: 16px;
  }
  fieldset {
    margin: 0;
    padding: 0;
    border: 0;
    display: grid;
    gap: 8px;
  }
  legend {
    display: flex;
    justify-content: space-between;
    width: 100%;
    margin-bottom: 6px;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-faint);
  }
  .quick {
    display: flex;
    gap: 10px;
    text-transform: none;
    letter-spacing: 0;
  }
  .link {
    padding: 0;
    border: 0;
    background: none;
    color: var(--accent);
    font-size: 12px;
  }
  .hosts {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(170px, 1fr));
    gap: 6px 12px;
  }
  .check {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .preview span {
    font-size: 12px;
    color: var(--text-dim);
  }
  pre {
    margin: 4px 0 0;
    padding: 10px 12px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border);
    background: var(--bg);
    font-size: 12.5px;
    white-space: pre-wrap;
    word-break: break-all;
  }
  .results {
    display: grid;
    gap: 6px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  summary {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--raised);
    cursor: pointer;
  }
  .name {
    font-weight: 500;
  }
  .badge {
    font-size: 12px;
    color: var(--ok);
  }
  .badge.bad {
    color: var(--danger);
  }
  .out {
    max-height: 260px;
    overflow: auto;
  }
</style>
