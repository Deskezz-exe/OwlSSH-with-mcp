<script lang="ts">
  import { onMount } from "svelte";
  import { i18n, t } from "../i18n/index.svelte";
  import { settings } from "../stores/settings.svelte";
  import { mcp } from "../stores/mcp.svelte";
  import { copyText } from "../terminal/clipboard";

  const s = $derived(settings.value!);
  let showToken = $state(false);
  let copied = $state("");

  const url = $derived(`http://127.0.0.1:${s.mcp.port}/mcp`);
  const claudeCommand = $derived(
    `claude mcp add --transport http owlssh ${url} --header "Authorization: Bearer ${s.mcp.token}"`,
  );
  const jsonConfig = $derived(
    JSON.stringify(
      { mcpServers: { owlssh: { type: "http", url, headers: { Authorization: `Bearer ${s.mcp.token}` } } } },
      null,
      2,
    ),
  );
  const mask = (text: string) => (showToken ? text : text.replaceAll(s.mcp.token, "•".repeat(12)));
  const timeFormat = $derived(new Intl.DateTimeFormat(i18n.locale, { dateStyle: "short", timeStyle: "medium" }));

  onMount(() => mcp.refresh());

  async function copy(what: string, text: string) {
    await copyText(text);
    copied = what;
    setTimeout(() => (copied = ""), 1500);
  }

  async function change(apply: (x: NonNullable<typeof settings.value>) => void) {
    await settings.update(apply);
    // The server restarts in the background; read its new state shortly after.
    setTimeout(() => mcp.refresh(), 400);
  }

  function regenerate() {
    change((x) => (x.mcp.token = crypto.randomUUID().replaceAll("-", "")));
  }
</script>

<div class="page">
  <h1>{t("mcp.title")}</h1>
  <p class="intro">{t("mcp.intro")}</p>

  <section>
    <div class="status" class:on={mcp.status?.running} class:bad={!!mcp.status?.error}>
      <span class="dot"></span>
      {#if mcp.status?.running}{t("mcp.running", { url: mcp.status.url })}
      {:else if mcp.status?.error}{t("mcp.failed", { error: mcp.status.error })}
      {:else}{t("mcp.stopped")}{/if}
    </div>

    <label class="row">
      <span class="label">{t("mcp.enabled")}</span>
      <input type="checkbox" class="switch" checked={s.mcp.enabled} onchange={(e) => change((x) => (x.mcp.enabled = e.currentTarget.checked))} />
    </label>
    <label class="row">
      <span class="label">{t("mcp.port")}</span>
      <input
        class="input number mono"
        type="number"
        min="1024"
        max="65535"
        value={s.mcp.port}
        onchange={(e) => {
          const port = Number(e.currentTarget.value);
          if (port >= 1024 && port <= 65535) change((x) => (x.mcp.port = port));
        }}
      />
    </label>
    <label class="row">
      <span class="label">{t("mcp.confirm")}<small>{t("mcp.confirmHint")}</small></span>
      <input type="checkbox" class="switch" checked={s.mcp.confirmDangerous} onchange={(e) => change((x) => (x.mcp.confirmDangerous = e.currentTarget.checked))} />
    </label>
    <div class="row">
      <span class="label">{t("mcp.token")}<small>{t("mcp.tokenHint")}</small></span>
      <div class="token">
        <code class="mono">{showToken ? s.mcp.token : "•".repeat(12)}</code>
        <button class="btn ghost" onclick={() => (showToken = !showToken)}>{showToken ? t("mcp.hide") : t("mcp.show")}</button>
        <button class="btn ghost" onclick={regenerate}>{t("mcp.regenerate")}</button>
      </div>
    </div>
  </section>

  <section>
    <h2>{t("mcp.connectTitle")}</h2>
    <div class="snippet">
      <div class="snippet-head">
        <span>{t("mcp.claudeCode")}</span>
        <button class="btn ghost" onclick={() => copy("cli", claudeCommand)}>{copied === "cli" ? t("mcp.copied") : t("mcp.copy")}</button>
      </div>
      <pre class="mono">{mask(claudeCommand)}</pre>
    </div>
    <div class="snippet">
      <div class="snippet-head">
        <span>{t("mcp.otherClients")}</span>
        <button class="btn ghost" onclick={() => copy("json", jsonConfig)}>{copied === "json" ? t("mcp.copied") : t("mcp.copy")}</button>
      </div>
      <pre class="mono">{mask(jsonConfig)}</pre>
    </div>
  </section>

  <section>
    <h2>{t("mcp.activity")}</h2>
    {#if mcp.activity.length === 0}
      <p class="empty">{t("mcp.activityEmpty")}</p>
    {:else}
      <ul class="log">
        {#each mcp.activity as entry, i (entry.time + entry.tool + i)}
          <li>
            <span class="time">{timeFormat.format(entry.time)}</span>
            <span class="tool mono">{entry.tool}</span>
            <span class="server">{entry.server ?? ""}</span>
            <span class="detail mono" title={entry.detail}>{entry.detail}</span>
            <span class="outcome {entry.outcome}">
              {t(`mcp.outcome.${entry.outcome}`)}{entry.exitCode !== null && entry.exitCode !== 0 ? ` · ${t("mcp.exit", { code: entry.exitCode })}` : ""}
            </span>
          </li>
        {/each}
      </ul>
    {/if}
  </section>
</div>

<style>
  .page {
    height: 100%;
    overflow: auto;
    padding: 22px 26px 40px;
  }
  .page > * {
    max-width: 860px;
  }
  h1 {
    margin: 0 0 6px;
    font-family: var(--font-display);
    font-weight: 500;
    font-size: 20px;
  }
  .intro {
    margin: 0 0 20px;
    color: var(--text-dim);
    max-width: 640px;
  }
  section {
    margin-bottom: 22px;
    padding: 6px 18px 14px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
  }
  h2 {
    margin: 12px 0 10px;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-faint);
  }
  .status {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 12px 0;
    font-weight: 500;
    color: var(--text-dim);
  }
  .dot {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: var(--text-faint);
  }
  .status.on .dot {
    background: var(--ok);
    box-shadow: 0 0 0 4px color-mix(in srgb, var(--ok) 20%, transparent);
  }
  .status.on {
    color: var(--text);
  }
  .status.bad .dot {
    background: var(--danger);
  }
  .status.bad {
    color: var(--danger);
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 20px;
    min-height: 54px;
    padding: 8px 0;
    border-top: 1px solid var(--border);
  }
  .label {
    display: grid;
    gap: 2px;
    font-weight: 500;
  }
  small {
    font-weight: 400;
    font-size: 12px;
    color: var(--text-faint);
  }
  .number {
    width: 110px;
    text-align: right;
  }
  .token {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .token code {
    padding: 6px 10px;
    border-radius: var(--radius-sm);
    background: var(--bg);
    border: 1px solid var(--border);
    font-size: 12px;
  }
  .snippet + .snippet {
    margin-top: 12px;
  }
  .snippet-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: 13px;
    font-weight: 500;
    margin-bottom: 4px;
  }
  pre {
    margin: 0;
    padding: 10px 12px;
    border-radius: var(--radius-sm);
    background: var(--bg);
    border: 1px solid var(--border);
    font-size: 12px;
    white-space: pre-wrap;
    word-break: break-all;
  }
  .empty {
    margin: 0;
    color: var(--text-faint);
  }
  .log {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .log li {
    display: grid;
    grid-template-columns: 150px 120px 110px minmax(0, 1fr) auto;
    gap: 10px;
    align-items: center;
    padding: 7px 0;
    border-top: 1px solid var(--border);
    font-size: 12.5px;
  }
  .time {
    color: var(--text-faint);
    font-size: 12px;
  }
  .tool {
    color: var(--accent);
    font-size: 12px;
  }
  .server {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .detail {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-dim);
    font-size: 12px;
  }
  .outcome {
    font-size: 11.5px;
    padding: 1px 8px;
    border-radius: 10px;
    background: color-mix(in srgb, var(--ok) 15%, transparent);
    color: var(--ok);
  }
  .outcome.error {
    background: var(--danger-soft);
    color: var(--danger);
  }
  .outcome.denied {
    background: color-mix(in srgb, var(--warn) 15%, transparent);
    color: var(--warn);
  }
</style>
