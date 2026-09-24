<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { api, errorText, type Entry, type Folder, type VaultView } from "./api";

  let {
    entry,
    folders,
    defaultFolder,
    onSaved,
    onCancel,
  }: {
    entry: Entry | null;
    folders: Folder[];
    defaultFolder: string | null;
    onSaved: (id: string, v: VaultView) => void;
    onCancel: () => void;
  } = $props();

  // Die Props werden bewusst nur beim Öffnen übernommen, danach gehört der Zustand dem Formular.
  const initial = untrack(() => entry);
  let label = $state(initial?.label ?? "");
  let username = $state(initial?.username ?? "");
  let password = $state("");
  let original = "";
  let url = $state(initial?.url ?? "");
  let notes = $state(initial?.notes ?? "");
  let folder = $state(initial?.folder ?? untrack(() => defaultFolder) ?? "");
  let favorite = $state(initial?.favorite ?? false);
  let show = $state(!initial);
  let busy = $state(false);
  let error = $state("");
  let labelInput = $state<HTMLInputElement>();

  onMount(async () => {
    labelInput?.focus();
    if (initial) {
      original = await api.reveal(initial.id);
      password = original;
    }
  });

  async function generate() {
    try {
      password = await api.generate();
      show = true;
    } catch (err) {
      error = errorText(err);
    }
  }

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    error = "";
    busy = true;
    try {
      const result = await api.save({
        id: initial?.id ?? null,
        label,
        username,
        password: initial && password === original ? null : password,
        url,
        notes,
        folder: folder || null,
        favorite,
      });
      onSaved(result.id, result.vault);
    } catch (err) {
      error = errorText(err);
      busy = false;
    }
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") onCancel();
  }
</script>

<svelte:window onkeydown={onKey} />

<form onsubmit={submit}>
  <h2>{initial ? "Bearbeiten" : "Neuer Eintrag"}</h2>

  <div>
    <label for="label">Name</label>
    <input id="label" bind:this={labelInput} bind:value={label} required maxlength="64" />
  </div>

  <div>
    <label for="username">Benutzername</label>
    <input id="username" bind:value={username} autocomplete="off" spellcheck="false" maxlength="64" />
  </div>

  <div>
    <label for="password">Passwort</label>
    <div class="row">
      <input
        id="password"
        class="mono"
        type={show ? "text" : "password"}
        bind:value={password}
        required
        autocomplete="off"
        spellcheck="false"
        maxlength="256"
      />
      <button type="button" onclick={() => (show = !show)}>{show ? "Verbergen" : "Anzeigen"}</button>
      <button type="button" onclick={generate}>Generieren</button>
    </div>
  </div>

  <div>
    <label for="url">Adresse</label>
    <input id="url" bind:value={url} placeholder="https://" spellcheck="false" maxlength="2048" />
  </div>

  <div class="row">
    <div class="grow">
      <label for="folder">Ordner</label>
      <select id="folder" bind:value={folder}>
        <option value="">Kein Ordner</option>
        {#each folders as f (f.id)}<option value={f.id}>{f.label}</option>{/each}
      </select>
    </div>
    <label class="check"><input type="checkbox" bind:checked={favorite} /> Favorit</label>
  </div>

  <div>
    <label for="notes">Notizen</label>
    <textarea id="notes" bind:value={notes} rows="3" maxlength="4096"></textarea>
  </div>

  {#if error}<p class="error">{error}</p>{/if}

  <div class="row end">
    <button type="button" onclick={onCancel} disabled={busy}>Abbrechen</button>
    <button class="primary" type="submit" disabled={busy}>{busy ? "Speichere …" : "Speichern"}</button>
  </div>
</form>

<style>
  form {
    display: flex;
    flex-direction: column;
    gap: 12px;
    max-width: 520px;
  }
  textarea {
    min-height: 72px;
  }
  h2 {
    margin: 0 0 4px;
    font-size: 20px;
    font-weight: 600;
  }
  .row {
    display: flex;
    gap: 8px;
    align-items: flex-end;
  }
  .grow {
    flex: 1;
  }
  .end {
    justify-content: flex-end;
  }
  .check {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 36px;
    margin: 0 4px 0 8px;
    color: hsl(var(--foreground));
    font-size: 14px;
    white-space: nowrap;
  }
  .check input {
    width: 16px;
    height: 16px;
    margin: 0;
    accent-color: hsl(var(--primary));
  }
</style>
