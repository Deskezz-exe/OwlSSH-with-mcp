<script lang="ts" module>
  export interface MenuItem {
    label: string;
    action: () => void;
    danger?: boolean;
    disabled?: boolean;
    separatorBefore?: boolean;
  }
</script>

<script lang="ts">
  let {
    x,
    y,
    items,
    onclose,
  }: { x: number; y: number; items: MenuItem[]; onclose: () => void } = $props();

  let menu: HTMLDivElement | undefined = $state();
  let pos = $state({ left: 0, top: 0 });

  // Keep the menu inside the window.
  $effect(() => {
    if (!menu) return;
    const { width, height } = menu.getBoundingClientRect();
    pos = {
      left: Math.min(x, window.innerWidth - width - 6),
      top: Math.min(y, window.innerHeight - height - 6),
    };
    menu.querySelector<HTMLButtonElement>("button:not(:disabled)")?.focus();
  });

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Escape") onclose();
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const buttons = [...(menu?.querySelectorAll<HTMLButtonElement>("button:not(:disabled)") ?? [])];
      const i = buttons.indexOf(document.activeElement as HTMLButtonElement);
      const next = e.key === "ArrowDown" ? i + 1 : i - 1;
      buttons[(next + buttons.length) % buttons.length]?.focus();
    }
  }
</script>

<svelte:window {onkeydown} onblur={onclose} />

<div class="layer" role="presentation" onmousedown={onclose} oncontextmenu={(e) => { e.preventDefault(); onclose(); }}></div>
<div class="menu" role="menu" bind:this={menu} style:left="{pos.left}px" style:top="{pos.top}px">
  {#each items as item}
    {#if item.separatorBefore}<hr />{/if}
    <button
      role="menuitem"
      class:danger={item.danger}
      disabled={item.disabled}
      onclick={() => {
        onclose();
        item.action();
      }}>{item.label}</button
    >
  {/each}
</div>

<style>
  .layer {
    position: fixed;
    inset: 0;
    z-index: 60;
  }
  .menu {
    position: fixed;
    z-index: 61;
    min-width: 190px;
    padding: 5px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    box-shadow: var(--shadow);
  }
  button {
    display: block;
    width: 100%;
    padding: 7px 10px;
    border: 0;
    border-radius: var(--radius-sm);
    background: transparent;
    text-align: left;
  }
  button:hover,
  button:focus-visible {
    background: var(--hover);
    outline: none;
  }
  button:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .danger {
    color: var(--danger);
  }
  hr {
    margin: 4px 6px;
    border: 0;
    border-top: 1px solid var(--border);
  }
</style>
