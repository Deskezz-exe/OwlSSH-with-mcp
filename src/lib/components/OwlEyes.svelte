<!--
  Connection status as a pair of owl eyes: open and amber when connected,
  blinking while connecting, closed when idle, red when the attempt failed.
-->
<script lang="ts">
  import type { ConnState } from "../stores/hosts.svelte";
  import { t } from "../i18n/index.svelte";

  let { state = "idle", size = 18 }: { state?: ConnState; size?: number } = $props();

  const labels = {
    idle: "status.disconnected",
    connecting: "status.connecting",
    connected: "status.connected",
    error: "status.error",
  } as const;
</script>

<svg
  class="eyes {state}"
  width={size}
  height={size * 0.55}
  viewBox="0 0 22 12"
  role="img"
  aria-label={t(labels[state])}
>
  <title>{t(labels[state])}</title>
  {#if state === "idle"}
    <path d="M2 7 Q5.5 9.5 9 7 M13 7 Q16.5 9.5 20 7" />
  {:else if state === "error"}
    <path d="M3 4 L8 9 M8 4 L3 9 M14 4 L19 9 M19 4 L14 9" />
  {:else}
    <g class="lids">
      <circle cx="5.5" cy="6" r="4.2" />
      <circle cx="16.5" cy="6" r="4.2" />
    </g>
    <circle class="pupil" cx="6.3" cy="6" r="1.5" />
    <circle class="pupil" cx="17.3" cy="6" r="1.5" />
  {/if}
</svg>

<style>
  .eyes {
    flex: none;
    display: block;
    overflow: visible;
  }
  path {
    fill: none;
    stroke-width: 1.8;
    stroke-linecap: round;
  }
  .idle path {
    stroke: var(--text-faint);
  }
  .error path {
    stroke: var(--danger);
  }
  .lids circle {
    fill: var(--accent);
  }
  .pupil {
    fill: var(--bg);
  }
  .connecting .lids,
  .connecting .pupil {
    transform-origin: 50% 50%;
    transform-box: view-box;
    animation: blink 1.1s ease-in-out infinite;
  }
  .connecting .lids circle {
    fill: var(--text-dim);
  }
  @keyframes blink {
    0%,
    55%,
    100% {
      transform: scaleY(1);
    }
    70% {
      transform: scaleY(0.12);
    }
  }
</style>
