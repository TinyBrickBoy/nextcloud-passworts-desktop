<script lang="ts" module>
  import { api } from "../api";

  // Pro Sitzung jede Domain nur einmal anfragen, das Backend drosselt zusätzlich.
  const cache = new Map<string, Promise<string | null>>();

  export function clearFaviconCache() {
    cache.clear();
  }

  function load(domain: string) {
    let pending = cache.get(domain);
    if (!pending) {
      pending = api.favicon(domain).catch(() => null);
      cache.set(domain, pending);
    }
    return pending;
  }
</script>

<script lang="ts">
  import { domainOf } from "../api";
  import { ui } from "../store.svelte";

  let { label, url, size = 32 }: { label: string; url: string; size?: number } = $props();

  let src = $state<string | null>(null);
  let element = $state<HTMLElement>();

  const domain = $derived(domainOf(url));
  const letter = $derived((label.trim()[0] ?? "?").toUpperCase());
  // Ruhige, gleichbleibende Farbe je Name, nur als leichte Tönung.
  const hue = $derived([...label].reduce((h, c) => (h * 31 + c.charCodeAt(0)) % 360, 7));

  $effect(() => {
    src = null;
    const target = domain;
    if (!target || !ui.settings.favicons || !element) return;
    let cancelled = false;
    // Erst laden, wenn der Eintrag sichtbar wird, das schont das Rate Limit des Servers.
    const observer = new IntersectionObserver((items) => {
      if (!items.some((i) => i.isIntersecting)) return;
      observer.disconnect();
      load(target).then((data) => {
        if (!cancelled) src = data;
      });
    });
    observer.observe(element);
    return () => {
      cancelled = true;
      observer.disconnect();
    };
  });
</script>

<span
  bind:this={element}
  class="avatar"
  style="--size:{size}px; --hue:{hue}"
  aria-hidden="true"
>
  {#if src}
    <img {src} alt="" draggable="false" />
  {:else}
    {letter}
  {/if}
</span>

<style>
  .avatar {
    flex: none;
    display: inline-grid;
    place-items: center;
    width: var(--size);
    height: var(--size);
    border-radius: calc(var(--size) / 4);
    background: hsl(var(--hue) 30% 50% / 0.12);
    color: hsl(var(--hue) 35% 40%);
    font-weight: 600;
    font-size: calc(var(--size) * 0.42);
    overflow: hidden;
    border: 1px solid hsl(var(--border));
  }
  :global(:root[data-theme="dark"]) .avatar {
    color: hsl(var(--hue) 45% 72%);
  }
  @media (prefers-color-scheme: dark) {
    :global(:root:not([data-theme="light"])) .avatar {
      color: hsl(var(--hue) 45% 72%);
    }
  }
  img {
    width: 70%;
    height: 70%;
    object-fit: contain;
  }
</style>
