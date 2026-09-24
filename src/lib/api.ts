import { invoke } from "@tauri-apps/api/core";

export interface Account {
  server: string;
  user: string;
}

export interface Settings {
  language: string;
  theme: "system" | "light" | "dark";
  lockMinutes: number;
  clipboardSeconds: number;
  favicons: boolean;
  sort: "name" | "edited";
}

export interface Status {
  account: Account | null;
  unlocked: boolean;
  settings: Settings;
}

export interface Token {
  id: string;
  type: string;
  label: string;
  description: string;
}

export interface UnlockRequirements {
  needsPassword: boolean;
  tokens: Token[];
}

export interface CustomField {
  index: number;
  label: string;
  type: string;
  value: string;
}

export interface Entry {
  id: string;
  label: string;
  username: string;
  url: string;
  notes: string;
  folder: string;
  tags: string[];
  favorite: boolean;
  editable: boolean;
  shared: boolean;
  status: number;
  edited: number;
  created: number;
  hasPassword: boolean;
  fields: CustomField[];
}

export interface Folder {
  id: string;
  label: string;
  parent: string;
}

export interface Tag {
  id: string;
  label: string;
  color: string;
}

export interface VaultView {
  entries: Entry[];
  trash: Entry[];
  folders: Folder[];
  tags: Tag[];
  broken: number;
  encrypted: boolean;
  lockable: boolean;
}

export interface FieldInput {
  label: string;
  type: string;
  /** null = unverändertes geheimes Feld, dann gilt index */
  value: string | null;
  index: number | null;
}

export interface SaveInput {
  id: string | null;
  label: string;
  username: string;
  password: string | null;
  url: string;
  notes: string;
  folder: string | null;
  favorite: boolean;
  tags: string[];
  fields: FieldInput[] | null;
}

export interface GeneratorOptions {
  length: number;
  lowercase: boolean;
  uppercase: boolean;
  digits: boolean;
  symbols: boolean;
  avoidAmbiguous: boolean;
}

export type CopyField = "password" | "username" | "url";

export const api = {
  status: () => invoke<Status>("status"),
  setSettings: (settings: Settings, systemLang: string) =>
    invoke<Settings>("set_settings", { settings, systemLang }),
  setSystemLanguage: (lang: string) => invoke<void>("set_system_language", { lang }),
  loginBrowser: (server: string) => invoke<Account>("login_browser", { server }),
  loginCancel: () => invoke<void>("login_cancel"),
  loginManual: (server: string, user: string, appPassword: string) =>
    invoke<Account>("login_manual", { server, user, appPassword }),
  logout: () => invoke<void>("logout"),
  unlockRequirements: () => invoke<UnlockRequirements>("unlock_requirements"),
  unlock: (password: string | null, token: { id: string; code: string } | null) =>
    invoke<VaultView>("unlock", { password, token }),
  lock: () => invoke<void>("lock"),
  refresh: () => invoke<VaultView>("refresh"),
  reveal: (id: string) => invoke<string>("reveal", { id }),
  revealField: (id: string, index: number) => invoke<string>("reveal_field", { id, index }),
  copy: (id: string, field: CopyField) => invoke<void>("copy", { id, field }),
  copyField: (id: string, index: number) => invoke<void>("copy_field", { id, index }),
  copyText: (text: string) => invoke<void>("copy_text", { text }),
  save: (input: SaveInput) => invoke<{ id: string; vault: VaultView }>("save", { input }),
  setFavorite: (id: string, favorite: boolean) => invoke<VaultView>("set_favorite", { id, favorite }),
  delete: (id: string) => invoke<VaultView>("delete", { id }),
  restore: (id: string) => invoke<VaultView>("restore", { id }),
  emptyTrash: () => invoke<VaultView>("empty_trash"),
  createFolder: (label: string, parent: string | null) =>
    invoke<{ id: string; vault: VaultView }>("create_folder", { label, parent }),
  renameFolder: (id: string, label: string) => invoke<VaultView>("rename_folder", { id, label }),
  deleteFolder: (id: string) => invoke<VaultView>("delete_folder", { id }),
  createTag: (label: string, color: string) => invoke<{ id: Tag; vault: VaultView }>("create_tag", { label, color }),
  deleteTag: (id: string) => invoke<VaultView>("delete_tag", { id }),
  generate: (options: GeneratorOptions) => invoke<string>("generate", { options }),
  generateServer: () => invoke<string>("generate_server"),
  favicon: (domain: string) => invoke<string | null>("favicon", { domain }),
};

export function errorText(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : String(e);
}

/** Domain einer gespeicherten Adresse, auch ohne Schema. */
export function domainOf(url: string): string {
  if (!url) return "";
  try {
    return new URL(/^[a-z][a-z0-9+.-]*:\/\//i.test(url) ? url : `https://${url}`).hostname.replace(/^www\./, "");
  } catch {
    return "";
  }
}

export function openableUrl(url: string): string {
  return /^[a-z][a-z0-9+.-]*:/i.test(url) ? url : `https://${url}`;
}
