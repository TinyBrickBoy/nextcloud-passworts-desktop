<script lang="ts">
  import { onMount } from "svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { api, errorText, type Account, type CopyField, type Entry, type VaultView } from "./api";
  import Editor from "./Editor.svelte";

  let {
    vault = $bindable(),
    account,
    onLock,
    onLogout,
  }: { vault: VaultView; account: Account; onLock: () => void; onLogout: () => void } = $props();

  type Filter = "all" | "favorites" | string;

  let filter = $state<Filter>("all");
  let query = $state("");
  let selectedId = $state<string | null>(null);
  let editing = $state<Entry | "new" | null>(null);
  let revealed = $state<string | null>(null);
  let revealedFields = $state<Record<number, string>>({});
  let toast = $state("");
  let error = $state("");
  let busy = $state(false);
  let searchInput = $state<HTMLInputElement>();
  let toastTimer: ReturnType<typeof setTimeout> | undefined;

  const folderName = $derived(new Map(vault.folders.map((f) => [f.id, f.label])));

  const visible = $derived.by(() => {
    const q = query.trim().toLowerCase();
    return vault.entries.filter((e) => {
      if (filter === "favorites" && !e.favorite) return false;
      if (filter !== "all" && filter !== "favorites" && e.folder !== filter) return false;
      if (!q) return true;
      return [e.label, e.username, e.url, e.notes].some((v) => v.toLowerCase().includes(q));
    });
  });

  const selected = $derived(vault.entries.find((e) => e.id === selectedId) ?? null);

  $effect(() => {
    // Anzeige zurücksetzen, sobald ein anderer Eintrag gewählt wird.
    selectedId;
    revealed = null;
    revealedFields = {};
  });

  function notify(text: string) {
    toast = text;
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => (toast = ""), 2500);
  }

  async function copy(field: CopyField) {
    if (!selected) return;
    await api.copy(selected.id, field);
    const names = { password: "Passwort", username: "Benutzername", url: "Adresse" };
    notify(`${names[field]} kopiert${field === "password" ? " · wird in 30 s geleert" : ""}`);
  }

  async function copyCustom(index: number, label: string) {
    if (!selected) return;
    await api.copyField(selected.id, index);
    notify(`${label} kopiert · wird in 30 s geleert`);
  }

  async function toggleReveal() {
    if (!selected) return;
    revealed = revealed === null ? await api.reveal(selected.id) : null;
  }

  async function toggleRevealField(index: number) {
    if (!selected) return;
    if (index in revealedFields) {
      const { [index]: _, ...rest } = revealedFields;
      revealedFields = rest;
    } else {
      revealedFields = { ...revealedFields, [index]: await api.revealField(selected.id, index) };
    }
  }

  async function trash() {
    if (!selected || !confirm(`„${selected.label}“ in den Papierkorb verschieben?`)) return;
    busy = true;
    try {
      vault = await api.trash(selected.id);
      selectedId = null;
      notify("In den Papierkorb verschoben");
    } catch (err) {
      error = errorText(err);
    } finally {
      busy = false;
    }
  }

  async function reload() {
    busy = true;
    error = "";
    try {
      vault = await api.refresh();
      notify("Aktualisiert");
    } catch (err) {
      error = errorText(err);
    } finally {
      busy = false;
    }
  }

  function onSaved(id: string, v: VaultView) {
    vault = v;
    editing = null;
    selectedId = id;
    notify("Gespeichert");
  }

  function open(url: string) {
    const target = /^[a-z][a-z0-9+.-]*:/i.test(url) ? url : `https://${url}`;
    openUrl(target).catch((err) => (error = errorText(err)));
  }

  function host(url: string) {
    try {
      return new URL(/^[a-z]+:\/\//i.test(url) ? url : `https://${url}`).host;
    } catch {
      return url;
    }
  }

  function edited(ts: number) {
    return ts ? new Date(ts * 1000).toLocaleDateString("de-DE", { dateStyle: "medium" }) : "–";
  }

  function move(delta: number) {
    if (!visible.length) return;
    const i = visible.findIndex((e) => e.id === selectedId);
    const next = Math.min(visible.length - 1, Math.max(0, i + delta));
    selectedId = visible[next].id;
    document.getElementById(`entry-${selectedId}`)?.scrollIntoView({ block: "nearest" });
  }

  function onKey(e: KeyboardEvent) {
    if (editing) return;
    const mod = e.ctrlKey || e.metaKey;
    const inField = e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement;
    if (mod && e.key === "f") {
      e.preventDefault();
      searchInput?.focus();
      searchInput?.select();
    } else if (mod && e.key === "n") {
      e.preventDefault();
      editing = "new";
    } else if (mod && e.key === "l") {
      e.preventDefault();
      onLock();
    } else if (mod && e.key === "c" && selected && !inField && !window.getSelection()?.toString()) {
      e.preventDefault();
      copy("password");
    } else if (mod && e.key === "b" && selected) {
      e.preventDefault();
      copy("username");
    } else if (e.key === "ArrowDown" && (!inField || e.target === searchInput)) {
      e.preventDefault();
      move(1);
    } else if (e.key === "ArrowUp" && (!inField || e.target === searchInput)) {
      e.preventDefault();
      move(-1);
    } else if (e.key === "Enter" && e.target === searchInput && selected) {
      copy("password");
    } else if (e.key === "Escape" && e.target === searchInput) {
      query = "";
    }
  }

  onMount(() => searchInput?.focus());
</script>

<svelte:window onkeydown={onKey} />

<div class="layout">
  <nav class="sidebar">
    <button class="nav" class:active={filter === "all"} onclick={() => (filter = "all")}>
      Alle <span class="count">{vault.entries.length}</span>
    </button>
    <button class="nav" class:active={filter === "favorites"} onclick={() => (filter = "favorites")}>Favoriten</button>

    {#if vault.folders.length}
      <div class="section">Ordner</div>
      {#each vault.folders as f (f.id)}
        <button class="nav" class:active={filter === f.id} onclick={() => (filter = f.id)}>{f.label}</button>
      {/each}
    {/if}

    <div class="spacer"></div>
    <div class="account muted" title={account.server}>{account.user}</div>
    <div class="actions">
      <button class="ghost" onclick={reload} disabled={busy} title="Aktualisieren">Aktualisieren</button>
      <button class="ghost" onclick={onLock} title="Sperren (Strg+L)">Sperren</button>
    </div>
    <button class="ghost muted small" onclick={() => confirm("Konto von diesem Gerät entfernen?") && onLogout()}>
      Abmelden
    </button>
  </nav>

  <section class="list">
    <div class="search">
      <input bind:this={searchInput} bind:value={query} placeholder="Suchen" spellcheck="false" />
      <button class="primary" onclick={() => (editing = "new")} title="Neu (Strg+N)">Neu</button>
    </div>
    <div class="entries">
      {#each visible as e (e.id)}
        <button id="entry-{e.id}" class="entry" class:active={e.id === selectedId} onclick={() => (selectedId = e.id)}>
          <span class="label">{e.label}</span>
          <span class="sub muted">{e.username || host(e.url) || " "}</span>
        </button>
      {:else}
        <p class="empty muted">{query ? "Keine Treffer" : "Keine Einträge"}</p>
      {/each}
    </div>
    {#if vault.broken > 0}
      <p class="broken">{vault.broken} Einträge konnten nicht entschlüsselt werden</p>
    {/if}
  </section>

  <section class="detail">
    {#if editing}
      <Editor
        entry={editing === "new" ? null : editing}
        folders={vault.folders}
        defaultFolder={filter !== "all" && filter !== "favorites" ? filter : null}
        {onSaved}
        onCancel={() => (editing = null)}
      />
    {:else if selected}
      <header>
        <div>
          <h2 class="selectable">{selected.label}</h2>
          {#if selected.folder && folderName.get(selected.folder)}
            <span class="muted">{folderName.get(selected.folder)}</span>
          {/if}
        </div>
        <div class="row">
          {#if selected.editable}<button onclick={() => (editing = selected)}>Bearbeiten</button>{/if}
          <button class="danger" onclick={trash} disabled={busy}>Löschen</button>
        </div>
      </header>

      {#if selected.status === 2}
        <p class="status bad">Dieses Passwort wurde in einem Datenleck gefunden</p>
      {:else if selected.status === 1}
        <p class="status warn">Dieses Passwort ist doppelt vergeben oder veraltet</p>
      {/if}

      <dl>
        {#if selected.username}
          <dt>Benutzername</dt>
          <dd>
            <span class="value selectable">{selected.username}</span>
            <button class="ghost" onclick={() => copy("username")}>Kopieren</button>
          </dd>
        {/if}

        <dt>Passwort</dt>
        <dd>
          <span class="value mono selectable">{revealed ?? "••••••••••••"}</span>
          <button class="ghost" onclick={toggleReveal}>{revealed === null ? "Anzeigen" : "Verbergen"}</button>
          <button class="ghost" onclick={() => copy("password")}>Kopieren</button>
        </dd>

        {#if selected.url}
          <dt>Adresse</dt>
          <dd>
            <span class="value selectable">{selected.url}</span>
            <button class="ghost" onclick={() => open(selected.url)}>Öffnen</button>
            <button class="ghost" onclick={() => copy("url")}>Kopieren</button>
          </dd>
        {/if}

        {#each selected.fields as f (f.index)}
          <dt>{f.label}</dt>
          <dd>
            {#if f.type === "secret"}
              <span class="value mono selectable">{revealedFields[f.index] ?? "••••••••"}</span>
              <button class="ghost" onclick={() => toggleRevealField(f.index)}>
                {f.index in revealedFields ? "Verbergen" : "Anzeigen"}
              </button>
            {:else}
              <span class="value selectable">{f.value}</span>
            {/if}
            <button class="ghost" onclick={() => copyCustom(f.index, f.label)}>Kopieren</button>
          </dd>
        {/each}

        {#if selected.notes}
          <dt>Notizen</dt>
          <dd><p class="notes selectable">{selected.notes}</p></dd>
        {/if}
      </dl>

      <p class="meta muted">Zuletzt geändert {edited(selected.edited)}</p>
    {:else}
      <div class="placeholder muted">
        <p>Eintrag auswählen</p>
        <p class="keys">Strg+F Suchen · Strg+N Neu · Strg+C Passwort kopieren · Strg+B Benutzername · Strg+L Sperren</p>
      </div>
    {/if}

    {#if error}<p class="error">{error}</p>{/if}
  </section>

  {#if toast}<div class="toast">{toast}</div>{/if}
</div>

<style>
  .layout {
    height: 100%;
    display: grid;
    grid-template-columns: 190px 300px 1fr;
  }

  .sidebar {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 16px 10px;
    background: hsl(var(--muted) / 0.4);
    border-right: 1px solid hsl(var(--border));
    overflow-y: auto;
  }
  .nav {
    width: 100%;
    height: 34px;
    padding: 0 12px;
    justify-content: space-between;
    font-weight: 400;
    border: none;
    text-align: left;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .nav.active {
    background: hsl(var(--accent));
    font-weight: 600;
  }
  .count {
    color: hsl(var(--muted-foreground));
    font-weight: normal;
  }
  .section {
    margin: 14px 12px 4px;
    font-size: 12px;
    color: hsl(var(--muted-foreground));
  }
  .spacer {
    flex: 1;
  }
  .account {
    padding: 0 12px 6px;
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .actions {
    display: flex;
    gap: 2px;
  }
  .actions button {
    flex: 1;
    padding-inline: 6px;
    font-weight: 400;
  }
  .small {
    height: 30px;
    font-size: 12px;
    font-weight: 400;
  }

  .list {
    display: flex;
    flex-direction: column;
    border-right: 1px solid hsl(var(--border));
    min-height: 0;
  }
  .search {
    display: flex;
    gap: 8px;
    padding: 12px;
    border-bottom: 1px solid hsl(var(--border));
  }
  .entries {
    flex: 1;
    overflow-y: auto;
    padding: 6px;
  }
  .entry {
    flex-direction: column;
    align-items: flex-start;
    gap: 0;
    width: 100%;
    height: auto;
    border: none;
    text-align: left;
    padding: 8px 12px;
    font-weight: 400;
  }
  .entry .label {
    font-weight: 500;
  }
  .entry.active {
    background: hsl(var(--accent));
  }
  .label,
  .sub {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 100%;
  }
  .sub {
    font-size: 12px;
  }
  .empty {
    text-align: center;
    margin-top: 32px;
  }
  .broken {
    margin: 0;
    padding: 8px 12px;
    font-size: 12px;
    color: hsl(var(--warning));
    border-top: 1px solid hsl(var(--border));
  }

  .detail {
    padding: 32px 40px;
    overflow-y: auto;
    min-width: 0;
  }
  header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 16px;
    margin-bottom: 20px;
  }
  h2 {
    margin: 0;
    font-size: 20px;
    font-weight: 600;
    overflow-wrap: anywhere;
  }
  .row {
    display: flex;
    gap: 6px;
  }
  .status {
    margin: 0 0 16px;
    font-size: 13px;
  }
  .bad {
    color: hsl(var(--destructive));
  }
  .warn {
    color: hsl(var(--warning));
  }
  dl {
    margin: 0;
  }
  dt {
    color: hsl(var(--muted-foreground));
    font-size: 12px;
    margin-top: 14px;
  }
  dd {
    margin: 2px 0 0;
    display: flex;
    align-items: center;
    gap: 2px;
    border-bottom: 1px solid hsl(var(--border));
    padding-bottom: 6px;
  }
  .value {
    flex: 1;
    min-width: 0;
    overflow-wrap: anywhere;
  }
  dd button {
    height: 28px;
    padding: 0 10px;
    font-size: 12px;
    color: hsl(var(--muted-foreground));
  }
  .notes {
    margin: 0;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .meta {
    margin-top: 24px;
    font-size: 12px;
  }
  .placeholder {
    height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
  }
  .keys {
    font-size: 12px;
    max-width: 320px;
  }

  .toast {
    position: fixed;
    bottom: 16px;
    left: 50%;
    transform: translateX(-50%);
    padding: 8px 14px;
    background: hsl(var(--primary));
    color: hsl(var(--primary-foreground));
    border-radius: var(--radius);
    font-size: 13px;
  }
</style>
