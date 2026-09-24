<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { Eye, EyeOff, Wand2, Plus, X, Star, LoaderCircle } from "@lucide/svelte";
  import { api, errorText, type Entry, type FieldInput, type VaultView } from "../api";
  import { t } from "../i18n.svelte";
  import Strength from "../components/Strength.svelte";
  import Generator from "./Generator.svelte";

  let {
    entry,
    vault,
    defaultFolder,
    defaultTag,
    onSaved,
    onCancel,
    onCreateTag,
  }: {
    entry: Entry | null;
    vault: VaultView;
    defaultFolder: string | null;
    defaultTag: string | null;
    onSaved: (id: string, v: VaultView) => void;
    onCancel: () => void;
    onCreateTag: () => Promise<string | null>;
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
  let tags = $state<string[]>(initial ? [...initial.tags] : untrack(() => (defaultTag ? [defaultTag] : [])));
  type EditField = { label: string; type: string; value: string; index: number | null; touched: boolean };
  let fields = $state<EditField[]>(
    (initial?.fields ?? []).map((f) => ({ label: f.label, type: f.type, value: f.value, index: f.index, touched: false })),
  );
  let fieldsChanged = $state(false);
  let show = $state(!initial);
  let showGenerator = $state(!initial);
  let busy = $state(false);
  let error = $state("");
  let labelInput = $state<HTMLInputElement>();

  const fieldTypes = $derived([
    { value: "text", label: t("field_text") },
    { value: "secret", label: t("field_secret") },
    { value: "email", label: t("field_email") },
    { value: "url", label: t("field_url") },
  ]);
  const availableTags = $derived(vault.tags.filter((tag) => !tags.includes(tag.id)));

  onMount(async () => {
    labelInput?.focus();
    if (initial) {
      original = await api.reveal(initial.id);
      password = original;
    }
  });

  function addField() {
    fields.push({ label: "", type: "text", value: "", index: null, touched: true });
    fieldsChanged = true;
  }

  function removeField(i: number) {
    fields.splice(i, 1);
    fieldsChanged = true;
  }

  async function addTag(e: Event) {
    const select = e.currentTarget as HTMLSelectElement;
    const value = select.value;
    select.value = "";
    if (value === "__new") {
      const id = await onCreateTag();
      if (id) tags.push(id);
    } else if (value) {
      tags.push(value);
    }
  }

  async function submit(e?: Event) {
    e?.preventDefault();
    error = "";
    busy = true;
    try {
      const fieldInputs: FieldInput[] | null = fieldsChanged
        ? fields.map((f) => ({
            label: f.label,
            type: f.type,
            // Geheime Felder, die nicht angefasst wurden, kennt das Backend selbst.
            value: f.type === "secret" && !f.touched && f.index !== null ? null : f.value,
            index: f.index,
          }))
        : null;
      const result = await api.save({
        id: initial?.id ?? null,
        label,
        username,
        password: initial && password === original ? null : password,
        url,
        notes,
        folder: folder || null,
        favorite,
        tags,
        fields: fieldInputs,
      });
      onSaved(result.id, result.vault);
    } catch (err) {
      error = errorText(err);
      busy = false;
    }
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape" && !document.querySelector("[role=dialog]")) onCancel();
    if ((e.ctrlKey || e.metaKey) && e.key === "s") {
      e.preventDefault();
      submit();
    }
  }
</script>

<svelte:window onkeydown={onKey} />

<form class="editor" onsubmit={submit}>
  <header>
    <h2>{initial ? t("edit_entry") : t("new_entry")}</h2>
    <button
      type="button"
      class="icon fav"
      class:active={favorite}
      onclick={() => (favorite = !favorite)}
      aria-label={t("favorite")}
      title={t("favorite")}
    >
      <Star size={18} />
    </button>
  </header>

  <section class="card group">
    <div>
      <label for="label">{t("name")}</label>
      <input id="label" bind:this={labelInput} bind:value={label} required maxlength="64" placeholder="GitHub" />
    </div>
    <div>
      <label for="username">{t("username")}</label>
      <input id="username" bind:value={username} autocomplete="off" spellcheck="false" maxlength="64" />
    </div>
    <div>
      <label for="password">{t("password")}</label>
      <div class="password">
        <input
          id="password"
          class="mono"
          type={show ? "text" : "password"}
          bind:value={password}
          required
          autocomplete="new-password"
          spellcheck="false"
          maxlength="256"
        />
        <button type="button" class="icon" onclick={() => (show = !show)} aria-label={show ? t("hide") : t("show")}>
          {#if show}<EyeOff size={16} />{:else}<Eye size={16} />{/if}
        </button>
        <button
          type="button"
          class="icon"
          class:active={showGenerator}
          onclick={() => (showGenerator = !showGenerator)}
          aria-label={t("generator")}
          title={t("generator")}
        >
          <Wand2 size={16} />
        </button>
      </div>
      <div class="below"><Strength {password} /></div>
      {#if showGenerator}
        <Generator
          onUse={(pw) => {
            password = pw;
            show = true;
          }}
        />
      {/if}
    </div>
    <div>
      <label for="url">{t("website")}</label>
      <input id="url" bind:value={url} placeholder="https://" spellcheck="false" maxlength="2048" />
    </div>
  </section>

  <section class="card group">
    <div class="two">
      <div>
        <label for="folder">{t("folder")}</label>
        <select id="folder" bind:value={folder}>
          <option value="">{t("no_folder")}</option>
          {#each vault.folders as f (f.id)}<option value={f.id}>{f.label}</option>{/each}
        </select>
      </div>
      <div>
        <label for="add-tag">{t("tags")}</label>
        <select id="add-tag" onchange={addTag}>
          <option value="">{t("add_tag")}</option>
          {#each availableTags as tag (tag.id)}<option value={tag.id}>{tag.label}</option>{/each}
          <option value="__new">{t("new_tag")} …</option>
        </select>
      </div>
    </div>
    {#if tags.length}
      <div class="tag-list">
        {#each tags as id (id)}
          {@const tag = vault.tags.find((x) => x.id === id)}
          {#if tag}
            <span class="badge">
              <span class="dot" style="background:{tag.color}"></span>{tag.label}
              <button type="button" class="remove" onclick={() => (tags = tags.filter((x) => x !== id))} aria-label={t("remove")}>
                <X size={12} />
              </button>
            </span>
          {/if}
        {/each}
      </div>
    {/if}
  </section>

  <section class="card group">
    <div class="section-head">
      <span class="label">{t("custom_fields")}</span>
      <button type="button" class="sm ghost" onclick={addField} disabled={fields.length >= 20}>
        <Plus size={14} />{t("add_field")}
      </button>
    </div>
    {#each fields as field, i (i)}
      <div class="custom">
        <input bind:value={field.label} placeholder={t("field_name")} maxlength="48" oninput={() => (fieldsChanged = true)} />
        <select bind:value={field.type} onchange={() => { fieldsChanged = true; field.touched = true; }}>
          {#each fieldTypes as type (type.value)}<option value={type.value}>{type.label}</option>{/each}
        </select>
        <input
          type={field.type === "secret" ? "password" : "text"}
          class:mono={field.type === "secret"}
          bind:value={field.value}
          placeholder={field.type === "secret" && !field.touched && field.index !== null ? t("unchanged") : t("value")}
          maxlength="320"
          oninput={() => { fieldsChanged = true; field.touched = true; }}
        />
        <button type="button" class="icon" onclick={() => removeField(i)} aria-label={t("remove")}><X size={16} /></button>
      </div>
    {:else}
      <p class="muted hint">{t("custom_fields_hint")}</p>
    {/each}
  </section>

  <section class="card group">
    <div>
      <label for="notes">{t("notes")}</label>
      <textarea id="notes" bind:value={notes} rows="4" maxlength="4096"></textarea>
    </div>
  </section>

  {#if error}<p class="error">{error}</p>{/if}

  <div class="actions">
    <span class="muted keys"><kbd>Ctrl S</kbd> {t("save")} · <kbd>Esc</kbd> {t("cancel")}</span>
    <button type="button" onclick={onCancel} disabled={busy}>{t("cancel")}</button>
    <button class="primary" type="submit" disabled={busy}>
      {#if busy}<LoaderCircle size={16} class="spin" />{t("saving")}{:else}{t("save")}{/if}
    </button>
  </div>
</form>

<style>
  .editor {
    max-width: 720px;
    display: flex;
    flex-direction: column;
    gap: 16px;
    animation: fade-in 0.15s ease-out;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  h2 {
    font-size: 20px;
    font-weight: 600;
  }
  .fav.active {
    color: hsl(var(--warning));
  }
  .fav.active :global(svg) {
    fill: currentColor;
  }
  .group {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 18px;
  }
  .password {
    position: relative;
  }
  .password input {
    padding-right: 76px;
  }
  .password button {
    position: absolute;
    top: 4px;
  }
  .password button:nth-of-type(1) {
    right: 38px;
  }
  .password button:nth-of-type(2) {
    right: 4px;
  }
  .below {
    min-height: 8px;
    margin: 8px 0;
  }
  .two {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
  }
  .tag-list {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .badge .remove {
    width: 16px;
    height: 16px;
    padding: 0;
    margin-right: -4px;
    border: none;
    border-radius: 50%;
    background: transparent;
    color: hsl(var(--muted-foreground));
  }
  .badge .remove:hover:not(:disabled) {
    background: hsl(var(--border));
    color: hsl(var(--foreground));
  }
  .section-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .section-head .label {
    margin: 0;
  }
  .custom {
    display: grid;
    grid-template-columns: 1fr 120px 1.4fr auto;
    gap: 8px;
    align-items: center;
  }
  .hint {
    margin: -6px 0 0;
    font-size: 13px;
  }
  .actions {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 8px;
    padding-bottom: 8px;
  }
  .keys {
    margin-right: auto;
    font-size: 12px;
    display: flex;
    align-items: center;
    gap: 4px;
  }
</style>
