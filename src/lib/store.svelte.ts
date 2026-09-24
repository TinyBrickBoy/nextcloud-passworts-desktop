import { api, type Settings } from "./api";
import { setLanguage, systemLanguage } from "./i18n.svelte";

export const defaultSettings: Settings = {
  language: "auto",
  theme: "system",
  lockMinutes: 5,
  clipboardSeconds: 30,
  favicons: true,
  sort: "name",
};

export const ui = $state({
  settings: { ...defaultSettings } as Settings,
  toast: null as { text: string; kind: "info" | "error" } | null,
  dialog: null as DialogRequest | null,
});

type DialogRequest = {
  title: string;
  message?: string;
  confirmLabel: string;
  danger?: boolean;
  /** Mit Eingabefeld, dann liefert die Antwort den Text */
  input?: { value: string; placeholder?: string };
  resolve: (value: string | boolean | null) => void;
};

let toastTimer: ReturnType<typeof setTimeout> | undefined;

export function toast(text: string, kind: "info" | "error" = "info") {
  ui.toast = { text, kind };
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => (ui.toast = null), kind === "error" ? 5000 : 2600);
}

export function confirmDialog(options: { title: string; message?: string; confirmLabel: string; danger?: boolean }) {
  return new Promise<boolean>((resolve) => {
    ui.dialog = { ...options, resolve: (v) => resolve(v === true) };
  });
}

export function promptDialog(options: { title: string; confirmLabel: string; value?: string; placeholder?: string }) {
  return new Promise<string | null>((resolve) => {
    ui.dialog = {
      title: options.title,
      confirmLabel: options.confirmLabel,
      input: { value: options.value ?? "", placeholder: options.placeholder },
      resolve: (v) => resolve(typeof v === "string" ? v : null),
    };
  });
}

export function applySettings(settings: Settings) {
  ui.settings = settings;
  setLanguage(settings.language);
  const root = document.documentElement;
  if (settings.theme === "system") root.removeAttribute("data-theme");
  else root.setAttribute("data-theme", settings.theme);
}

export async function saveSettings(patch: Partial<Settings>) {
  const next = { ...ui.settings, ...patch };
  applySettings(next);
  try {
    applySettings(await api.setSettings(next, systemLanguage()));
  } catch {
    /* Einstellungen gelten trotzdem für diese Sitzung */
  }
}
