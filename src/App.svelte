<script lang="ts">
  import { onMount } from "svelte";
  import { getVersion } from "@tauri-apps/api/app";

  let version = $state("");

  onMount(async () => {
    version = await getVersion();
  });
</script>

<main class="splash">
  <img src="/logo.png" alt="OwlSSH" width="96" height="96" />
  <h1>OwlSSH</h1>
  <p>v{version}</p>
</main>

<style>
  .splash {
    height: 100vh;
    display: grid;
    place-content: center;
    justify-items: center;
    gap: 8px;
  }
  h1 {
    margin: 0;
    font-size: 28px;
  }
  p {
    margin: 0;
    color: var(--text-dim);
  }
</style>
