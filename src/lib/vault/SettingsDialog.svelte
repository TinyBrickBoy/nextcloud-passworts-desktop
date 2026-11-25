<script lang="ts">
  import { onMount } from "svelte";
  import { X, Sun, Moon, Monitor } from "@lucide/svelte";
  import { api, errorText, type Account } from "../api";
  import { languages, t } from "../i18n.svelte";
  import { saveSettings, ui } from "../store.svelte";

  let {
    account,
    lockable,
    encrypted,
    onClose,
  }: { account: Account; lockable: boolean; encrypted: boolean; onClose: () => void } = $props();

  type Quick = "off" | "biometric" | "pin";
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
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && onClose()} />

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="backdrop" onclick={(e) => e.target === e.currentTarget && onClose()}>
  <div class="dialog card" role="dialog" aria-modal="true" aria-labelledby="settings-title">
    <header>
      <h2 id="settings-title">{t("settings")}</h2>
      <button class="icon" onclick={onClose} aria-label={t("close")}><X size={18} /></button>
    </header>

    <div class="scroll">
    <section>
      <h3>{t("appearance")}</h3>
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
      <div class="setting">
        <div class="text">
          <span>{t("show_favicons")}</span>
          <small class="muted">{t("show_favicons_hint")}</small>
        </div>
        <input type="checkbox" checked={ui.settings.favicons} onchange={(e) => saveSettings({ favicons: e.currentTarget.checked })} />
      </div>
    </section>

    <section>
      <h3>{t("security")}</h3>
      <div class="setting">
        <div class="text">
          <span>{t("offline_cache")}</span>
          <small class="muted">{t("offline_cache_hint")}</small>
        </div>
        <input type="checkbox" checked={ui.settings.offlineCache} onchange={(e) => saveSettings({ offlineCache: e.currentTarget.checked })} />
      </div>
      <div class="setting">
        <div class="text">
          <span>{t("auto_lock")}</span>
          <small class="muted">{lockable ? t("auto_lock_hint") : t("auto_lock_unavailable")}</small>
        </div>
        <select
          value={ui.settings.lockMinutes}
          disabled={!lockable}
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
    </section>

    {#if encrypted}
      <section>
        <h3>{t("quick_unlock")}</h3>
        <small class="muted">{t("quick_unlock_hint")}</small>
        <div class="setting">
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
      </section>
    {/if}

    <section>
      <h3>{t("account")}</h3>
      <div class="setting">
        <div class="text">
          <span>{account.user}</span>
          <small class="muted selectable">{account.server}</small>
        </div>
      </div>
    </section>

    <footer class="muted">Passwords Desktop · {t("about_text")}</footer>
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 40;
    display: grid;
    place-items: center;
    padding: 24px;
    background: hsl(240 10% 3.9% / 0.45);
    animation: fade-in 0.12s ease-out;
  }
  .dialog {
    width: 100%;
    max-width: 520px;
    max-height: 100%;
    display: flex;
    flex-direction: column;
    padding: 0;
    box-shadow: var(--shadow-lg);
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 24px 12px;
    border-bottom: 1px solid hsl(var(--border));
    flex: none;
  }
  h2 {
    font-size: 18px;
    font-weight: 600;
  }
  .scroll {
    overflow-y: auto;
    padding: 0 24px 20px;
  }
  section {
    padding: 12px 0;
    border-bottom: 1px solid hsl(var(--border));
  }
  h3 {
    margin-bottom: 4px;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: hsl(var(--muted-foreground));
  }
  .setting {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    min-height: 48px;
    padding: 6px 0;
  }
  .text {
    display: flex;
    flex-direction: column;
    flex: 1;
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
    width: 170px;
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
    padding: 8px 0;
  }
  .quick .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  footer {
    padding-top: 14px;
    font-size: 12px;
  }
</style>
