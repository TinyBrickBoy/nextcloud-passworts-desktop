<script lang="ts">
  import { CircleCheck, CircleAlert } from "@lucide/svelte";
  import { ui } from "../store.svelte";
</script>

{#if ui.toast}
  {#key ui.toast}
    <div class="toast card" class:error={ui.toast.kind === "error"} role="status">
      {#if ui.toast.kind === "error"}
        <CircleAlert size={16} />
      {:else}
        <CircleCheck size={16} />
      {/if}
      <span>{ui.toast.text}</span>
    </div>
  {/key}
{/if}

<style>
  .toast {
    position: fixed;
    bottom: 20px;
    left: 50%;
    transform: translateX(-50%);
    z-index: 60;
    display: flex;
    align-items: center;
    gap: 10px;
    max-width: min(520px, calc(100vw - 32px));
    padding: 10px 16px;
    font-size: 13px;
    font-weight: 500;
    box-shadow: var(--shadow-lg);
    animation: fade-in 0.15s ease-out;
  }
  .toast :global(svg) {
    flex: none;
    color: hsl(var(--success));
  }
  .error :global(svg) {
    color: hsl(var(--destructive));
  }
  span {
    overflow-wrap: anywhere;
  }
</style>
