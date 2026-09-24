<script lang="ts">
  import { ui } from "../store.svelte";
  import { t } from "../i18n.svelte";

  let value = $state("");
  let input = $state<HTMLInputElement>();
  let confirmButton = $state<HTMLButtonElement>();

  $effect(() => {
    const dialog = ui.dialog;
    if (!dialog) return;
    value = dialog.input?.value ?? "";
    queueMicrotask(() => (dialog.input ? input?.select() : confirmButton?.focus()));
  });

  function close(result: string | boolean | null) {
    const dialog = ui.dialog;
    ui.dialog = null;
    dialog?.resolve(result);
  }

  function submit(e: SubmitEvent) {
    e.preventDefault();
    if (ui.dialog?.input) {
      if (value.trim()) close(value.trim());
    } else {
      close(true);
    }
  }
</script>

{#if ui.dialog}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="backdrop" onclick={(e) => e.target === e.currentTarget && close(null)}>
    <div
      class="dialog card"
      role="dialog"
      aria-modal="true"
      aria-labelledby="dialog-title"
      tabindex="-1"
      onkeydown={(e) => e.key === "Escape" && (e.stopPropagation(), close(null))}
    >
      <form onsubmit={submit}>
      <h2 id="dialog-title">{ui.dialog.title}</h2>
      {#if ui.dialog.message}<p class="muted">{ui.dialog.message}</p>{/if}
      {#if ui.dialog.input}
        <input bind:this={input} bind:value placeholder={ui.dialog.input.placeholder} maxlength="48" />
      {/if}
      <div class="actions">
        <button type="button" onclick={() => close(null)}>{t("cancel")}</button>
        <button
          bind:this={confirmButton}
          type="submit"
          class={ui.dialog.danger ? "danger solid" : "primary"}
          disabled={!!ui.dialog.input && !value.trim()}
        >
          {ui.dialog.confirmLabel}
        </button>
      </div>
      </form>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 50;
    display: grid;
    place-items: center;
    padding: 24px;
    background: hsl(240 10% 3.9% / 0.45);
    animation: fade-in 0.12s ease-out;
  }
  .dialog {
    width: 100%;
    max-width: 400px;
    padding: 24px;
    box-shadow: var(--shadow-lg);
  }
  form {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  h2 {
    font-size: 17px;
    font-weight: 600;
  }
  p {
    margin: -6px 0 0;
    font-size: 14px;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 6px;
  }
</style>
