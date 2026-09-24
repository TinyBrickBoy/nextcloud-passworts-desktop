<script lang="ts">
  import {
    KeyRound,
    Star,
    ShieldAlert,
    Trash2,
    Folder as FolderIcon,
    Plus,
    Pencil,
    X,
    Settings as SettingsIcon,
    Lock,
    RefreshCw,
    LogOut,
  } from "@lucide/svelte";
  import type { Account, VaultView } from "../api";
  import { t } from "../i18n.svelte";

  let {
    vault,
    account,
    filter = $bindable(),
    busy,
    onCreateFolder,
    onRenameFolder,
    onDeleteFolder,
    onCreateTag,
    onDeleteTag,
    onSettings,
    onRefresh,
    onLock,
    onLogout,
  }: {
    vault: VaultView;
    account: Account;
    filter: string;
    busy: boolean;
    onCreateFolder: () => void;
    onRenameFolder: (id: string) => void;
    onDeleteFolder: (id: string) => void;
    onCreateTag: () => void;
    onDeleteTag: (id: string) => void;
    onSettings: () => void;
    onRefresh: () => void;
    onLock: () => void;
    onLogout: () => void;
  } = $props();

  const favorites = $derived(vault.entries.filter((e) => e.favorite).length);
  const issues = $derived(vault.entries.filter((e) => e.status === 1 || e.status === 2).length);
  const folderCount = $derived.by(() => {
    const counts = new Map<string, number>();
    for (const e of vault.entries) counts.set(e.folder, (counts.get(e.folder) ?? 0) + 1);
    return counts;
  });
  const tagCount = $derived.by(() => {
    const counts = new Map<string, number>();
    for (const e of vault.entries) for (const tag of e.tags) counts.set(tag, (counts.get(tag) ?? 0) + 1);
    return counts;
  });
  const host = $derived(account.server.replace(/^https?:\/\//, ""));
</script>

<nav class="sidebar">
  <div class="scroll">
    <div class="group">
      <button class="item" class:active={filter === "all"} onclick={() => (filter = "all")}>
        <KeyRound size={16} /><span class="name">{t("all_items")}</span><span class="count">{vault.entries.length}</span>
      </button>
      <button class="item" class:active={filter === "favorites"} onclick={() => (filter = "favorites")}>
        <Star size={16} /><span class="name">{t("favorites")}</span><span class="count">{favorites || ""}</span>
      </button>
      <button class="item" class:active={filter === "security"} onclick={() => (filter = "security")}>
        <ShieldAlert size={16} /><span class="name">{t("security")}</span>
        {#if issues}<span class="count warn">{issues}</span>{/if}
      </button>
      <button class="item" class:active={filter === "trash"} onclick={() => (filter = "trash")}>
        <Trash2 size={16} /><span class="name">{t("trash")}</span><span class="count">{vault.trash.length || ""}</span>
      </button>
    </div>

    <div class="heading">
      <span>{t("folders")}</span>
      <button class="icon small" onclick={onCreateFolder} aria-label={t("new_folder")} title={t("new_folder")}>
        <Plus size={14} />
      </button>
    </div>
    <div class="group">
      {#each vault.folders as folder (folder.id)}
        <div class="row">
          <button
            class="item"
            class:active={filter === `folder:${folder.id}`}
            onclick={() => (filter = `folder:${folder.id}`)}
          >
            <FolderIcon size={16} /><span class="name">{folder.label}</span>
            <span class="count">{folderCount.get(folder.id) ?? ""}</span>
          </button>
          <span class="actions">
            <button class="icon small" onclick={() => onRenameFolder(folder.id)} aria-label={t("rename")} title={t("rename")}>
              <Pencil size={13} />
            </button>
            <button class="icon small" onclick={() => onDeleteFolder(folder.id)} aria-label={t("delete")} title={t("delete")}>
              <X size={14} />
            </button>
          </span>
        </div>
      {:else}
        <p class="empty">{t("no_folders")}</p>
      {/each}
    </div>

    <div class="heading">
      <span>{t("tags")}</span>
      <button class="icon small" onclick={onCreateTag} aria-label={t("new_tag")} title={t("new_tag")}>
        <Plus size={14} />
      </button>
    </div>
    <div class="group">
      {#each vault.tags as tag (tag.id)}
        <div class="row">
          <button class="item" class:active={filter === `tag:${tag.id}`} onclick={() => (filter = `tag:${tag.id}`)}>
            <span class="tag-dot" style="background:{tag.color || 'hsl(var(--muted-foreground))'}"></span>
            <span class="name">{tag.label}</span>
            <span class="count">{tagCount.get(tag.id) ?? ""}</span>
          </button>
          <span class="actions">
            <button class="icon small" onclick={() => onDeleteTag(tag.id)} aria-label={t("delete")} title={t("delete")}>
              <X size={14} />
            </button>
          </span>
        </div>
      {:else}
        <p class="empty">{t("no_tags")}</p>
      {/each}
    </div>
  </div>

  <div class="footer">
    <div class="account">
      <span class="avatar">{account.user[0]?.toUpperCase()}</span>
      <div class="who">
        <span class="truncate user">{account.user}</span>
        <span class="truncate muted host" title={account.server}>{host}</span>
      </div>
    </div>
    <div class="tools">
      <button class="icon" onclick={onRefresh} disabled={busy} aria-label={t("refresh")} title={t("refresh")}>
        <RefreshCw size={16} class={busy ? "spin" : ""} />
      </button>
      <button class="icon" onclick={onSettings} aria-label={t("settings")} title="{t('settings')} (Ctrl+,)">
        <SettingsIcon size={16} />
      </button>
      {#if vault.lockable}
        <button class="icon" onclick={onLock} aria-label={t("lock")} title="{t('lock')} (Ctrl+L)">
          <Lock size={16} />
        </button>
      {/if}
      <button class="icon" onclick={onLogout} aria-label={t("logout")} title={t("logout")}>
        <LogOut size={16} />
      </button>
    </div>
  </div>
</nav>

<style>
  .sidebar {
    display: flex;
    flex-direction: column;
    min-height: 0;
    background: hsl(var(--sidebar));
    border-right: 1px solid hsl(var(--border));
  }
  .scroll {
    flex: 1;
    overflow-y: auto;
    padding: 14px 10px;
  }
  .group {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin: 20px 4px 4px 10px;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: hsl(var(--muted-foreground));
  }
  .row {
    position: relative;
  }
  .item {
    width: 100%;
    height: 34px;
    justify-content: flex-start;
    gap: 10px;
    padding: 0 10px;
    border: none;
    background: transparent;
    font-weight: 400;
    color: hsl(var(--foreground) / 0.85);
  }
  .item :global(svg) {
    flex: none;
    color: hsl(var(--muted-foreground));
  }
  .item:hover:not(:disabled) {
    background: hsl(var(--accent));
  }
  .item.active {
    background: hsl(var(--accent));
    color: hsl(var(--foreground));
    font-weight: 500;
  }
  .item.active :global(svg) {
    color: hsl(var(--foreground));
  }
  .name {
    flex: 1;
    min-width: 0;
    text-align: left;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .count {
    font-size: 12px;
    color: hsl(var(--muted-foreground));
    font-variant-numeric: tabular-nums;
  }
  .count.warn {
    padding: 0 7px;
    border-radius: 999px;
    color: hsl(var(--warning));
    background: hsl(var(--warning) / 0.12);
    font-weight: 600;
  }
  .actions {
    position: absolute;
    right: 4px;
    top: 50%;
    transform: translateY(-50%);
    display: none;
    background: hsl(var(--accent));
    border-radius: 6px;
  }
  .row:hover .actions,
  .row:focus-within .actions {
    display: flex;
  }
  .row:hover .count {
    visibility: hidden;
  }
  button.small {
    width: 24px;
    height: 24px;
  }
  .tag-dot {
    width: 10px;
    height: 10px;
    margin: 0 3px;
    border-radius: 50%;
    flex: none;
  }
  .empty {
    margin: 2px 10px;
    font-size: 12px;
    color: hsl(var(--muted-foreground));
  }
  .footer {
    border-top: 1px solid hsl(var(--border));
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .account {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }
  .avatar {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border-radius: 50%;
    background: hsl(var(--primary));
    color: hsl(var(--primary-foreground));
    font-weight: 600;
    font-size: 13px;
    flex: none;
  }
  .who {
    display: flex;
    flex-direction: column;
    min-width: 0;
    line-height: 1.3;
  }
  .user {
    font-weight: 500;
    font-size: 13px;
  }
  .host {
    font-size: 12px;
  }
  .tools {
    display: flex;
    justify-content: space-between;
  }
</style>
