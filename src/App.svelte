<script lang="ts">
  import { onMount } from "svelte";
  import { api, type Account, type VaultView } from "./lib/api";
  import { systemLanguage } from "./lib/i18n.svelte";
  import { applySettings, ui } from "./lib/store.svelte";
  import Setup from "./lib/Setup.svelte";
  import Unlock from "./lib/Unlock.svelte";
  import Vault from "./lib/Vault.svelte";
  import DialogHost from "./lib/components/DialogHost.svelte";
  import Toast from "./lib/components/Toast.svelte";
  import { clearFaviconCache } from "./lib/components/Avatar.svelte";

  let screen = $state<"loading" | "setup" | "unlock" | "vault">("loading");
  let account = $state<Account | null>(null);
  let vault = $state<VaultView | null>(null);
  let idleTimer: ReturnType<typeof setTimeout> | undefined;

  // Automatisch sperren nur, wenn das Konto beim Entsperren eine Eingabe verlangt.
  const autoLock = $derived(screen === "vault" && !!vault?.lockable && ui.settings.lockMinutes > 0);

  function resetIdle() {
    clearTimeout(idleTimer);
    if (autoLock) idleTimer = setTimeout(lock, ui.settings.lockMinutes * 60_000);
  }

  $effect(() => {
    // Neu starten, wenn sich Einstellung oder Zustand ändern.
    void autoLock;
    void ui.settings.lockMinutes;
    resetIdle();
  });

  onMount(async () => {
    const status = await api.status();
    applySettings(status.settings);
    api.setSystemLanguage(systemLanguage()).catch(() => {});
    account = status.account;
    screen = account ? "unlock" : "setup";

    for (const ev of ["mousemove", "mousedown", "keydown", "wheel"]) {
      window.addEventListener(ev, resetIdle, { passive: true });
    }
    // Das Kontextmenü des Webviews passt nicht zu einer Desktop App.
    window.addEventListener("contextmenu", (e) => {
      if (!(e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement)) e.preventDefault();
    });
  });

  function onLoggedIn(a: Account) {
    account = a;
    screen = "unlock";
  }

  function onUnlocked(v: VaultView) {
    vault = v;
    screen = "vault";
  }

  async function lock() {
    clearTimeout(idleTimer);
    await api.lock();
    clearFaviconCache();
    vault = null;
    screen = account ? "unlock" : "setup";
  }

  async function logout() {
    clearTimeout(idleTimer);
    await api.logout();
    clearFaviconCache();
    vault = null;
    account = null;
    screen = "setup";
  }
</script>

{#if screen === "setup"}
  <Setup {onLoggedIn} />
{:else if screen === "unlock" && account}
  <Unlock {account} {onUnlocked} onLogout={logout} />
{:else if screen === "vault" && vault && account}
  <Vault bind:vault {account} onLock={lock} onLogout={logout} />
{/if}

<DialogHost />
<Toast />
