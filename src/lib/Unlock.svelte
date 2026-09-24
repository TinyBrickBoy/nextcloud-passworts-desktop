<script lang="ts">
  import { onMount } from "svelte";
  import { api, errorText, type Account, type UnlockRequirements, type VaultView } from "./api";

  let {
    account,
    onUnlocked,
    onLogout,
  }: { account: Account; onUnlocked: (v: VaultView) => void; onLogout: () => void } = $props();

  let req = $state<UnlockRequirements | null>(null);
  let password = $state("");
  let tokenId = $state("");
  let code = $state("");
  let busy = $state(false);
  let error = $state("");
  let passwordInput = $state<HTMLInputElement>();

  const host = $derived(account.server.replace(/^https?:\/\//, ""));

  onMount(load);

  async function load() {
    error = "";
    busy = true;
    try {
      req = await api.unlockRequirements();
      tokenId = req.tokens[0]?.id ?? "";
      if (!req.needsPassword && req.tokens.length === 0) {
        onUnlocked(await api.unlock(null, null));
        return;
      }
    } catch (err) {
      error = errorText(err);
    } finally {
      busy = false;
    }
    passwordInput?.focus();
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
      passwordInput?.select();
    }
  }
</script>

<main class="center">
  <form class="stack" onsubmit={submit}>
    <div>
      <h1>Gesperrt</h1>
      <p class="muted">{account.user} · {host}</p>
    </div>

    {#if req?.needsPassword}
      <div>
        <label for="pw">Verschlüsselungspasswort</label>
        <input id="pw" type="password" bind:this={passwordInput} bind:value={password} disabled={busy} required />
      </div>
    {/if}

    {#if req && req.tokens.length > 0}
      {#if req.tokens.length > 1}
        <div>
          <label for="token">Zweiter Faktor</label>
          <select id="token" bind:value={tokenId} disabled={busy}>
            {#each req.tokens as t (t.id)}<option value={t.id}>{t.label || t.type}</option>{/each}
          </select>
        </div>
      {/if}
      <div>
        <label for="code">Code</label>
        <input id="code" bind:value={code} inputmode="numeric" autocomplete="one-time-code" disabled={busy} required />
      </div>
    {/if}

    {#if req === null && !error}
      <p class="muted">Verbinde …</p>
    {:else if req}
      <button class="primary" type="submit" disabled={busy}>{busy ? "Entsperre …" : "Entsperren"}</button>
    {:else}
      <button type="button" onclick={load}>Erneut versuchen</button>
    {/if}

    {#if error}<p class="error">{error}</p>{/if}

    <button type="button" class="ghost muted" onclick={onLogout} disabled={busy}>Abmelden</button>
  </form>
</main>

<style>
  .center {
    height: 100%;
    display: grid;
    place-items: center;
    padding: 24px;
  }
  .stack {
    width: 100%;
    max-width: 320px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  h1 {
    margin: 0;
    font-size: 22px;
    font-weight: 600;
  }
  p {
    margin: 2px 0 0;
    overflow-wrap: anywhere;
  }
</style>
