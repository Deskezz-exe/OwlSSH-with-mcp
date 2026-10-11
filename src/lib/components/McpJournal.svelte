<!-- The audit log: filters, pages of 15 calls, and a drawer with the full details of a call. -->
<script lang="ts">
  import { i18n, t } from "../i18n/index.svelte";
  import { mcp } from "../stores/mcp.svelte";
  import { api, type AuditEntry, type AuditOutcome, type AuditPage } from "../api";

  const PAGE_SIZE = 15;
  const outcomes: (AuditOutcome | null)[] = [null, "ok", "error", "denied"];

  let outcome = $state<AuditOutcome | null>(null);
  let client = $state<string | null>(null);
  let page = $state(0);
  let data = $state<AuditPage>({ entries: [], total: 0, clients: [] });
  let open = $state<string | null>(null);

  const pages = $derived(Math.max(1, Math.ceil(data.total / PAGE_SIZE)));
  const timeFormat = $derived(new Intl.DateTimeFormat(i18n.locale, { dateStyle: "short", timeStyle: "medium" }));
  const keyOf = (e: AuditEntry, i: number) => `${e.time}-${e.tool}-${i}`;

  async function load() {
    data = await api
      .mcpActivity({ offset: page * PAGE_SIZE, limit: PAGE_SIZE, outcome, client })
      .catch(() => ({ entries: [], total: 0, clients: [] }));
    if (page > 0 && page >= pages) page = pages - 1;
  }

  // Reload when filters or the page change, and when a new call arrives.
  $effect(() => {
    void [outcome, client, page, mcp.activityTick];
    load();
  });

  function setOutcome(value: AuditOutcome | null) {
    outcome = value;
    page = 0;
    open = null;
  }

  function goto(next: number) {
    page = Math.min(pages - 1, Math.max(0, next));
    open = null;
  }
</script>

<div class="toolbar">
  <div class="segmented" role="radiogroup" aria-label={t("mcp.colStatus")}>
    {#each outcomes as value}
      <button class:on={outcome === value} role="radio" aria-checked={outcome === value} onclick={() => setOutcome(value)}>
        {value ? t(`mcp.outcome.${value}`) : t("mcp.filterAll")}
      </button>
    {/each}
  </div>
  {#if data.clients.length}
    <select
      class="input client"
      aria-label={t("mcp.filterClient")}
      value={client ?? ""}
      onchange={(e) => {
        client = e.currentTarget.value || null;
        page = 0;
      }}
    >
      <option value="">{t("mcp.allClients")}</option>
      {#each data.clients as name}<option value={name}>{name}</option>{/each}
    </select>
  {/if}
</div>

<div class="table" role="table" style:--rows={PAGE_SIZE}>
    <div class="row head" role="row">
      <span>{t("mcp.colTime")}</span>
      <span>{t("mcp.colClient")}</span>
      <span>{t("mcp.colTool")}</span>
      <span>{t("mcp.colServer")}</span>
      <span class="status-col">{t("mcp.colStatus")}</span>
    </div>
    {#each data.entries as entry, i (keyOf(entry, i))}
      {@const key = keyOf(entry, i)}
      <button
        class="row item"
        class:open={open === key}
        aria-expanded={open === key}
        onclick={() => (open = open === key ? null : key)}
      >
        <span class="time">{timeFormat.format(entry.time)}</span>
        <span class="client-name">{entry.client ?? t("mcp.unknownClient")}</span>
        <span class="tool mono">{entry.tool}</span>
        <span class="server">{entry.server ?? ""}</span>
        <span class="status-col">
          <span class="outcome {entry.outcome}">{t(`mcp.outcome.${entry.outcome}`)}</span>
          <span class="chev" aria-hidden="true">{open === key ? "▴" : "▾"}</span>
        </span>
      </button>
      {#if open === key}
        <div class="drawer">
          <span class="label">{t("mcp.details")}</span>
          {#if entry.detail}
            <pre class="mono">{entry.detail}</pre>
          {:else}
            <p class="none">{t("mcp.noDetails")}</p>
          {/if}
          {#if entry.exitCode !== null}
            <span class="exit">{t("mcp.exit", { code: entry.exitCode })}</span>
          {/if}
        </div>
      {/if}
    {/each}
    {#if data.entries.length === 0}
      <p class="empty">{t("mcp.activityEmpty")}</p>
    {/if}
  </div>

  <div class="pager">
    <span class="count">
      {data.total === 0 ? "" : t("mcp.shown", {
        from: page * PAGE_SIZE + 1,
        to: Math.min(data.total, (page + 1) * PAGE_SIZE),
        total: data.total,
      })}
    </span>
    <div class="nav">
      <button class="btn ghost" disabled={page === 0} onclick={() => goto(page - 1)}>‹ {t("mcp.prev")}</button>
      <span class="pageno">{t("mcp.page", { page: page + 1, pages })}</span>
      <button class="btn ghost" disabled={page >= pages - 1} onclick={() => goto(page + 1)}>{t("mcp.next")} ›</button>
    </div>
  </div>

<style>
  .toolbar {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    align-items: center;
    margin-bottom: 12px;
  }
  .client {
    width: auto;
    min-width: 170px;
    height: 36px;
  }
  .empty {
    margin: 0;
    padding: 40px 14px;
    text-align: center;
    color: var(--text-faint);
  }
  /* Always tall enough for a full page, so filtering never makes the block jump. */
  .table {
    --row: 38px;
    min-height: calc(var(--row) * (var(--rows) + 1) + 2px);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    overflow: hidden;
  }
  .row {
    display: grid;
    grid-template-columns: 150px 130px minmax(110px, 1fr) minmax(90px, 1fr) 130px;
    gap: 12px;
    align-items: center;
    width: 100%;
    height: var(--row);
    padding: 0 14px;
    border: 0;
    background: transparent;
    text-align: left;
    font-size: 13px;
  }
  .head {
    background: var(--raised);
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-faint);
  }
  .item {
    border-top: 1px solid var(--border);
    cursor: pointer;
  }
  .item:hover {
    background: var(--hover);
  }
  .item.open {
    background: var(--accent-soft);
    box-shadow: inset 3px 0 0 var(--accent);
  }
  .time {
    font-size: 12px;
    color: var(--text-dim);
  }
  .client-name {
    font-weight: 500;
  }
  .tool {
    font-size: 12px;
    color: var(--accent);
  }
  .server,
  .client-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .status-col {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 10px;
  }
  .chev {
    color: var(--text-faint);
    font-size: 11px;
  }
  .outcome {
    font-size: 11.5px;
    padding: 2px 9px;
    border-radius: 10px;
    background: color-mix(in srgb, var(--ok) 15%, transparent);
    color: var(--ok);
  }
  .outcome.error {
    background: var(--danger-soft);
    color: var(--danger);
  }
  .outcome.denied {
    background: color-mix(in srgb, var(--warn) 16%, transparent);
    color: var(--warn);
  }
  .drawer {
    display: grid;
    gap: 6px;
    padding: 4px 14px 14px 17px;
    background: var(--accent-soft);
    box-shadow: inset 3px 0 0 var(--accent);
    animation: drop 0.14s ease-out;
  }
  .label {
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-faint);
  }
  pre {
    margin: 0;
    max-height: 320px;
    overflow: auto;
    padding: 10px 12px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border);
    background: var(--input);
    font-size: 12.5px;
    white-space: pre-wrap;
    word-break: break-word;
  }
  .none,
  .exit {
    margin: 0;
    font-size: 12px;
    color: var(--text-dim);
  }
  .pager {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-top: 10px;
    font-size: 12.5px;
    color: var(--text-dim);
  }
  .nav {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .pageno {
    min-width: 110px;
    text-align: center;
  }
  @keyframes drop {
    from {
      opacity: 0;
      transform: translateY(-4px);
    }
  }
</style>
