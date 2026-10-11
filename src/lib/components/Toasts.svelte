<script lang="ts">
  import { toast } from "../stores/toast.svelte";
</script>

<div class="toasts" aria-live="polite">
  {#each toast.list as item (item.id)}
    <button class="toast {item.tone}" onclick={() => toast.dismiss(item.id)}>{item.text}</button>
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    right: 16px;
    bottom: 40px;
    z-index: 70;
    display: grid;
    gap: 8px;
    justify-items: end;
  }
  .toast {
    max-width: 380px;
    padding: 10px 14px;
    border: 1px solid var(--border);
    border-left: 3px solid var(--accent);
    border-radius: var(--radius-sm);
    background: var(--surface);
    box-shadow: var(--shadow);
    text-align: left;
    animation: slide 0.18s ease-out;
  }
  .toast.error {
    border-left-color: var(--danger);
  }
  @keyframes slide {
    from {
      opacity: 0;
      transform: translateX(12px);
    }
  }
</style>
