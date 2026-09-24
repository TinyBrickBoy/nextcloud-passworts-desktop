<script lang="ts">
  import { Globe, LoaderCircle, ExternalLink } from "@lucide/svelte";
  import { api, errorText, type Account } from "./api";
  import { t } from "./i18n.svelte";
  import AuthShell from "./components/AuthShell.svelte";

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

<AuthShell title="Passwords" subtitle={t("setup_subtitle")}>
  <form onsubmit={submit}>
    <div>
      <label for="server">{t("server")}</label>
      <div class="with-icon">
        <Globe size={16} />
        <!-- svelte-ignore a11y_autofocus -->
        <input
          id="server"
          bind:value={server}
          placeholder="cloud.example.com"
          autofocus
          required
          disabled={waiting}
          spellcheck="false"
          autocomplete="url"
        />
      </div>
    </div>

    {#if manual}
      <div>
        <label for="user">{t("username")}</label>
        <input id="user" bind:value={user} required disabled={waiting} autocomplete="username" spellcheck="false" />
      </div>
      <div>
        <label for="apppw">{t("app_password")}</label>
        <input id="apppw" type="password" bind:value={appPassword} required disabled={waiting} />
        <p class="hint muted">{t("app_password_hint")}</p>
      </div>
    {/if}

    {#if waiting && !manual}
      <div class="waiting">
        <LoaderCircle size={16} class="spin" />
        <span>{t("waiting_for_browser")}</span>
      </div>
      <button type="button" onclick={() => api.loginCancel()}>{t("cancel")}</button>
    {:else}
      <button class="primary" type="submit" disabled={waiting}>
        {#if waiting}<LoaderCircle size={16} class="spin" />{:else if !manual}<ExternalLink size={16} />{/if}
        {manual ? t("connect") : t("login_browser")}
      </button>
    {/if}

    {#if error}<p class="error">{error}</p>{/if}
  </form>

  <button type="button" class="ghost switch" onclick={() => (manual = !manual)} disabled={waiting}>
    {manual ? t("login_browser_instead") : t("login_app_password")}
  </button>
</AuthShell>

<style>
  form {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .with-icon {
    position: relative;
  }
  .with-icon :global(svg) {
    position: absolute;
    left: 12px;
    top: 50%;
    transform: translateY(-50%);
    color: hsl(var(--muted-foreground));
    pointer-events: none;
  }
  .with-icon input {
    padding-left: 36px;
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
  :global(.spin) {
    animation: spin 0.9s linear infinite;
  }
  @keyframes -global-spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
