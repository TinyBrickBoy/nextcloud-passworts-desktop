<script lang="ts">
  import { Search, Plus, Star, ArrowDownAZ, Clock, Trash2, TriangleAlert, Users } from "@lucide/svelte";
  import { domainOf, type Entry } from "../api";
  import { t } from "../i18n.svelte";
  import { saveSettings, ui } from "../store.svelte";
  import Avatar from "../components/Avatar.svelte";

  let {
    entries,
    title,
    selectedId = $bindable(),
    query = $bindable(),
    trashMode,
    searchInput = $bindable(),
    onNew,
    onEmptyTrash,
  }: {
    entries: Entry[];
    title: string;
    selectedId: string | null;
    query: string;
    trashMode: boolean;
    searchInput?: HTMLInputElement;
    onNew: () => void;
    onEmptyTrash: () => void;
  } = $props();

  function subtitle(e: Entry) {
    return e.username || domainOf(e.url) || " ";
  }
</script>

<section class="list">
  <div class="toolbar">
    <div class="search">
      <Search size={16} />
      <input
        bind:this={searchInput}
        bind:value={query}
        placeholder={t("search")}
        spellcheck="false"
        aria-label={t("search")}
      />
      {#if !query}<kbd>Ctrl F</kbd>{/if}
    </div>
    <button class="primary new" onclick={onNew} title="{t('new_entry')} (Ctrl+N)" aria-label={t("new_entry")}>
      <Plus size={16} />
    </button>
  </div>

  <div class="head">
    <span class="title truncate">{title}</span>
    <span class="count">{entries.length}</span>
    <span class="spacer"></span>
    {#if trashMode}
      {#if entries.length}
        <button class="ghost sm danger" onclick={onEmptyTrash}><Trash2 size={14} />{t("empty_trash")}</button>
      {/if}
    {:else}
      <button
        class="icon sort"
        onclick={() => saveSettings({ sort: ui.settings.sort === "name" ? "edited" : "name" })}
        title={ui.settings.sort === "name" ? t("sort_name") : t("sort_edited")}
        aria-label={t("sort")}
      >
        {#if ui.settings.sort === "name"}<ArrowDownAZ size={16} />{:else}<Clock size={16} />{/if}
      </button>
    {/if}
  </div>

  <div class="entries" role="listbox" aria-label={title}>
    {#each entries as e (e.id)}
      <button
        id="entry-{e.id}"
        class="entry"
        class:active={e.id === selectedId}
        role="option"
        aria-selected={e.id === selectedId}
        onclick={() => (selectedId = e.id)}
      >
        <Avatar label={e.label} url={e.url} size={34} />
        <span class="text">
          <span class="label truncate">{e.label}</span>
          <span class="sub truncate">{subtitle(e)}</span>
        </span>
        <span class="marks">
          {#if e.status === 2}
            <span class="mark bad" title={t("status_breached")}><TriangleAlert size={14} /></span>
          {:else if e.status === 1}
            <span class="mark warn" title={t("status_weak")}><TriangleAlert size={14} /></span>
          {/if}
          {#if e.shared}<span class="mark" title={t("shared")}><Users size={14} /></span>{/if}
          {#if e.favorite}<span class="mark star"><Star size={14} /></span>{/if}
        </span>
      </button>
    {:else}
      <div class="empty">
        {#if query}
          <Search size={28} strokeWidth={1.5} />
          <p>{t("no_results")}</p>
        {:else if trashMode}
          <Trash2 size={28} strokeWidth={1.5} />
          <p>{t("trash_empty")}</p>
        {:else}
          <Star size={28} strokeWidth={1.5} />
          <p>{t("no_entries")}</p>
          <button class="sm" onclick={onNew}><Plus size={14} />{t("new_entry")}</button>
        {/if}
      </div>
    {/each}
  </div>
</section>

<style>
  .list {
    display: flex;
    flex-direction: column;
    min-height: 0;
    border-right: 1px solid hsl(var(--border));
  }
  .toolbar {
    display: flex;
    gap: 8px;
    padding: 14px 14px 10px;
  }
  .search {
    position: relative;
    flex: 1;
  }
  .search :global(svg) {
    position: absolute;
    left: 11px;
    top: 50%;
    transform: translateY(-50%);
    color: hsl(var(--muted-foreground));
    pointer-events: none;
  }
  .search input {
    height: 36px;
    padding-left: 34px;
    padding-right: 64px;
    background: hsl(var(--muted) / 0.5);
    border-color: transparent;
  }
  .search input:focus-visible {
    background: hsl(var(--background));
    border-color: hsl(var(--input));
  }
  .search kbd {
    position: absolute;
    right: 8px;
    top: 50%;
    transform: translateY(-50%);
    pointer-events: none;
  }
  .new {
    width: 36px;
    padding: 0;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 36px;
    padding: 0 12px 0 18px;
    border-bottom: 1px solid hsl(var(--border));
  }
  .title {
    font-weight: 600;
    font-size: 13px;
  }
  .count {
    font-size: 12px;
    color: hsl(var(--muted-foreground));
  }
  .spacer {
    flex: 1;
  }
  .sort {
    width: 28px;
    height: 28px;
  }
  .entries {
    flex: 1;
    overflow-y: auto;
    padding: 6px 8px 12px;
  }
  .entry {
    width: 100%;
    height: auto;
    justify-content: flex-start;
    gap: 12px;
    padding: 8px 10px;
    border: none;
    background: transparent;
    text-align: left;
    font-weight: 400;
  }
  .entry:hover:not(:disabled) {
    background: hsl(var(--accent) / 0.6);
  }
  .entry.active {
    background: hsl(var(--accent));
  }
  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    line-height: 1.35;
  }
  .label {
    font-weight: 500;
  }
  .sub {
    font-size: 12px;
    color: hsl(var(--muted-foreground));
  }
  .marks {
    display: flex;
    gap: 4px;
    flex: none;
  }
  .mark {
    display: inline-flex;
    color: hsl(var(--muted-foreground));
  }
  .mark.star {
    color: hsl(var(--warning));
  }
  .mark.star :global(svg) {
    fill: currentColor;
  }
  .mark.bad {
    color: hsl(var(--destructive));
  }
  .mark.warn {
    color: hsl(var(--warning));
  }
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    margin-top: 56px;
    padding: 0 20px;
    text-align: center;
    color: hsl(var(--muted-foreground));
  }
  .empty p {
    margin: 0;
  }
</style>
