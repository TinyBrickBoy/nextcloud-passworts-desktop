<script lang="ts">
  import { onMount } from "svelte";
  import { api, type Account, type VaultView } from "./lib/api";
  import Setup from "./lib/Setup.svelte";
  import Unlock from "./lib/Unlock.svelte";
  import Vault from "./lib/Vault.svelte";

  const IDLE_LOCK_MS = 5 * 60 * 1000;

  let screen = $state<"loading" | "setup" | "unlock" | "vault">("loading");
  let account = $state<Account | null>(null);
  let vault = $state<VaultView | null>(null);
  let idleTimer: ReturnType<typeof setTimeout> | undefined;

  onMount(async () => {
    const status = await api.status();
    account = status.account;
    screen = account ? "unlock" : "setup";

    const reset = () => {
      clearTimeout(idleTimer);
      if (screen === "vault") idleTimer = setTimeout(lock, IDLE_LOCK_MS);
    };
    for (const ev of ["mousemove", "mousedown", "keydown", "wheel"]) {
      window.addEventListener(ev, reset, { passive: true });
    }
    // Rechtsklick Menü des Webviews passt nicht zu einer Desktop App.
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
    idleTimer = setTimeout(lock, IDLE_LOCK_MS);
  }

  async function lock() {
    clearTimeout(idleTimer);
    await api.lock();
    vault = null;
    screen = account ? "unlock" : "setup";
  }

  async function logout() {
    clearTimeout(idleTimer);
    await api.logout();
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
