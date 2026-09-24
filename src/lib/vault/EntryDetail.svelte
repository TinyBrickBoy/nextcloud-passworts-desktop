<script lang="ts">
  import {
    Copy,
    Check,
    Eye,
    EyeOff,
    ExternalLink,
    Pencil,
    Trash2,
    Star,
    RotateCcw,
    Folder as FolderIcon,
    TriangleAlert,
    ShieldCheck,
    Users,
  } from "@lucide/svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { api, domainOf, errorText, openableUrl, type CopyField, type Entry, type VaultView } from "../api";
  import { formatDate, relativeTime, t } from "../i18n.svelte";
  import { toast, ui } from "../store.svelte";
  import Avatar from "../components/Avatar.svelte";

  let {
    entry,
    vault,
    trashMode,
    busy,
    onEdit,
    onDelete,
    onRestore,
    onFavorite,
  }: {
    entry: Entry;
    vault: VaultView;
    trashMode: boolean;
    busy: boolean;
    onEdit: () => void;
    onDelete: () => void;
    onRestore: () => void;
    onFavorite: () => void;
  } = $props();

  let revealed = $state<string | null>(null);
  let revealedFields = $state<Record<number, string>>({});
  let copied = $state<string | null>(null);
  let copiedTimer: ReturnType<typeof setTimeout> | undefined;

  $effect(() => {
    // Beim Wechsel des Eintrags alles wieder verbergen.
    void entry.id;
    revealed = null;
    revealedFields = {};
    copied = null;
  });

  const folder = $derived(vault.folders.find((f) => f.id === entry.folder));
  const tags = $derived(entry.tags.map((id) => vault.tags.find((tag) => tag.id === id)).filter((tag) => !!tag));
  const domain = $derived(domainOf(entry.url));

  function markCopied(key: string, label: string, secret: boolean) {
    copied = key;
    clearTimeout(copiedTimer);
    copiedTimer = setTimeout(() => (copied = null), 1500);
    const seconds = ui.settings.clipboardSeconds;
    toast(secret && seconds > 0 ? t("copied_clears", { name: label, n: seconds }) : t("copied", { name: label }));
  }

  export async function copy(field: CopyField) {
    try {
      await api.copy(entry.id, field);
      const labels = { password: t("password"), username: t("username"), url: t("website") };
      markCopied(field, labels[field], field === "password");
    } catch (err) {
      toast(errorText(err), "error");
    }
  }

  async function copyCustom(index: number, label: string, secret: boolean) {
    try {
      await api.copyField(entry.id, index);
      markCopied(`f${index}`, label, secret);
    } catch (err) {
      toast(errorText(err), "error");
    }
  }

  async function toggleReveal() {
    revealed = revealed === null ? await api.reveal(entry.id).catch(() => null) : null;
  }

  async function toggleField(index: number) {
    if (index in revealedFields) {
      const { [index]: _, ...rest } = revealedFields;
      revealedFields = rest;
    } else {
      const value = await api.revealField(entry.id, index).catch(() => null);
      if (value !== null) revealedFields = { ...revealedFields, [index]: value };
    }
  }

  function open(url: string) {
    openUrl(openableUrl(url)).catch((err) => toast(errorText(err), "error"));
  }
</script>

{#snippet copyButton(key: string, action: () => void)}
  <button class="icon" onclick={action} aria-label={t("copy")} title={t("copy")}>
    {#if copied === key}<Check size={16} class="ok" />{:else}<Copy size={16} />{/if}
  </button>
{/snippet}

<article class="detail">
  <header>
    <Avatar label={entry.label} url={entry.url} size={52} />
    <div class="heading">
      <h2 class="selectable">{entry.label}</h2>
      <div class="meta-line">
        {#if domain}
          <button class="link" onclick={() => open(entry.url)} title={entry.url}>
            {domain}<ExternalLink size={12} />
          </button>
        {/if}
        {#if folder}
          <span class="badge"><FolderIcon size={12} />{folder.label}</span>
        {/if}
        {#each tags as tag (tag.id)}
          <span class="badge"><span class="dot" style="background:{tag.color}"></span>{tag.label}</span>
        {/each}
        {#if entry.shared}<span class="badge"><Users size={12} />{t("shared")}</span>{/if}
      </div>
    </div>
    <div class="actions">
      {#if trashMode}
        <button onclick={onRestore} disabled={busy}><RotateCcw size={15} />{t("restore")}</button>
        <button class="danger" onclick={onDelete} disabled={busy}><Trash2 size={15} />{t("delete_forever")}</button>
      {:else}
        <button
          class="icon fav"
          class:active={entry.favorite}
          onclick={onFavorite}
          disabled={busy}
          aria-label={entry.favorite ? t("unfavorite") : t("favorite")}
          title={entry.favorite ? t("unfavorite") : t("favorite")}
        >
          <Star size={17} />
        </button>
        {#if entry.editable}
          <button onclick={onEdit} title="{t('edit')} (Ctrl+E)"><Pencil size={15} />{t("edit")}</button>
        {/if}
        <button class="icon" onclick={onDelete} disabled={busy} aria-label={t("move_to_trash")} title={t("move_to_trash")}>
          <Trash2 size={16} />
        </button>
      {/if}
    </div>
  </header>

  {#if entry.status === 2}
    <div class="alert error"><TriangleAlert size={16} /><span>{t("status_breached_long")}</span></div>
  {:else if entry.status === 1}
    <div class="alert warn"><TriangleAlert size={16} /><span>{t("status_weak_long")}</span></div>
  {/if}

  <section class="card fields">
    {#if entry.username}
      <div class="field">
        <span class="name">{t("username")}</span>
        <span class="value selectable truncate">{entry.username}</span>
        <span class="tools">{@render copyButton("username", () => copy("username"))}</span>
      </div>
    {/if}
    <div class="field">
      <span class="name">{t("password")}</span>
      <span class="value mono selectable" class:secret={revealed === null}>{revealed ?? "••••••••••••"}</span>
      <span class="tools">
        <button class="icon" onclick={toggleReveal} aria-label={revealed === null ? t("show") : t("hide")} title={revealed === null ? t("show") : t("hide")}>
          {#if revealed === null}<Eye size={16} />{:else}<EyeOff size={16} />{/if}
        </button>
        {@render copyButton("password", () => copy("password"))}
      </span>
    </div>
    {#if entry.url}
      <div class="field">
        <span class="name">{t("website")}</span>
        <span class="value selectable truncate">{entry.url}</span>
        <span class="tools">
          <button class="icon" onclick={() => open(entry.url)} aria-label={t("open")} title={t("open")}>
            <ExternalLink size={16} />
          </button>
          {@render copyButton("url", () => copy("url"))}
        </span>
      </div>
    {/if}
    {#each entry.fields as f (f.index)}
      {@const secret = f.type === "secret"}
      <div class="field">
        <span class="name truncate">{f.label}</span>
        <span class="value selectable" class:mono={secret} class:secret={secret && !(f.index in revealedFields)}>
          {secret ? (revealedFields[f.index] ?? "••••••••") : f.value}
        </span>
        <span class="tools">
          {#if secret}
            <button class="icon" onclick={() => toggleField(f.index)} aria-label={t("show")}>
              {#if f.index in revealedFields}<EyeOff size={16} />{:else}<Eye size={16} />{/if}
            </button>
          {:else if f.type === "url" && f.value}
            <button class="icon" onclick={() => open(f.value)} aria-label={t("open")}><ExternalLink size={16} /></button>
          {/if}
          {@render copyButton(`f${f.index}`, () => copyCustom(f.index, f.label, secret))}
        </span>
      </div>
    {/each}
  </section>

  {#if entry.notes}
    <section class="card notes">
      <span class="name">{t("notes")}</span>
      <p class="selectable">{entry.notes}</p>
    </section>
  {/if}

  <footer class="muted">
    {#if entry.status === 0 && !trashMode}
      <span class="safe"><ShieldCheck size={14} />{t("status_ok")}</span>
    {/if}
    <span title={formatDate(entry.edited)}>{t("changed", { when: relativeTime(entry.edited) })}</span>
    <span>{t("created", { when: formatDate(entry.created) })}</span>
  </footer>
</article>

<style>
  .detail {
    max-width: 720px;
    display: flex;
    flex-direction: column;
    gap: 18px;
    animation: fade-in 0.15s ease-out;
  }
  header {
    display: flex;
    align-items: flex-start;
    gap: 16px;
  }
  .heading {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding-top: 2px;
  }
  h2 {
    font-size: 22px;
    font-weight: 600;
    line-height: 1.25;
    overflow-wrap: anywhere;
  }
  .meta-line {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
  }
  .link {
    height: 22px;
    padding: 0;
    gap: 4px;
    border: none;
    background: none;
    color: hsl(var(--muted-foreground));
    font-weight: 400;
    margin-right: 4px;
  }
  .link:hover:not(:disabled) {
    background: none;
    color: hsl(var(--foreground));
    text-decoration: underline;
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 6px;
    flex: none;
  }
  .fav.active {
    color: hsl(var(--warning));
  }
  .fav.active :global(svg) {
    fill: currentColor;
  }
  .fields {
    padding: 4px 0;
  }
  .field {
    display: grid;
    grid-template-columns: 140px 1fr auto;
    align-items: center;
    gap: 12px;
    min-height: 52px;
    padding: 6px 12px 6px 18px;
  }
  .field + .field {
    border-top: 1px solid hsl(var(--border));
  }
  .name {
    font-size: 13px;
    color: hsl(var(--muted-foreground));
  }
  .value {
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .value.secret {
    letter-spacing: 0.1em;
    color: hsl(var(--muted-foreground));
  }
  .tools {
    display: flex;
    gap: 2px;
    opacity: 0.55;
    transition: opacity 0.15s;
  }
  .field:hover .tools,
  .tools:focus-within {
    opacity: 1;
  }
  .tools :global(.ok) {
    color: hsl(var(--success));
  }
  .notes {
    padding: 16px 18px;
  }
  .notes p {
    margin: 8px 0 0;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    line-height: 1.6;
  }
  footer {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 18px;
    font-size: 12px;
  }
  .safe {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    color: hsl(var(--success));
  }
</style>
