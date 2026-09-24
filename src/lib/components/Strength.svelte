<script lang="ts" module>
  /** Grobe Schätzung der Entropie in Bit aus Zeichenvorrat und Länge. */
  export function entropy(password: string): number {
    if (!password) return 0;
    let pool = 0;
    if (/[a-z]/.test(password)) pool += 26;
    if (/[A-Z]/.test(password)) pool += 26;
    if (/[0-9]/.test(password)) pool += 10;
    if (/[^a-zA-Z0-9]/.test(password)) pool += 32;
    const unique = new Set(password).size;
    // Wiederholungen senken den Wert, damit "aaaaaaaaaaaa" nicht als stark gilt.
    const effectiveLength = Math.min(password.length, unique * 2);
    return Math.round(effectiveLength * Math.log2(Math.max(pool, 2)));
  }

  export function level(bits: number): 0 | 1 | 2 | 3 {
    if (bits < 40) return 0;
    if (bits < 60) return 1;
    if (bits < 80) return 2;
    return 3;
  }
</script>

<script lang="ts">
  import { t } from "../i18n.svelte";

  let { password }: { password: string } = $props();

  const bits = $derived(entropy(password));
  const lvl = $derived(level(bits));
  const labels = $derived([t("strength_weak"), t("strength_fair"), t("strength_good"), t("strength_strong")]);
</script>

{#if password}
  <div class="strength" data-level={lvl}>
    <div class="bars">
      {#each [0, 1, 2, 3] as i (i)}<span class:on={i <= lvl}></span>{/each}
    </div>
    <span class="text">{labels[lvl]} · {t("bits", { n: bits })}</span>
  </div>
{/if}

<style>
  .strength {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 12px;
    color: hsl(var(--muted-foreground));
  }
  .bars {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 4px;
    width: 120px;
  }
  .bars span {
    height: 4px;
    border-radius: 2px;
    background: hsl(var(--muted));
  }
  [data-level="0"] .on {
    background: hsl(var(--destructive));
  }
  [data-level="1"] .on {
    background: hsl(var(--warning));
  }
  [data-level="2"] .on,
  [data-level="3"] .on {
    background: hsl(var(--success));
  }
</style>
