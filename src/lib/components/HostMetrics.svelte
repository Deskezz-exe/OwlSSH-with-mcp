<script lang="ts">
  import { t } from "../i18n/index.svelte";
  import { metrics } from "../stores/metrics.svelte";
  import { hosts } from "../stores/hosts.svelte";
  import { formatDuration, formatKb, percent } from "../format";

  let { hostId }: { hostId: string } = $props();

  const data = $derived(metrics.byHost[hostId]);
  const connected = $derived(hosts.stateOf(hostId) === "connected");
  let showProcesses = $state(false);

  const bars = $derived.by(() => {
    if (!data || data === "unavailable") return [];
    return [
      { label: t("metrics.cpu"), value: data.cpuPercent, detail: data.cpus ? t("metrics.cores", { count: data.cpus }) : "" },
      {
        label: t("metrics.ram"),
        value: percent(data.memUsedKb, data.memTotalKb),
        detail: data.memTotalKb ? `${formatKb(data.memUsedKb ?? 0)} / ${formatKb(data.memTotalKb)}` : "",
      },
      {
        label: t("metrics.disk"),
        value: percent(data.diskUsedKb, data.diskTotalKb),
        detail: data.diskTotalKb ? `${formatKb(data.diskUsedKb ?? 0)} / ${formatKb(data.diskTotalKb)}` : "",
      },
    ];
  });

  /** Calm below 70 %, amber up to 90 %, red above. */
  const level = (v: number | null) => (v === null ? "none" : v >= 90 ? "high" : v >= 70 ? "mid" : "low");
</script>

<div class="metrics">
  {#if !connected}
    <p class="hint">{t("metrics.connectHint")}</p>
  {:else if !data}
    <p class="hint">{t("metrics.waiting")}</p>
  {:else if data === "unavailable"}
    <p class="hint">{t("metrics.unavailable")}</p>
  {:else}
    {#each bars as bar}
      <div class="bar {level(bar.value)}" title={bar.detail}>
        <span class="name">{bar.label}</span>
        <span class="track"><span class="fill" style:width="{bar.value ?? 0}%"></span></span>
        <span class="value mono">{bar.value === null ? "—" : `${Math.round(bar.value)}%`}</span>
      </div>
    {/each}

    <p class="facts">
      {#if data.uptimeSecs !== null}<span>{t("metrics.up", { time: formatDuration(data.uptimeSecs) })}</span>{/if}
      {#if data.load}<span>{t("metrics.load", { value: data.load[0].toFixed(2) })}</span>{/if}
      {#if data.docker}
        <span class="docker">{t("metrics.containers", { running: data.docker.running, total: data.docker.total })}</span>
      {/if}
    </p>
    {#if data.os}<p class="os" title={data.os}>{data.os}</p>{/if}

    {#if data.processes.length}
      <button class="toggle" onclick={() => (showProcesses = !showProcesses)} aria-expanded={showProcesses}>
        {showProcesses ? "▾" : "▸"} {t("metrics.topProcesses")}
      </button>
      {#if showProcesses}
        <ul class="procs mono">
          <li class="cols"><span>{t("metrics.colProcess")}</span><span>{t("metrics.colCpu")}</span><span>{t("metrics.colMem")}</span></li>
          {#each data.processes as p (p.pid)}
            <li><span class="pname" title={p.name}>{p.name}</span><span>{p.cpu.toFixed(1)}%</span><span class="dim">{p.mem.toFixed(1)}%</span></li>
          {/each}
        </ul>
      {/if}
    {/if}
  {/if}
</div>

<style>
  .metrics {
    display: grid;
    gap: 6px;
    padding: 10px 0 2px;
    border-top: 1px dashed var(--border);
  }
  .hint {
    margin: 0;
    font-size: 12px;
    color: var(--text-faint);
  }
  .bar {
    display: grid;
    grid-template-columns: 40px 1fr 40px;
    align-items: center;
    gap: 8px;
    font-size: 12px;
  }
  .name {
    color: var(--text-dim);
  }
  .track {
    height: 6px;
    border-radius: 3px;
    background: var(--raised);
    overflow: hidden;
  }
  .fill {
    display: block;
    height: 100%;
    border-radius: 3px;
    background: var(--ok);
    transition: width 0.6s ease;
  }
  .mid .fill {
    background: var(--warn);
  }
  .high .fill {
    background: var(--danger);
  }
  .value {
    text-align: right;
    font-size: 11.5px;
  }
  .high .value {
    color: var(--danger);
  }
  .facts {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 12px;
    margin: 2px 0 0;
    font-size: 12px;
    color: var(--text-dim);
  }
  .os {
    margin: 0;
    font-size: 11.5px;
    color: var(--text-faint);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .toggle {
    justify-self: start;
    padding: 0;
    border: 0;
    background: transparent;
    font-size: 12px;
    color: var(--text-dim);
  }
  .toggle:hover {
    color: var(--text);
  }
  .procs {
    margin: 0;
    padding: 0;
    list-style: none;
    font-size: 11.5px;
  }
  .procs li {
    display: grid;
    grid-template-columns: 1fr 52px 48px;
    gap: 6px;
    padding: 2px 0;
  }
  .procs .cols {
    font-family: var(--font-body);
    font-size: 10.5px;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-faint);
    border-bottom: 1px solid var(--border);
    margin-bottom: 2px;
  }
  .procs li span:not(.pname):not(:first-child) {
    text-align: right;
  }
  .pname {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .dim {
    color: var(--text-faint);
  }
</style>
