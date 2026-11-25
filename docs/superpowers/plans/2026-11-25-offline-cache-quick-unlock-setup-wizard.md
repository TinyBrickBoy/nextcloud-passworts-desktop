# Offline Cache, Quick Unlock & Setup Wizard Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an encrypted offline cache, biometric/PIN quick unlock, and a first-run setup wizard to the Nextcloud Passwords desktop client.

**Architecture:** Three features share a settings expansion. The offline cache stores a secretbox-encrypted blob of raw API data keyed by the app password. Quick unlock stores the encryption password behind a biometric gate (keyring) or PIN-encrypted file. The setup wizard is a stepped Svelte component shown once after the first unlock. All features are opt-in.

**Tech Stack:** Rust (Tauri 2 backend), Svelte 5 + TypeScript (frontend), existing crypto crates (`argon2`, `crypto_secretbox`, `rand`), `keyring` crate for biometric storage.

**Spec:** `docs/superpowers/specs/2026-11-25-offline-cache-setup-wizard-design.md`

## Global Constraints

- No new crate dependencies. All crypto uses the existing `argon2`, `crypto_secretbox`, `blake2`, `rand` crates.
- The `keyring` crate (already a dependency) handles biometric-gated storage.
- All new settings use `#[serde(default)]` so existing `settings.json` files deserialize without error.
- `setupDone` defaults to `true` via a custom serde default so existing users skip the wizard.
- Commit messages follow Conventional Commits in English. No Claude attribution.
- All localization keys are added in both `en.ts` and `de.ts`.
- Tests run with `cd src-tauri && cargo test --release`.
- The `RawPassword`, `RawFolder`, `RawTag` structs gain `Serialize` (they only have `Deserialize` today).

## Review Focus

1. **Corrupt or truncated `cache.bin`** — `read_cache` must not panic on partial reads, zero-length files, or garbage data; it returns `Err` and the caller deletes the file and falls through to online-only mode.
2. **Wrong PIN beyond the 5-attempt limit** — after the 5th failure `pin.bin` and `pin_attempts.json` must both be deleted atomically (delete both even if one is missing); the user must see the "PIN removed" error and fall back to the full password field.
3. **App password changed between cache write and read** — Argon2 produces a different key, `secretbox_open` fails, `read_cache` returns `Err`; the caller silently deletes the stale cache and proceeds online. Must not show a confusing crypto error to the user.
4. **Quick unlock enabled but `pin.bin` / keyring entry deleted externally** — `retrieve_pin` or `retrieve_biometric` returns `Err`; the frontend falls back to the password field with an error message, and the setting is reset to `"off"`.
5. **Offline vault write attempt** — every mutating `Vault` method must check `self.offline` and return the `offline_unavailable` error before touching `self.api`; the frontend must not show write UI elements when `vault.offline` is true.

---

### Task 1: Settings expansion & Serialize on raw types

**Files:**
- Modify: `src-tauri/src/settings.rs`
- Modify: `src-tauri/src/api.rs` (add `Serialize` to `RawPassword`, `RawFolder`, `RawTag`)
- Modify: `src/lib/api.ts`
- Modify: `src/lib/store.svelte.ts`
- Modify: `src/lib/locales/en.ts`
- Modify: `src/lib/locales/de.ts`

**Interfaces:**
- Consumes: nothing new
- Produces: `Settings { offline_cache: bool, quick_unlock: String, setup_done: bool }` (Rust); `Settings { offlineCache: boolean, quickUnlock: "off"|"biometric"|"pin", setupDone: boolean }` (TS). `RawPassword`, `RawFolder`, `RawTag` now implement `Serialize + Deserialize`.

- [ ] **Step 1: Add `Serialize` derive to raw API types in `api.rs`**

Add `Serialize` to the `#[derive(...)]` on `RawPassword`, `RawFolder`, `RawTag`. No other changes.

- [ ] **Step 2: Add new fields to `Settings` in `settings.rs`**

```rust
pub offline_cache: bool,    // default: false
pub quick_unlock: String,   // default: "off"
pub setup_done: bool,       // default: true  (so existing users skip the wizard)
```

All three with `#[serde(default)]`. The `setup_done` default is `true` via a helper `fn default_true() -> bool { true }` with `#[serde(default = "default_true")]`.

Update `Default for Settings` to match.

- [ ] **Step 3: Update frontend `Settings` interface in `api.ts`**

Add `offlineCache: boolean`, `quickUnlock: "off" | "biometric" | "pin"`, `setupDone: boolean`.

- [ ] **Step 4: Update `defaultSettings` in `store.svelte.ts`**

Add `offlineCache: false`, `quickUnlock: "off"`, `setupDone: true`.

- [ ] **Step 5: Add all new localization keys to `en.ts` and `de.ts`**

English keys:
```
offline_cache: "Offline cache"
offline_cache_hint: "Keep an encrypted copy of your vault on this device so it's available offline"
offline_banner: "Offline — read only"
offline_unavailable: "Not available offline"
reconnect: "Reconnect"
reconnected: "Back online"
quick_unlock: "Quick unlock"
quick_unlock_hint: "Skip the encryption password with biometric or PIN"
quick_unlock_off: "Off"
quick_unlock_biometric: "Biometric"
quick_unlock_pin: "PIN"
biometric_unavailable: "No biometric hardware detected"
biometric_unlock: "Unlock with biometric"
biometric_failed: "Biometric authentication failed"
pin_unlock: "Unlock with PIN"
pin_set: "Set PIN"
pin_confirm: "Confirm PIN"
pin_mismatch: "PINs don't match"
pin_removed: "PIN removed after too many failed attempts"
pin_attempts_left: "{n} attempts remaining"
use_password: "Use password instead"
enter_password_to_enable: "Enter your encryption password to enable quick unlock"
wizard_welcome: "Let's set things up"
wizard_step_appearance: "Appearance"
wizard_step_security: "Security"
wizard_step_quick_unlock: "Quick unlock"
wizard_step_data: "Data"
wizard_done: "You're all set"
wizard_start: "Open vault"
wizard_skip: "Skip"
wizard_next: "Next"
wizard_back: "Back"
```

German translations for all keys above.

- [ ] **Step 6: Add i18n backend keys to `i18n.rs`**

Add `"offline_unavailable"` and `"reconnected"` to the backend message table (these are returned from Rust commands).

- [ ] **Step 7: Verify existing tests still pass**

Run: `cd src-tauri && cargo test --release`
Expected: all tests pass, no compile errors.

- [ ] **Step 8: Verify frontend compiles**

Run: `npm run check`
Expected: no type errors.

- [ ] **Step 9: Commit**

```bash
git add src-tauri/src/settings.rs src-tauri/src/api.rs src-tauri/src/i18n.rs \
  src/lib/api.ts src/lib/store.svelte.ts src/lib/locales/en.ts src/lib/locales/de.ts
git commit -m "feat: add settings for offline cache, quick unlock and setup wizard"
```

---

### Task 2: Cache module — encrypt/decrypt/delete

**Files:**
- Create: `src-tauri/src/cache.rs`
- Modify: `src-tauri/src/lib.rs` (add `pub mod cache;`)
- Modify: `src-tauri/src/crypto.rs` (make `pwhash`, `secretbox_seal`, `secretbox_open` `pub(crate)`)

**Interfaces:**
- Consumes: `crypto::pwhash`, `crypto::secretbox_seal`, `crypto::secretbox_open`, `api::RawPassword`, `api::RawFolder`, `api::RawTag` (all with `Serialize + Deserialize` from Task 1)
- Produces:
  - `CachePayload { version: u32, timestamp: i64, needs_password: bool, keychains: HashMap<String, String>, passwords: Vec<RawPassword>, trashed: Vec<RawPassword>, folders: Vec<RawFolder>, tags: Vec<RawTag> }`
  - `write_cache(dir: &Path, app_password: &str, payload: &CachePayload) -> Result<(), String>`
  - `read_cache(dir: &Path, app_password: &str) -> Result<CachePayload, String>`
  - `delete_cache(dir: &Path)`

- [ ] **Step 1: Make crypto helpers pub(crate)**

In `crypto.rs`, change `fn pwhash(...)`, `fn secretbox_open(...)`, `fn secretbox_seal(...)` from `fn` to `pub(crate) fn`.

- [ ] **Step 2: Write the failing test for cache roundtrip**

In `cache.rs` (at the bottom, in a `#[cfg(test)] mod tests`):

```rust
#[test]
fn cache_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let payload = CachePayload {
        version: 1,
        timestamp: 1700000000,
        needs_password: true,
        keychains: HashMap::from([("CSEv1r1".into(), "abc123".into())]),
        passwords: vec![],
        trashed: vec![],
        folders: vec![],
        tags: vec![],
    };
    write_cache(dir.path(), "test-app-password-1234", &payload).unwrap();
    let loaded = read_cache(dir.path(), "test-app-password-1234").unwrap();
    assert_eq!(loaded.version, 1);
    assert_eq!(loaded.timestamp, 1700000000);
    assert!(loaded.needs_password);
    assert_eq!(loaded.keychains["CSEv1r1"], "abc123");
}
```

Add `tempfile = "3"` to `[dev-dependencies]` in `Cargo.toml`.

- [ ] **Step 3: Write the failing test for wrong password**

```rust
#[test]
fn cache_wrong_password_fails() {
    let dir = tempfile::tempdir().unwrap();
    let payload = CachePayload {
        version: 1, timestamp: 0, needs_password: false,
        keychains: HashMap::new(), passwords: vec![],
        trashed: vec![], folders: vec![], tags: vec![],
    };
    write_cache(dir.path(), "correct-password-12345", &payload).unwrap();
    assert!(read_cache(dir.path(), "wrong-password-123456").is_err());
}
```

- [ ] **Step 4: Write the failing test for corrupt file**

```rust
#[test]
fn cache_corrupt_file_fails() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("cache.bin"), b"garbage").unwrap();
    assert!(read_cache(dir.path(), "any-password-1234567").is_err());
}
```

- [ ] **Step 5: Write the failing test for delete**

```rust
#[test]
fn cache_delete_removes_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache.bin");
    std::fs::write(&path, b"data").unwrap();
    delete_cache(dir.path());
    assert!(!path.exists());
}
```

- [ ] **Step 6: Run tests to verify they fail**

Run: `cd src-tauri && cargo test --release cache`
Expected: compile errors (module doesn't exist yet).

- [ ] **Step 7: Implement `cache.rs`**

`CachePayload` struct with `Serialize + Deserialize`. `write_cache`: serialize payload to JSON bytes, generate random 16-byte salt, call `crypto::pwhash(app_password.as_bytes(), &salt)` to get a 32-byte key, call `crypto::secretbox_seal(&json_bytes, &key)` to get `nonce || ciphertext`, write `salt || sealed` to `cache.bin`. `read_cache`: read file, split first 16 bytes as salt, derive key with `pwhash`, call `secretbox_open` on the rest, deserialize JSON. `delete_cache`: `let _ = std::fs::remove_file(dir.join("cache.bin"))`.

Register `pub mod cache;` in `lib.rs`.

- [ ] **Step 8: Run tests to verify they pass**

Run: `cd src-tauri && cargo test --release cache`
Expected: all 4 tests pass.

- [ ] **Step 9: Commit**

```bash
git add src-tauri/src/cache.rs src-tauri/src/crypto.rs src-tauri/src/lib.rs src-tauri/Cargo.toml
git commit -m "feat(cache): add encrypted cache module with write, read, delete"
```

---

### Task 3: Vault offline mode & fetch/decrypt split

**Files:**
- Modify: `src-tauri/src/vault.rs`
- Modify: `src-tauri/src/api.rs` (new `fetch_all` method)

**Interfaces:**
- Consumes: `cache::CachePayload`, `api::RawPassword`, `api::RawFolder`, `api::RawTag`, `crypto::Keychain`
- Produces:
  - `Api::fetch_all(&self) -> Result<RawVaultData, ApiError>` where `RawVaultData { passwords: Vec<RawPassword>, trashed: Vec<RawPassword>, folders: Vec<RawFolder>, tags: Vec<RawTag> }`
  - `Vault.offline: bool`
  - `Vault::from_cache(payload: CachePayload, keychain: Option<Keychain>, lockable: bool) -> Result<Vault, String>`
  - `Vault::refresh(&mut self) -> Result<Option<RawVaultData>, String>` (returns the raw data for caching when online)
  - `VaultView.offline: bool`
  - All write methods return `Err(t("offline_unavailable"))` when `self.offline`.

- [ ] **Step 1: Add `RawVaultData` struct and `Api::fetch_all` in `api.rs`**

```rust
pub struct RawVaultData {
    pub passwords: Vec<RawPassword>,
    pub trashed: Vec<RawPassword>,
    pub folders: Vec<RawFolder>,
    pub tags: Vec<RawTag>,
}
```

`fetch_all` calls `passwords()`, `trashed_passwords()`, `folders()`, `tags()` via `tokio::try_join!` and returns `RawVaultData`.

- [ ] **Step 2: Write test for offline vault blocking writes**

In `vault.rs` tests:

```rust
#[test]
fn offline_vault_blocks_save() {
    let vault = Vault {
        api: Arc::new(Api::new("https://localhost", "u", "p").unwrap()),
        keychain: None,
        lockable: false,
        entries: vec![],
        trash: vec![],
        folders: vec![],
        tags: vec![],
        broken: 0,
        offline: true,
    };
    let view = vault.view();
    assert!(view.offline);
}
```

- [ ] **Step 3: Add `offline: bool` to `Vault`, `VaultView`, and `Vault::new`**

`Vault` gains `pub offline: bool`. `Vault::new` sets it to `false`. `VaultView` gains `pub offline: bool`. `Vault::view` populates it from `self.offline`.

- [ ] **Step 4: Refactor `Vault::refresh` to use `Api::fetch_all` and return raw data**

Change the signature to `pub async fn refresh(&mut self) -> Result<Option<RawVaultData>, String>`. Internally call `self.api.fetch_all()`, clone the raw data before decrypting, decrypt in place, return `Ok(Some(raw))` when online. When `self.offline`, return `Ok(None)`.

The existing `decrypt_password`, `decrypt_folder`, `decrypt_tag` calls stay the same, operating on clones of the raw data.

- [ ] **Step 5: Add `Vault::from_cache`**

`pub fn from_cache(payload: CachePayload, keychain: Option<Keychain>, lockable: bool) -> Result<Vault, String>`: decrypt each `RawPassword`/`RawFolder`/`RawTag` from the payload using the keychain (same as `refresh` does), build a `Vault` with `offline: true` and a dummy `Api` (never used since all writes check `self.offline`).

Wait — the vault needs an `Arc<Api>` to compile. Create a minimal `Api` from the account credentials that exists but whose session is `None`. Write methods will check `self.offline` before calling it.

- [ ] **Step 6: Guard all write methods with offline check**

In each of `save`, `set_favorite`, `delete`, `restore`, `empty_trash`, `create_folder`, `rename_folder`, `delete_folder`, `create_tag`, `delete_tag`: add at the top:
```rust
if self.offline { return Err(t("offline_unavailable").into()); }
```

- [ ] **Step 7: Run all tests**

Run: `cd src-tauri && cargo test --release`
Expected: all pass. The existing `custom_fields_*` tests unchanged, new offline test passes.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/vault.rs src-tauri/src/api.rs
git commit -m "feat(vault): add offline mode flag, fetch/decrypt split, and from_cache constructor"
```

---

### Task 4: Integrate cache into unlock/lock/logout flow

**Files:**
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `cache::write_cache`, `cache::read_cache`, `cache::delete_cache`, `cache::CachePayload`, `Vault::from_cache`, `Vault::refresh` (returns `Option<RawVaultData>`), `Api::fetch_all`, `Settings.offline_cache`
- Produces: The `unlock` command transparently falls back to offline mode on network errors. The `refresh` command writes cache on success. `lock` works unchanged. `logout` deletes the cache.

- [ ] **Step 1: Modify `unlock` to write cache after online success**

After `vault.refresh().await?` succeeds and returns `Some(raw_data)`, if `settings.offline_cache` is true: capture the raw data and encrypted keychains, build a `CachePayload`, spawn a background task with `tauri::async_runtime::spawn_blocking` to call `write_cache`.

The encrypted keychains come from the `opened.keys` HashMap — store them before consuming in keychain decryption.

- [ ] **Step 2: Modify `unlock` to fall back to offline mode on network error**

Wrap the online path (session_request + session_open + vault build + refresh) in a match. If any step returns a network error (`ApiError::Network`): check if `cache.bin` exists via `read_cache`. If it succeeds, build a `Vault::from_cache`. If it fails (no cache or corrupt), return the original network error.

For the offline path with CSE: the `password` parameter is still available (the user entered it). Use it to decrypt the keychain from the cached `keychains` field via `Keychain::decrypt`.

- [ ] **Step 3: Modify `unlock_requirements` to fall back to cached requirements**

When `state.api()?.session_request()` fails with a network error: attempt `read_cache` and return `UnlockRequirements { needs_password: payload.needs_password, tokens: vec![] }`. If no cache exists, return the original error.

- [ ] **Step 4: Modify `refresh` to write cache on success**

After `vault.refresh().await` returns `Ok(Some(raw_data))`, if `settings.offline_cache`: write cache in a background task. If the vault was offline and refresh succeeds, this means reconnection happened — but per the design, reconnect locks and re-unlocks, so this path only runs for online vaults doing a manual refresh.

- [ ] **Step 5: Modify `logout` to delete cache**

Add `cache::delete_cache(&state.config_dir);` in `logout` after removing the account.

- [ ] **Step 6: Modify `lock` — no changes needed**

Lock already drops the vault and clears favicons. The cache file stays on disk (it's encrypted). Verify no changes needed.

- [ ] **Step 7: Run all tests**

Run: `cd src-tauri && cargo test --release`
Expected: all pass.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat(cache): integrate offline cache into unlock, refresh, and logout"
```

---

### Task 5: PIN unlock module

**Files:**
- Create: `src-tauri/src/quick_unlock.rs`
- Modify: `src-tauri/src/lib.rs` (add `pub mod quick_unlock;` and new Tauri commands)

**Interfaces:**
- Consumes: `crypto::pwhash`, `crypto::secretbox_seal`, `crypto::secretbox_open`
- Produces:
  - `store_pin(dir: &Path, password: &str, pin: &str) -> Result<(), String>`
  - `retrieve_pin(dir: &Path, pin: &str) -> Result<Zeroizing<String>, String>`
  - `remove_pin(dir: &Path)`
  - `pin_attempts_remaining(dir: &Path) -> u32`
  - `is_biometric_available() -> bool`
  - `store_biometric(server: &str, user: &str, password: &str) -> Result<(), String>`
  - `retrieve_biometric(server: &str, user: &str) -> Result<Zeroizing<String>, String>`
  - `remove_biometric(server: &str, user: &str)`
  - Tauri commands: `setup_pin`, `setup_biometric`, `biometric_available`, `remove_quick_unlock`

- [ ] **Step 1: Write failing test for PIN roundtrip**

In `quick_unlock.rs` tests:

```rust
#[test]
fn pin_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    store_pin(dir.path(), "my-encryption-password", "123456").unwrap();
    let retrieved = retrieve_pin(dir.path(), "123456").unwrap();
    assert_eq!(retrieved.as_str(), "my-encryption-password");
}
```

- [ ] **Step 2: Write failing test for wrong PIN**

```rust
#[test]
fn pin_wrong_pin_fails() {
    let dir = tempfile::tempdir().unwrap();
    store_pin(dir.path(), "my-encryption-password", "123456").unwrap();
    assert!(retrieve_pin(dir.path(), "654321").is_err());
    assert_eq!(pin_attempts_remaining(dir.path()), 4);
}
```

- [ ] **Step 3: Write failing test for PIN lockout**

```rust
#[test]
fn pin_lockout_after_five_failures() {
    let dir = tempfile::tempdir().unwrap();
    store_pin(dir.path(), "secret", "111111").unwrap();
    for _ in 0..5 {
        let _ = retrieve_pin(dir.path(), "000000");
    }
    assert!(!dir.path().join("pin.bin").exists());
    assert!(!dir.path().join("pin_attempts.json").exists());
}
```

- [ ] **Step 4: Write failing test for PIN reset on success**

```rust
#[test]
fn pin_resets_counter_on_success() {
    let dir = tempfile::tempdir().unwrap();
    store_pin(dir.path(), "secret", "111111").unwrap();
    let _ = retrieve_pin(dir.path(), "000000"); // 1 failure
    let _ = retrieve_pin(dir.path(), "000000"); // 2 failures
    assert_eq!(pin_attempts_remaining(dir.path()), 3);
    retrieve_pin(dir.path(), "111111").unwrap(); // success
    assert_eq!(pin_attempts_remaining(dir.path()), 5);
}
```

- [ ] **Step 5: Run tests to verify they fail**

Run: `cd src-tauri && cargo test --release pin`
Expected: compile errors.

- [ ] **Step 6: Implement PIN functions in `quick_unlock.rs`**

`store_pin`: generate 16-byte salt, derive key with `pwhash(pin.as_bytes(), &salt)`, seal the password with `secretbox_seal`, write `salt || sealed` to `pin.bin`, delete `pin_attempts.json` if it exists.

`retrieve_pin`: read `pin.bin`, split salt, derive key, `secretbox_open`. On failure: read attempt counter from `pin_attempts.json` (default 0), increment, write back. If counter reaches 5: delete both files, return error. On success: delete `pin_attempts.json`, return password.

`remove_pin`: delete `pin.bin` and `pin_attempts.json`.

`pin_attempts_remaining`: read counter from `pin_attempts.json`, return `5 - count`. If file doesn't exist, return 5.

- [ ] **Step 7: Implement biometric functions**

`is_biometric_available() -> bool`: on Linux, check if the Secret Service D-Bus interface is available (try creating a keyring entry and check for errors). On Windows, return `true` (Windows Hello is universally available on Windows 10+). On other platforms, return `false`.

`store_biometric`: use `keyring::Entry::new("nextcloud-passwords-desktop:encryption", &format!("{user}@{server}"))` and `.set_password(password)`.

`retrieve_biometric`: same entry, `.get_password()`. The keyring backend prompts for biometric/password authentication when the keyring is locked.

`remove_biometric`: same entry, `.delete_credential()`.

- [ ] **Step 8: Register module and add Tauri commands in `lib.rs`**

Add `pub mod quick_unlock;`. Add commands:

- `biometric_available() -> bool`: calls `quick_unlock::is_biometric_available()`
- `setup_biometric(password: String) -> CmdResult<()>`: calls `store_biometric` with account server/user
- `setup_pin(password: String, pin: String) -> CmdResult<()>`: validates PIN is 4-8 digits, calls `store_pin`
- `remove_quick_unlock() -> CmdResult<()>`: calls `remove_biometric` and `remove_pin`, resets setting to `"off"`

Register all in `invoke_handler`.

- [ ] **Step 9: Modify `unlock` to accept quick unlock input**

Add an optional parameter `quick_unlock: Option<String>` to the `unlock` command. When present and non-empty:
- If `settings.quick_unlock == "pin"`: call `retrieve_pin(dir, &quick_unlock_value)` to get the encryption password.
- If `settings.quick_unlock == "biometric"`: call `retrieve_biometric(server, user)`.
- Use the retrieved password in place of the `password` parameter for the rest of the unlock flow.

On retrieval failure: return the error (frontend shows fallback).

- [ ] **Step 10: Modify `logout` to remove quick unlock data**

Add `quick_unlock::remove_biometric(...)` and `quick_unlock::remove_pin(...)` in `logout`.

- [ ] **Step 11: Run all tests**

Run: `cd src-tauri && cargo test --release`
Expected: all pass.

- [ ] **Step 12: Commit**

```bash
git add src-tauri/src/quick_unlock.rs src-tauri/src/lib.rs
git commit -m "feat(quick-unlock): add PIN and biometric unlock module with Tauri commands"
```

---

### Task 6: Frontend — offline mode UI

**Files:**
- Modify: `src/lib/api.ts` (add `VaultView.offline` and new API calls)
- Modify: `src/lib/Vault.svelte`
- Modify: `src/lib/vault/Sidebar.svelte`
- Modify: `src/lib/vault/EntryList.svelte`
- Modify: `src/lib/vault/SettingsDialog.svelte`
- Modify: `src/lib/Unlock.svelte`

**Interfaces:**
- Consumes: `VaultView.offline: boolean`, `Settings.offlineCache: boolean`, backend `unlock` (transparent offline), backend `refresh`
- Produces: Offline banner in vault, disabled write controls, offline cache toggle in settings, "Reconnect" button

- [ ] **Step 1: Add `offline` to `VaultView` in `api.ts`**

Add `offline: boolean` to the `VaultView` interface.

- [ ] **Step 2: Add new API calls to `api.ts`**

```typescript
biometricAvailable: () => invoke<boolean>("biometric_available"),
setupBiometric: (password: string) => invoke<void>("setup_biometric", { password }),
setupPin: (password: string, pin: string) => invoke<void>("setup_pin", { password, pin }),
removeQuickUnlock: () => invoke<void>("remove_quick_unlock"),
```

Update `unlock` signature to accept optional `quickUnlock`:
```typescript
unlock: (password: string | null, token: {...} | null, quickUnlock?: string) =>
  invoke<VaultView>("unlock", { password, token, quickUnlock: quickUnlock ?? null }),
```

- [ ] **Step 3: Add offline banner to `Vault.svelte`**

Above the `{#if vault.broken > 0}` block, add:
```svelte
{#if vault.offline}
  <div class="alert info offline-banner">
    <WifiOff size={16} /><span>{t("offline_banner")}</span>
  </div>
{/if}
```

Import `WifiOff` from `@lucide/svelte`. Style the banner with the existing `.alert` pattern.

- [ ] **Step 4: Disable write UI when offline in `Vault.svelte`**

Pass `vault.offline` to `EntryList` and conditionally hide the "New entry" button. In `onKey`, skip `Ctrl+N` and `Delete` when offline. Disable `toggleFavorite`, `deleteSelected`, `restoreSelected`, `emptyTrash` when offline.

- [ ] **Step 5: Change refresh to reconnect when offline in `Vault.svelte`**

Modify `refresh()`: when `vault.offline`, call `onLock()` instead (this triggers lock → re-unlock per the reconnect design). The sidebar refresh button already calls `onRefresh` which is `refresh`.

- [ ] **Step 6: Show "Reconnect" in sidebar when offline**

In `Sidebar.svelte`: change the refresh button's title/label to `t("reconnect")` when `vault.offline`. Pass `offline: boolean` as a new prop.

- [ ] **Step 7: Add offline cache toggle to `SettingsDialog.svelte`**

Under the Security section, add a setting row with checkbox:
```svelte
<div class="setting">
  <div class="text">
    <span>{t("offline_cache")}</span>
    <small class="muted">{t("offline_cache_hint")}</small>
  </div>
  <input type="checkbox" checked={ui.settings.offlineCache}
    onchange={(e) => saveSettings({ offlineCache: e.currentTarget.checked })} />
</div>
```

- [ ] **Step 8: Verify frontend compiles**

Run: `npm run check`
Expected: no type errors.

- [ ] **Step 9: Commit**

```bash
git add src/lib/api.ts src/lib/Vault.svelte src/lib/vault/Sidebar.svelte \
  src/lib/vault/EntryList.svelte src/lib/vault/SettingsDialog.svelte
git commit -m "feat(ui): add offline mode banner, disable writes, and cache toggle in settings"
```

---

### Task 7: Frontend — quick unlock UI

**Files:**
- Modify: `src/lib/Unlock.svelte`
- Modify: `src/lib/vault/SettingsDialog.svelte`

**Interfaces:**
- Consumes: `Settings.quickUnlock`, `api.biometricAvailable()`, `api.setupBiometric()`, `api.setupPin()`, `api.removeQuickUnlock()`, `api.unlock(..., quickUnlock)`
- Produces: PIN/biometric unlock UI on the unlock screen, quick unlock settings section in settings dialog

- [ ] **Step 1: Add quick unlock to `Unlock.svelte`**

When `req?.needsPassword` is true, check `ui.settings.quickUnlock`:

- If `"pin"`: show a PIN input (4–8 digit, numeric, masked with show/hide toggle) in place of the password field. Below it, a "Use password instead" link that swaps back to the password field. On submit, call `api.unlock(null, token, pin)`.

- If `"biometric"`: show an additional "Unlock with biometric" button (with `Fingerprint` icon from lucide) below the password field. On click, call `api.unlock(null, token, "biometric")`. The password field stays visible as fallback.

On error from quick unlock: show the error, switch to the password field.

- [ ] **Step 2: Add quick unlock section to `SettingsDialog.svelte`**

New section "Quick unlock" between Security and Account, only visible when `vault.encrypted` is true (passed as a new `encrypted: boolean` prop):

Radio group with three options: Off / Biometric / PIN.
- Off: calls `api.removeQuickUnlock()`.
- Biometric: disabled with `{t("biometric_unavailable")}` hint when `biometricAvailable` state is false (fetched `onMount`). When selected: show a small inline password input + confirm button. On confirm: call `api.setupBiometric(password)`, save setting.
- PIN: show two masked numeric inputs (PIN + confirm PIN). Validate they match and are 4-8 digits. On confirm: call `api.setupPin(password, pin)`, save setting. Needs the encryption password too — show it in the same inline form.

- [ ] **Step 3: Pass `encrypted` prop through to `SettingsDialog`**

In `Vault.svelte`, update the `SettingsDialog` instantiation:
```svelte
<SettingsDialog {account} lockable={vault.lockable} encrypted={vault.encrypted} onClose={...} />
```

- [ ] **Step 4: Verify frontend compiles**

Run: `npm run check`
Expected: no type errors.

- [ ] **Step 5: Commit**

```bash
git add src/lib/Unlock.svelte src/lib/vault/SettingsDialog.svelte src/lib/Vault.svelte
git commit -m "feat(ui): add PIN and biometric quick unlock to unlock screen and settings"
```

---

### Task 8: Setup wizard

**Files:**
- Create: `src/lib/SetupWizard.svelte`
- Modify: `src/App.svelte`

**Interfaces:**
- Consumes: `Settings`, `VaultView.encrypted`, `api.biometricAvailable()`, `api.setupBiometric()`, `api.setupPin()`, `saveSettings()`, `applySettings()`
- Produces: `SetupWizard` component, `"wizard"` screen state in `App.svelte`

- [ ] **Step 1: Create `SetupWizard.svelte`**

Props: `account: Account`, `vault: VaultView`, `onDone: () => void`.

State: `step` (0–4), local copies of settings being configured.

Uses `AuthShell` for layout consistency. Each step is a card inside:

**Step 0 — Appearance:** language dropdown + theme segmented control (reuse the exact markup from `SettingsDialog`). Apply live with `applySettings` / `saveSettings`.

**Step 1 — Security:** auto-lock dropdown + clipboard dropdown (same markup as `SettingsDialog`).

**Step 2 — Quick Unlock:** only shown when `vault.encrypted`. Radio: Off / Biometric (if available) / PIN. Setting up biometric or PIN requires the encryption password — show an inline password input + the PIN fields (for PIN). Call `api.setupBiometric` or `api.setupPin`. If the user picks "Off", skip.

**Step 3 — Data:** offline cache toggle + favicons toggle.

**Step 4 — Done:** "You're all set" message + "Open vault" button.

Navigation: Back/Next buttons at the bottom. Skip button at the top right that calls `saveSettings({ setupDone: true })` and `onDone()`. Step 2 is skipped (not shown) when `!vault.encrypted`.

The "Open vault" button on the last step calls `saveSettings({ setupDone: true })` then `onDone()`.

- [ ] **Step 2: Add `"wizard"` screen to `App.svelte`**

Add `screen` state value `"wizard"`. After `onUnlocked`:
```typescript
function onUnlocked(v: VaultView) {
    vault = v;
    if (!ui.settings.setupDone) {
        screen = "wizard";
    } else {
        screen = "vault";
    }
}
```

Add the wizard to the template:
```svelte
{:else if screen === "wizard" && vault && account}
  <SetupWizard {account} {vault} onDone={() => (screen = "vault")} />
```

Import `SetupWizard`.

- [ ] **Step 3: Verify frontend compiles**

Run: `npm run check`
Expected: no type errors.

- [ ] **Step 4: Commit**

```bash
git add src/lib/SetupWizard.svelte src/App.svelte
git commit -m "feat(wizard): add first-run setup wizard with appearance, security, quick unlock, and data steps"
```

---

### Task 9: Integration test & final polish

**Files:**
- Modify: various (bug fixes found during manual testing)
- Test: manual verification

**Interfaces:**
- Consumes: everything from Tasks 1–8
- Produces: a working, tested build

- [ ] **Step 1: Run the full Rust test suite**

Run: `cd src-tauri && cargo test --release`
Expected: all tests pass.

- [ ] **Step 2: Run the frontend type check**

Run: `npm run check`
Expected: no errors.

- [ ] **Step 3: Build the app**

Run: `npm run tauri build`
Expected: builds without errors.

- [ ] **Step 4: Verify settings backward compatibility**

Create a `settings.json` with the old format (no new fields). Start the app. It should load without errors, `setupDone` should default to `true`, the user should go straight to unlock without seeing the wizard.

- [ ] **Step 5: Verify new user sees wizard**

Delete `settings.json` and `account.json` from the config dir. Set up a new account. After first unlock, the wizard should appear. Complete or skip it. The vault should load.

- [ ] **Step 6: Commit any fixes**

```bash
git add -A
git commit -m "fix: address integration issues found during testing"
```

Only commit if there are actual changes. Skip if everything worked.
