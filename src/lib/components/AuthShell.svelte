<script lang="ts">
  import type { Snippet } from "svelte";
  import { KeyRound } from "@lucide/svelte";

  let { title, subtitle, children }: { title: string; subtitle?: string; children: Snippet } = $props();
</script>

<main class="shell">
  <div class="panel">
    <div class="brand">
      <span class="logo"><KeyRound size={22} strokeWidth={2.2} /></span>
      <div>
        <h1>{title}</h1>
        {#if subtitle}<p class="muted">{subtitle}</p>{/if}
      </div>
    </div>
    <div class="card body">
      {@render children()}
    </div>
  </div>
</main>

<style>
  .shell {
    height: 100%;
    display: grid;
    place-items: center;
    padding: 24px;
    overflow-y: auto;
    background: hsl(var(--sidebar));
  }
  .panel {
    width: 100%;
    max-width: 380px;
    display: flex;
    flex-direction: column;
    gap: 20px;
    animation: fade-in 0.2s ease-out;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 14px;
  }
  .logo {
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    border-radius: 12px;
    background: hsl(var(--primary));
    color: hsl(var(--primary-foreground));
    flex: none;
  }
  h1 {
    font-size: 20px;
    font-weight: 600;
  }
  p {
    margin: 2px 0 0;
    overflow-wrap: anywhere;
  }
  .body {
    padding: 24px;
    display: flex;
    flex-direction: column;
    gap: 16px;
    box-shadow: var(--shadow-lg);
  }
</style>
