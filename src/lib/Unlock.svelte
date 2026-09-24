<script lang="ts">
  import { onMount } from "svelte";
  import { LoaderCircle, LockKeyholeOpen, Eye, EyeOff } from "@lucide/svelte";
  import { api, errorText, type Account, type UnlockRequirements, type VaultView } from "./api";
  import { t } from "./i18n.svelte";
  import AuthShell from "./components/AuthShell.svelte";

  let {
    account,
    onUnlocked,
    onLogout,
  }: { account: Account; onUnlocked: (v: VaultView) => void; onLogout: () => void } = $props();

  let req = $state<UnlockRequirements | null>(null);
  let password = $state("");
  let show = $state(false);
  let tokenId = $state("");
  let code = $state("");
  let busy = $state(false);
  let error = $state("");
  let passwordInput = $state<HTMLInputElement>();

  const host = $derived(account.server.replace(/^https?:\/\//, ""));
  const needsInput = $derived(!!req && (req.needsPassword || req.tokens.length > 0));

  onMount(load);

  async function load() {
    error = "";
    busy = true;
    try {
      req = await api.unlockRequirements();
      tokenId = req.tokens[0]?.id ?? "";
      if (!req.needsPassword && req.tokens.length === 0) {
        // Nichts abzufragen: direkt öffnen.
        onUnlocked(await api.unlock(null, null));
        return;
      }
    } catch (err) {
      error = errorText(err);
    } finally {
      busy = false;
    }
    queueMicrotask(() => passwordInput?.focus());
  }

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    if (!req) return;
    error = "";
    busy = true;
    try {
      const token = req.tokens.length ? { id: tokenId, code } : null;
      onUnlocked(await api.unlock(req.needsPassword ? password : null, token));
      password = "";
    } catch (err) {
      error = errorText(err);
      busy = false;
      queueMicrotask(() => passwordInput?.select());
    }
  }
</script>

<AuthShell title={needsInput ? t("locked_title") : t("opening")} subtitle="{account.user} · {host}">
  <form onsubmit={submit}>
    {#if req?.needsPassword}
      <div>
        <label for="pw">{t("encryption_password")}</label>
        <div class="password">
          <input
            id="pw"
            type={show ? "text" : "password"}
            bind:this={passwordInput}
            bind:value={password}
            disabled={busy}
            required
            autocomplete="current-password"
          />
          <button type="button" class="icon" onclick={() => (show = !show)} aria-label={show ? t("hide") : t("show")}>
            {#if show}<EyeOff size={16} />{:else}<Eye size={16} />{/if}
          </button>
        </div>
        <p class="hint muted">{t("encryption_password_hint")}</p>
      </div>
    {/if}

    {#if req && req.tokens.length > 0}
      {#if req.tokens.length > 1}
        <div>
          <label for="token">{t("second_factor")}</label>
          <select id="token" bind:value={tokenId} disabled={busy}>
            {#each req.tokens as tk (tk.id)}<option value={tk.id}>{tk.label || tk.type}</option>{/each}
          </select>
        </div>
      {/if}
      <div>
        <label for="code">{t("code")}</label>
        <input
          id="code"
          bind:value={code}
          inputmode="numeric"
          autocomplete="one-time-code"
          disabled={busy}
          required
          class="mono"
        />
      </div>
    {/if}

    {#if !req && !error}
      <div class="waiting"><LoaderCircle size={16} class="spin" /><span>{t("connecting")}</span></div>
    {:else if needsInput}
      <button class="primary" type="submit" disabled={busy}>
        {#if busy}<LoaderCircle size={16} class="spin" />{t("unlocking")}{:else}<LockKeyholeOpen size={16} />{t("unlock")}{/if}
      </button>
    {:else if busy}
      <div class="waiting"><LoaderCircle size={16} class="spin" /><span>{t("loading_vault")}</span></div>
    {:else}
      <button type="button" onclick={load}>{t("retry")}</button>
    {/if}

    {#if error}<p class="error">{error}</p>{/if}
  </form>

  <button type="button" class="ghost switch" onclick={onLogout} disabled={busy}>{t("logout")}</button>
</AuthShell>

<style>
  form {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .password {
    position: relative;
  }
  .password input {
    padding-right: 42px;
  }
  .password button {
    position: absolute;
    right: 4px;
    top: 4px;
  }
  .hint {
    margin: 6px 0 0;
    font-size: 12px;
  }
  .waiting {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 13px;
    color: hsl(var(--muted-foreground));
  }
  .switch {
    align-self: center;
    font-size: 13px;
    color: hsl(var(--muted-foreground));
    margin-top: -6px;
  }
</style>
