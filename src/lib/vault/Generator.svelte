<script lang="ts">
  import { onMount } from "svelte";
  import { RefreshCw, Cloud } from "@lucide/svelte";
  import { api, errorText, type GeneratorOptions } from "../api";
  import { t } from "../i18n.svelte";
  import { toast } from "../store.svelte";

  let { onUse }: { onUse: (password: string) => void } = $props();

  const STORAGE_KEY = "generator-options";
  const defaults: GeneratorOptions = {
    length: 20,
    lowercase: true,
    uppercase: true,
    digits: true,
    symbols: true,
    avoidAmbiguous: false,
  };

  function loadOptions(): GeneratorOptions {
    try {
      return { ...defaults, ...JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "{}") };
    } catch {
      return { ...defaults };
    }
  }

  let options = $state(loadOptions());
  let busy = $state(false);

  async function generate() {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(options));
    } catch {
      /* Nur eine Bequemlichkeit */
    }
    onUse(await api.generate(options));
  }

  async function fromServer() {
    busy = true;
    try {
      onUse(await api.generateServer());
    } catch (err) {
      toast(errorText(err), "error");
    } finally {
      busy = false;
    }
  }

  onMount(generate);

  const toggles = $derived([
    { key: "uppercase", label: "A–Z" },
    { key: "lowercase", label: "a–z" },
    { key: "digits", label: "0–9" },
    { key: "symbols", label: "!#%" },
  ] as const);
</script>

<div class="generator card">
  <div class="row">
    <label for="gen-length" class="length-label">{t("length")}</label>
    <input id="gen-length" type="range" min="8" max="64" bind:value={options.length} oninput={generate} />
    <span class="length mono">{options.length}</span>
  </div>
  <div class="row chips">
    {#each toggles as toggle (toggle.key)}
      <label class="chip" class:on={options[toggle.key]}>
        <input type="checkbox" bind:checked={options[toggle.key]} onchange={generate} />
        <span class="mono">{toggle.label}</span>
      </label>
    {/each}
    <label class="chip" class:on={options.avoidAmbiguous}>
      <input type="checkbox" bind:checked={options.avoidAmbiguous} onchange={generate} />
      <span>{t("avoid_ambiguous")}</span>
    </label>
  </div>
  <div class="row buttons">
    <button type="button" class="sm" onclick={generate}><RefreshCw size={14} />{t("regenerate")}</button>
    <button type="button" class="sm ghost" onclick={fromServer} disabled={busy} title={t("server_generator_hint")}>
      <Cloud size={14} />{t("server_generator")}
    </button>
  </div>
</div>

<style>
  .generator {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 14px;
    background: hsl(var(--muted) / 0.4);
    box-shadow: none;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .length-label {
    margin: 0;
    font-size: 13px;
  }
  input[type="range"] {
    flex: 1;
  }
  .length {
    width: 28px;
    text-align: right;
    font-weight: 600;
  }
  .chips {
    flex-wrap: wrap;
    gap: 6px;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    height: 28px;
    margin: 0;
    padding: 0 10px;
    border: 1px solid hsl(var(--input));
    border-radius: 999px;
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    background: hsl(var(--background));
    color: hsl(var(--muted-foreground));
  }
  .chip.on {
    background: hsl(var(--primary));
    border-color: transparent;
    color: hsl(var(--primary-foreground));
  }
  .chip input {
    position: absolute;
    opacity: 0;
    pointer-events: none;
  }
  .chip:focus-within {
    outline: 2px solid hsl(var(--ring));
    outline-offset: 2px;
  }
  .buttons {
    gap: 6px;
  }
</style>
