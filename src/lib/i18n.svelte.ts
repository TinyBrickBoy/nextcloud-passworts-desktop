import de from "./locales/de";
import en from "./locales/en";

export type Messages = typeof en;
export type MessageKey = keyof Messages;

const dictionaries: Record<string, Messages> = { en, de };
export const languages = [
  { code: "en", name: "English" },
  { code: "de", name: "Deutsch" },
];

export function systemLanguage(): string {
  const lang = (navigator.languages?.[0] ?? navigator.language ?? "en").slice(0, 2).toLowerCase();
  return lang in dictionaries ? lang : "en";
}

export const locale = $state({ current: systemLanguage() });

/** Setzt die Sprache aus der Einstellung ("auto" folgt dem System). */
export function setLanguage(setting: string) {
  locale.current = setting === "auto" || !(setting in dictionaries) ? systemLanguage() : setting;
  document.documentElement.lang = locale.current;
}

/** Übersetzt einen Schlüssel, `{name}` Platzhalter werden aus `params` ersetzt. */
export function t(key: MessageKey, params?: Record<string, string | number>): string {
  const text = dictionaries[locale.current]?.[key] ?? en[key] ?? key;
  if (!params) return text;
  return text.replace(/\{(\w+)\}/g, (_, name) => String(params[name] ?? `{${name}}`));
}

export function formatDate(timestamp: number): string {
  if (!timestamp) return "–";
  return new Date(timestamp * 1000).toLocaleDateString(locale.current, { dateStyle: "medium" });
}

/** "vor 3 Tagen" bzw. "3 days ago" */
export function relativeTime(timestamp: number): string {
  if (!timestamp) return "–";
  const seconds = timestamp - Date.now() / 1000;
  const units: [Intl.RelativeTimeFormatUnit, number][] = [
    ["year", 31536000],
    ["month", 2592000],
    ["week", 604800],
    ["day", 86400],
    ["hour", 3600],
    ["minute", 60],
  ];
  const format = new Intl.RelativeTimeFormat(locale.current, { numeric: "auto" });
  for (const [unit, size] of units) {
    if (Math.abs(seconds) >= size) return format.format(Math.round(seconds / size), unit);
  }
  return format.format(0, "minute");
}
