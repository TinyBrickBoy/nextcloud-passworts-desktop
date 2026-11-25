<script lang="ts">
  import { onMount } from "svelte";
  import { Sun, Moon, Monitor, CircleCheck } from "@lucide/svelte";
  import { api, errorText, type Account, type VaultView } from "./api";
  import { languages, t } from "./i18n.svelte";
  import { saveSettings, ui } from "./store.svelte";
  import AuthShell from "./components/AuthShell.svelte";

  let { account, vault, onDone }: { account: Account; vault: VaultView; onDone: () => void } = $props();

  type Quick = "off" | "biometric" | "pin";
  // Schritte: 0 Aussehen, 1 Sicherheit, 2 Schnellentsperrung (nur verschlüsselt), 3 Daten, 4 Fertig
  const steps = $derived(vault.encrypted ? [0, 1, 2, 3, 4] : [0, 1, 3, 4]);
  let index = $state(0);
  const step = $derived(steps[index]);
  const last = $derived(index === steps.length - 1);

  const host = $derived(account.server.replace(/^https?:\/\//, ""));
  const titles = $derived<Record<number, string>>({
    0: t("wizard_step_appearance"),
    1: t("wizard_step_security"),
    2: t("wizard_step_quick_unlock"),
    3: t("wizard_step_data"),
    4: t("wizard_done"),
  });

  let biometricAvailable = $state(false);
  let pending = $state<Quick | null>(null);
  let qPassword = $state("");
  let qPin = $state("");
  let qPinConfirm = $state("");
  let qBusy = $state(false);
  let qError = $state("");

  onMount(async () => {
    try {
      biometricAvailable = await api.biometricAvailable();
    } catch {
      biometricAvailable = false;
    }
  });

  const current = $derived(pending ?? ui.settings.quickUnlock);
  const pinFormatOk = $derived(/^\d{4,8}$/.test(qPin));
  const pinMatch = $derived(qPin === qPinConfirm);
  const canConfirm = $derived(
    !qBusy && qPassword.length > 0 && (pending !== "pin" || (pinFormatOk && pinMatch)),
  );

  function resetQuickForm() {
    pending = null;
    qPassword = "";
    qPin = "";
    qPinConfirm = "";
    qError = "";
  }

  async function chooseQuick(mode: Quick) {
    qError = "";
    if (mode === "off") {
      resetQuickForm();
      if (ui.settings.quickUnlock === "off") return;
      qBusy = true;
      try {
        await api.removeQuickUnlock();
        await saveSettings({ quickUnlock: "off" });
      } catch (err) {
        qError = errorText(err);
      } finally {
        qBusy = false;
      }
      return;
    }
    pending = mode;
  }

  async function confirmQuick() {
    if (!pending || pending === "off" || !canConfirm) return;
    const mode = pending;
    qBusy = true;
    qError = "";
    try {
      if (mode === "pin") await api.setupPin(qPassword, qPin);
      else await api.setupBiometric(qPassword);
      await saveSettings({ quickUnlock: mode });
      resetQuickForm();
    } catch (err) {
      qError = errorText(err);
    } finally {
      qBusy = false;
    }
  }

  const themes = $derived([
    { value: "system", label: t("theme_system"), icon: Monitor },
    { value: "light", label: t("theme_light"), icon: Sun },
    { value: "dark", label: t("theme_dark"), icon: Moon },
  ] as const);
  const lockOptions = [1, 5, 15, 30, 60, 0];
  const clipboardOptions = [15, 30, 60, 120, 0];

  function minutes(n: number) {
    return n === 0 ? t("never") : t("minutes", { n });
  }
  function seconds(n: number) {
    return n === 0 ? t("never") : t("seconds", { n });
  }

  async function finish() {
    await saveSettings({ setupDone: true });
    onDone();
  }
</script>

<AuthShell title={t("wizard_welcome")} subtitle="{account.user} · {host}">
  <div class="head">
    <h2>{titles[step]}</h2>
    <button class="ghost" onclick={finish}>{t("wizard_skip")}</button>
  </div>
  <div class="dots" aria-hidden="true">
    {#each steps as s, i (s)}<span class:on={i <= index}></span>{/each}
  </div>

  {#if step === 0}
    <div class="setting">
      <div class="text"><span>{t("theme")}</span></div>
      <div class="segmented" role="radiogroup" aria-label={t("theme")}>
        {#each themes as theme (theme.value)}
          {@const Icon = theme.icon}
          <button
            role="radio"
            aria-checked={ui.settings.theme === theme.value}
            class:on={ui.settings.theme === theme.value}
            onclick={() => saveSettings({ theme: theme.value })}
          >
            <Icon size={14} />{theme.label}
          </button>
        {/each}
      </div>
    </div>
    <div class="setting">
      <div class="text"><span>{t("language")}</span></div>
      <select value={ui.settings.language} onchange={(e) => saveSettings({ language: e.currentTarget.value })}>
        <option value="auto">{t("language_auto")}</option>
        {#each languages as lang (lang.code)}<option value={lang.code}>{lang.name}</option>{/each}
      </select>
    </div>
  {:else if step === 1}
    <div class="setting">
      <div class="text">
        <span>{t("auto_lock")}</span>
        <small class="muted">{vault.lockable ? t("auto_lock_hint") : t("auto_lock_unavailable")}</small>
      </div>
      <select
        value={ui.settings.lockMinutes}
        disabled={!vault.lockable}
        onchange={(e) => saveSettings({ lockMinutes: Number(e.currentTarget.value) })}
      >
        {#each lockOptions as n (n)}<option value={n}>{minutes(n)}</option>{/each}
      </select>
    </div>
    <div class="setting">
      <div class="text">
        <span>{t("clear_clipboard")}</span>
        <small class="muted">{t("clear_clipboard_hint")}</small>
      </div>
      <select
        value={ui.settings.clipboardSeconds}
        onchange={(e) => saveSettings({ clipboardSeconds: Number(e.currentTarget.value) })}
      >
        {#each clipboardOptions as n (n)}<option value={n}>{seconds(n)}</option>{/each}
      </select>
    </div>
  {:else if step === 2}
    <small class="muted">{t("quick_unlock_hint")}</small>
    <div class="segmented" role="radiogroup" aria-label={t("quick_unlock")}>
      <button role="radio" aria-checked={current === "off"} class:on={current === "off"} disabled={qBusy} onclick={() => chooseQuick("off")}>
        {t("quick_unlock_off")}
      </button>
      <button
        role="radio"
        aria-checked={current === "biometric"}
        class:on={current === "biometric"}
        disabled={qBusy || !biometricAvailable}
        onclick={() => chooseQuick("biometric")}
      >
        {t("quick_unlock_biometric")}
      </button>
      <button role="radio" aria-checked={current === "pin"} class:on={current === "pin"} disabled={qBusy} onclick={() => chooseQuick("pin")}>
        {t("quick_unlock_pin")}
      </button>
    </div>
    {#if !biometricAvailable}<small class="muted">{t("biometric_unavailable")}</small>{/if}
    {#if pending}
      <form
        class="quick"
        onsubmit={(e) => {
          e.preventDefault();
          confirmQuick();
        }}
      >
        <input
          type="password"
          bind:value={qPassword}
          placeholder={t("encryption_password")}
          autocomplete="current-password"
          disabled={qBusy}
          required
        />
        {#if pending === "pin"}
          <input
            type="password"
            bind:value={qPin}
            placeholder={t("pin_set")}
            inputmode="numeric"
            maxlength={8}
            autocomplete="off"
            disabled={qBusy}
            required
          />
          <input
            type="password"
            bind:value={qPinConfirm}
            placeholder={t("pin_confirm")}
            inputmode="numeric"
            maxlength={8}
            autocomplete="off"
            disabled={qBusy}
            required
          />
          {#if qPinConfirm && !pinMatch}
            <small class="error">{t("pin_mismatch")}</small>
          {/if}
        {/if}
        <div class="actions">
          <button type="button" onclick={resetQuickForm} disabled={qBusy}>{t("cancel")}</button>
          <button class="primary" type="submit" disabled={!canConfirm}>{t("save")}</button>
        </div>
      </form>
    {/if}
    {#if qError}<small class="error">{qError}</small>{/if}
  {:else if step === 3}
    <div class="setting">
      <div class="text">
        <span>{t("offline_cache")}</span>
        <small class="muted">{t("offline_cache_hint")}</small>
      </div>
      <input type="checkbox" checked={ui.settings.offlineCache} onchange={(e) => saveSettings({ offlineCache: e.currentTarget.checked })} />
    </div>
    <div class="setting">
      <div class="text">
        <span>{t("show_favicons")}</span>
        <small class="muted">{t("show_favicons_hint")}</small>
      </div>
      <input type="checkbox" checked={ui.settings.favicons} onchange={(e) => saveSettings({ favicons: e.currentTarget.checked })} />
    </div>
  {:else}
    <div class="done"><CircleCheck size={40} /></div>
  {/if}

  <div class="nav">
    <button onclick={() => (index -= 1)} disabled={index === 0 || qBusy}>{t("wizard_back")}</button>
    {#if last}
      <button class="primary" onclick={finish}>{t("wizard_start")}</button>
    {:else}
      <button class="primary" onclick={() => (index += 1)} disabled={qBusy}>{t("wizard_next")}</button>
    {/if}
  </div>
</AuthShell>

<style>
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  h2 {
    font-size: 16px;
    font-weight: 600;
  }
  .dots {
    display: flex;
    gap: 6px;
  }
  .dots span {
    flex: 1;
    height: 3px;
    border-radius: 2px;
    background: hsl(var(--muted));
  }
  .dots span.on {
    background: hsl(var(--primary));
  }
  .setting {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    min-height: 48px;
  }
  .text {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .text span {
    font-weight: 500;
  }
  small {
    font-size: 12px;
    line-height: 1.4;
  }
  select {
    width: 150px;
    height: 36px;
    flex: none;
  }
  .segmented {
    display: flex;
    padding: 3px;
    gap: 2px;
    border-radius: var(--radius);
    background: hsl(var(--muted));
  }
  .segmented button {
    height: 30px;
    padding: 0 10px;
    gap: 6px;
    font-size: 13px;
    border: none;
    background: transparent;
    color: hsl(var(--muted-foreground));
  }
  .segmented button.on {
    background: hsl(var(--background));
    color: hsl(var(--foreground));
    box-shadow: var(--shadow);
  }
  .quick {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .quick .actions,
  .nav {
    display: flex;
    justify-content: space-between;
    gap: 8px;
  }
  .quick .actions {
    justify-content: flex-end;
  }
  .done {
    display: grid;
    place-items: center;
    padding: 16px 0;
    color: hsl(var(--primary));
  }
</style>
