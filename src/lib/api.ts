import { invoke } from "@tauri-apps/api/core";

export interface Account {
  server: string;
  user: string;
}

export interface Status {
  account: Account | null;
  unlocked: boolean;
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
  favorite: boolean;
  editable: boolean;
  status: number;
  edited: number;
  fields: CustomField[];
}

export interface Folder {
  id: string;
  label: string;
  parent: string;
}

export interface VaultView {
  entries: Entry[];
  folders: Folder[];
  broken: number;
  encrypted: boolean;
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
}

export type CopyField = "password" | "username" | "url";

export const api = {
  status: () => invoke<Status>("status"),
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
  trash: (id: string) => invoke<VaultView>("trash", { id }),
  generate: () => invoke<string>("generate"),
};

export function errorText(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : "Unbekannter Fehler";
}
