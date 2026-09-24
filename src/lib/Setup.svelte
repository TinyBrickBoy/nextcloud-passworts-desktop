<script lang="ts">
  import { api, errorText, type Account } from "./api";

  let { onLoggedIn }: { onLoggedIn: (a: Account) => void } = $props();

  let server = $state("");
  let user = $state("");
  let appPassword = $state("");
  let manual = $state(false);
  let waiting = $state(false);
  let error = $state("");

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    error = "";
    waiting = true;
    try {
      onLoggedIn(manual ? await api.loginManual(server, user, appPassword) : await api.loginBrowser(server));
    } catch (err) {
      error = errorText(err);
    } finally {
      waiting = false;
    }
  }
</script>

<main class="center">
  <form class="stack" onsubmit={submit}>
    <div>
      <h1>Passwords</h1>
      <p class="muted">Mit deiner Nextcloud verbinden</p>
    </div>

    <div>
      <label for="server">Server</label>
      <!-- svelte-ignore a11y_autofocus -->
      <input id="server" bind:value={server} placeholder="cloud.example.com" autofocus required disabled={waiting} />
    </div>

    {#if manual}
      <div>
        <label for="user">Benutzername</label>
        <input id="user" bind:value={user} required disabled={waiting} autocomplete="off" />
      </div>
      <div>
        <label for="apppw">App Passwort</label>
        <input id="apppw" type="password" bind:value={appPassword} required disabled={waiting} />
        <p class="hint muted">Anlegen unter Einstellungen → Sicherheit → Geräte &amp; Sitzungen</p>
      </div>
    {/if}

    {#if waiting && !manual}
      <p class="muted">Bitte bestätige die Anmeldung im Browser …</p>
      <button type="button" onclick={() => api.loginCancel()}>Abbrechen</button>
    {:else}
      <button class="primary" type="submit" disabled={waiting}>
        {manual ? "Verbinden" : "Im Browser anmelden"}
      </button>
      <button type="button" class="ghost muted" onclick={() => (manual = !manual)} disabled={waiting}>
        {manual ? "Im Browser anmelden" : "Mit App Passwort anmelden"}
      </button>
    {/if}

    {#if error}<p class="error">{error}</p>{/if}
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
  }
  .hint {
    font-size: 12px;
    margin-top: 6px;
  }
</style>
