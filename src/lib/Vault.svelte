<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { KeyRound, TriangleAlert } from "@lucide/svelte";
  import { api, errorText, type Account, type Entry, type VaultView } from "./api";
  import { t } from "./i18n.svelte";
  import { confirmDialog, promptDialog, toast, ui } from "./store.svelte";
  import Sidebar from "./vault/Sidebar.svelte";
  import EntryList from "./vault/EntryList.svelte";
  import EntryDetail from "./vault/EntryDetail.svelte";
  import Editor from "./vault/Editor.svelte";
  import SettingsDialog from "./vault/SettingsDialog.svelte";

  let {
    vault = $bindable(),
    account,
    onLock,
    onLogout,
  }: { vault: VaultView; account: Account; onLock: () => void; onLogout: () => void } = $props();

  // Ruhige Farben für neue Tags (Tailwind 500er Töne).
  const TAG_COLORS = ["#3b82f6", "#10b981", "#f59e0b", "#ef4444", "#8b5cf6", "#ec4899", "#14b8a6", "#64748b"];

  let filter = $state("all");
  let query = $state("");
  let selectedId = $state<string | null>(null);
  let mode = $state<"view" | "edit" | "new">("view");
  let busy = $state(false);
  let showSettings = $state(false);
  let searchInput = $state<HTMLInputElement>();
  let detail = $state<EntryDetail>();

  const trashMode = $derived(filter === "trash");

  const visible = $derived.by(() => {
    const q = query.trim().toLowerCase();
    let list: Entry[] = trashMode ? vault.trash : vault.entries;
    if (filter === "favorites") list = list.filter((e) => e.favorite);
    else if (filter === "security") list = list.filter((e) => e.status === 1 || e.status === 2);
    else if (filter.startsWith("folder:")) list = list.filter((e) => e.folder === filter.slice(7));
    else if (filter.startsWith("tag:")) list = list.filter((e) => e.tags.includes(filter.slice(4)));
    if (q) {
      list = list.filter((e) =>
        [e.label, e.username, e.url, e.notes, ...e.fields.filter((f) => f.type !== "secret").map((f) => f.value)].some(
          (v) => v.toLowerCase().includes(q),
        ),
      );
    }
    if (ui.settings.sort === "edited" && !trashMode) list = [...list].sort((a, b) => b.edited - a.edited);
    return list;
  });

  const title = $derived.by(() => {
    if (filter === "favorites") return t("favorites");
    if (filter === "security") return t("security");
    if (filter === "trash") return t("trash");
    if (filter.startsWith("folder:")) return vault.folders.find((f) => f.id === filter.slice(7))?.label ?? t("folder");
    if (filter.startsWith("tag:")) return vault.tags.find((tag) => tag.id === filter.slice(4))?.label ?? t("tags");
    return t("all_items");
  });

  const selected = $derived(
    (trashMode ? vault.trash : vault.entries).find((e) => e.id === selectedId) ?? null,
  );

  $effect(() => {
    // Nur beim Wechsel des Filters: Auswahl verwerfen, wenn der Eintrag dort nicht vorkommt,
    // und einen offenen Editor schließen. Alles andere bewusst nicht als Abhängigkeit.
    void filter;
    untrack(() => {
      if (selectedId && !visible.some((e) => e.id === selectedId)) selectedId = null;
      if (mode === "edit") mode = "view";
    });
  });

  async function run<T>(action: () => Promise<T>, success?: string): Promise<T | undefined> {
    busy = true;
    try {
      const result = await action();
      if (success) toast(success);
      return result;
    } catch (err) {
      toast(errorText(err), "error");
      return undefined;
    } finally {
      busy = false;
    }
  }

  async function refresh() {
    const v = await run(() => api.refresh(), t("refreshed"));
    if (v) vault = v;
  }

  async function deleteSelected() {
    if (!selected) return;
    const forever = trashMode;
    const ok = await confirmDialog({
      title: forever ? t("delete_forever_title") : t("move_to_trash_title"),
      message: forever
        ? t("delete_forever_message", { name: selected.label })
        : t("move_to_trash_message", { name: selected.label }),
      confirmLabel: forever ? t("delete_forever") : t("move_to_trash"),
      danger: true,
    });
    if (!ok) return;
    const v = await run(() => api.delete(selected.id), forever ? t("deleted") : t("moved_to_trash"));
    if (v) {
      vault = v;
      selectedId = null;
    }
  }

  async function restoreSelected() {
    if (!selected) return;
    const v = await run(() => api.restore(selected.id), t("restored"));
    if (v) {
      vault = v;
      selectedId = null;
    }
  }

  async function emptyTrash() {
    const ok = await confirmDialog({
      title: t("empty_trash_title"),
      message: t("empty_trash_message", { n: vault.trash.length }),
      confirmLabel: t("empty_trash"),
      danger: true,
    });
    if (!ok) return;
    const v = await run(() => api.emptyTrash(), t("trash_emptied"));
    if (v) vault = v;
  }

  async function toggleFavorite() {
    if (!selected) return;
    const v = await run(() => api.setFavorite(selected.id, !selected.favorite));
    if (v) vault = v;
  }

  async function createFolder() {
    const label = await promptDialog({ title: t("new_folder"), confirmLabel: t("create"), placeholder: t("folder_name") });
    if (!label) return;
    const result = await run(() => api.createFolder(label, null), t("folder_created"));
    if (result) {
      vault = result.vault;
      filter = `folder:${result.id}`;
    }
  }

  async function renameFolder(id: string) {
    const folder = vault.folders.find((f) => f.id === id);
    const label = await promptDialog({ title: t("rename_folder"), confirmLabel: t("save"), value: folder?.label });
    if (!label || label === folder?.label) return;
    const v = await run(() => api.renameFolder(id, label));
    if (v) vault = v;
  }

  async function deleteFolder(id: string) {
    const folder = vault.folders.find((f) => f.id === id);
    const ok = await confirmDialog({
      title: t("delete_folder_title"),
      message: t("delete_folder_message", { name: folder?.label ?? "" }),
      confirmLabel: t("delete"),
      danger: true,
    });
    if (!ok) return;
    const v = await run(() => api.deleteFolder(id), t("folder_deleted"));
    if (v) {
      vault = v;
      if (filter === `folder:${id}`) filter = "all";
    }
  }

  async function createTag(): Promise<string | null> {
    const label = await promptDialog({ title: t("new_tag"), confirmLabel: t("create"), placeholder: t("tag_name") });
    if (!label) return null;
    const color = TAG_COLORS[vault.tags.length % TAG_COLORS.length];
    const result = await run(() => api.createTag(label, color), t("tag_created"));
    if (!result) return null;
    vault = result.vault;
    return result.id.id;
  }

  async function deleteTag(id: string) {
    const tag = vault.tags.find((x) => x.id === id);
    const ok = await confirmDialog({
      title: t("delete_tag_title"),
      message: t("delete_tag_message", { name: tag?.label ?? "" }),
      confirmLabel: t("delete"),
      danger: true,
    });
    if (!ok) return;
    const v = await run(() => api.deleteTag(id), t("tag_deleted"));
    if (v) {
      vault = v;
      if (filter === `tag:${id}`) filter = "all";
    }
  }

  function newEntry() {
    if (trashMode || filter === "security") filter = "all";
    mode = "new";
  }

  function onSaved(id: string, v: VaultView) {
    vault = v;
    mode = "view";
    selectedId = id;
    toast(t("saved"));
  }

  function move(delta: number) {
    if (!visible.length) return;
    const i = visible.findIndex((e) => e.id === selectedId);
    const next = i < 0 ? 0 : Math.min(visible.length - 1, Math.max(0, i + delta));
    selectedId = visible[next].id;
    mode = "view";
    document.getElementById(`entry-${selectedId}`)?.scrollIntoView({ block: "nearest" });
  }

  function onKey(e: KeyboardEvent) {
    if (mode !== "view" || showSettings || ui.dialog) return;
    const mod = e.ctrlKey || e.metaKey;
    const target = e.target as HTMLElement;
    const inField = target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement;
    const key = e.key.toLowerCase();

    if (mod && key === "f") {
      e.preventDefault();
      searchInput?.focus();
      searchInput?.select();
    } else if (mod && key === "n") {
      e.preventDefault();
      newEntry();
    } else if (mod && key === "l" && vault.lockable) {
      e.preventDefault();
      onLock();
    } else if (mod && key === ",") {
      e.preventDefault();
      showSettings = true;
    } else if (mod && key === "e" && selected?.editable && !trashMode) {
      e.preventDefault();
      mode = "edit";
    } else if (mod && key === "c" && selected && (!inField || target === searchInput) && !window.getSelection()?.toString()) {
      e.preventDefault();
      detail?.copy("password");
    } else if (mod && key === "b" && selected) {
      e.preventDefault();
      detail?.copy("username");
    } else if (key === "delete" && selected && !inField) {
      e.preventDefault();
      deleteSelected();
    } else if (e.key === "ArrowDown" && (!inField || target === searchInput)) {
      e.preventDefault();
      move(1);
    } else if (e.key === "ArrowUp" && (!inField || target === searchInput)) {
      e.preventDefault();
      move(-1);
    } else if (e.key === "Enter" && target === searchInput && selected && !trashMode) {
      detail?.copy("password");
    } else if (e.key === "Escape" && target === searchInput) {
      query = "";
    }
  }

  onMount(() => searchInput?.focus());
</script>

<svelte:window onkeydown={onKey} />

<div class="layout">
  <Sidebar
    {vault}
    {account}
    bind:filter
    {busy}
    onCreateFolder={createFolder}
    onRenameFolder={renameFolder}
    onDeleteFolder={deleteFolder}
    onCreateTag={() => void createTag()}
    onDeleteTag={deleteTag}
    onSettings={() => (showSettings = true)}
    onRefresh={refresh}
    {onLock}
    onLogout={async () => {
      const ok = await confirmDialog({
        title: t("logout_title"),
        message: t("logout_message"),
        confirmLabel: t("logout"),
        danger: true,
      });
      if (ok) onLogout();
    }}
  />

  <EntryList
    entries={visible}
    {title}
    bind:selectedId
    bind:query
    bind:searchInput
    {trashMode}
    onNew={newEntry}
    onEmptyTrash={emptyTrash}
  />

  <main class="content">
    {#if vault.broken > 0}
      <div class="alert warn broken">
        <TriangleAlert size={16} /><span>{t("broken_entries", { n: vault.broken })}</span>
      </div>
    {/if}

    {#if mode === "new" || (mode === "edit" && selected)}
      {#key mode === "edit" ? selected?.id : "new"}
        <Editor
          entry={mode === "edit" ? selected : null}
          {vault}
          defaultFolder={filter.startsWith("folder:") ? filter.slice(7) : null}
          defaultTag={filter.startsWith("tag:") ? filter.slice(4) : null}
          {onSaved}
          onCancel={() => (mode = "view")}
          onCreateTag={createTag}
        />
      {/key}
    {:else if selected}
      <EntryDetail
        bind:this={detail}
        entry={selected}
        {vault}
        {trashMode}
        {busy}
        onEdit={() => (mode = "edit")}
        onDelete={deleteSelected}
        onRestore={restoreSelected}
        onFavorite={toggleFavorite}
      />
    {:else}
      <div class="placeholder">
        <span class="mark"><KeyRound size={26} strokeWidth={1.6} /></span>
        <p>{t("select_entry")}</p>
        <div class="shortcuts">
          <span><kbd>Ctrl F</kbd>{t("search")}</span>
          <span><kbd>Ctrl N</kbd>{t("new_entry")}</span>
          <span><kbd>↑ ↓</kbd>{t("navigate")}</span>
          <span><kbd>Ctrl C</kbd>{t("copy_password")}</span>
          <span><kbd>Ctrl B</kbd>{t("copy_username")}</span>
          <span><kbd>Ctrl E</kbd>{t("edit")}</span>
          {#if vault.lockable}<span><kbd>Ctrl L</kbd>{t("lock")}</span>{/if}
          <span><kbd>Ctrl ,</kbd>{t("settings")}</span>
        </div>
      </div>
    {/if}
  </main>
</div>

{#if showSettings}
  <SettingsDialog {account} lockable={vault.lockable} onClose={() => (showSettings = false)} />
{/if}

<style>
  .layout {
    height: 100%;
    display: grid;
    grid-template-columns: 232px minmax(260px, 340px) minmax(0, 1fr);
  }
  .content {
    min-width: 0;
    overflow-y: auto;
    padding: 32px 40px;
  }
  .broken {
    max-width: 720px;
    margin-bottom: 20px;
  }
  .placeholder {
    height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 14px;
    color: hsl(var(--muted-foreground));
    text-align: center;
  }
  .placeholder p {
    margin: 0;
    font-size: 15px;
  }
  .mark {
    display: grid;
    place-items: center;
    width: 56px;
    height: 56px;
    border-radius: 16px;
    background: hsl(var(--muted));
    color: hsl(var(--foreground));
  }
  .shortcuts {
    display: grid;
    grid-template-columns: repeat(2, auto);
    gap: 8px 28px;
    margin-top: 12px;
    font-size: 12px;
    text-align: left;
  }
  .shortcuts span {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .shortcuts kbd {
    min-width: 52px;
    justify-content: center;
  }
  @media (max-width: 900px) {
    .layout {
      grid-template-columns: 200px minmax(220px, 280px) minmax(0, 1fr);
    }
    .content {
      padding: 24px;
    }
  }
</style>
