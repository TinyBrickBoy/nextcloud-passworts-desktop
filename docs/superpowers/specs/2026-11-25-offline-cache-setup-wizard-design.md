# Offline Cache, Quick Unlock & Setup Wizard

## Intent

Users want their passwords available without an internet connection, a faster way
to unlock than typing the full encryption password every time, and a comfortable
first-use experience. The offline cache stores an encrypted copy of the vault
locally. Quick unlock (biometric or PIN) lets users skip the full encryption
password on subsequent unlocks. The setup wizard runs once after the first login
and lets users configure all preferences before they ever see the vault.

## Why

- A desktop password manager that only works online is barely better than the web
  interface. Offline access is the main value proposition of a local app.
- Typing a long encryption password on every unlock is friction that drives users
  away. Other password managers (Bitwarden, 1Password) offer biometric and PIN
  unlock as standard convenience features.
- New users currently have to discover settings on their own after landing in the
  vault. A wizard makes features like the offline cache discoverable from the start.

## Goals

- Encrypted offline cache that works with and without CSE (client-side encryption).
- Read-only offline mode (viewing, copying, searching). Write operations require
  the server.
- Biometric unlock via platform APIs (Windows Hello, polkit/PAM on Linux) as a
  gate to retrieve the stored encryption password from the keyring.
- PIN unlock as a cross-platform alternative: a short numeric PIN protects the
  stored encryption password with Argon2 + brute-force lockout.
- One-time setup wizard after first login covering all important settings.
- No regression in the existing security model.

## Non-goals

- Offline editing, creating or deleting passwords (too complex for v1; sync
  conflicts, revision handling).
- Automatic background sync or periodic refresh while offline.
- Storing the raw encryption password in the keyring without any gate (biometric
  or PIN). The password is always protected by an additional factor.

## Anti-goals

- Storing plaintext passwords on disk under any circumstance.
- Storing the decrypted keychain on disk.
- Storing the encryption password in the keyring without a biometric or PIN gate.
- Making any of these features mandatory — they all stay opt-in.

## Constraints

- The app uses Tauri 2 (Rust backend, Svelte 5 frontend).
- Cryptographic primitives must stay within what the project already uses: Argon2id,
  XSalsa20Poly1305, BLAKE2b (crates: `argon2`, `crypto_secretbox`, `blake2`).
- The config directory (`app_config_dir()`) is the only writable location available
  on all platforms.
- The app password lives in the system keyring and is always available after login.

## Approaches considered

### 1. Raw JSON file (rejected)

Store the raw API responses as a JSON file. Simple, but metadata (timestamps, folder
IDs, entry count) would be in plaintext. Without CSE, actual passwords would be in
plaintext on disk. Unacceptable.

### 2. SQLite database (rejected)

Structured storage with encrypted columns. Adds a dependency (`rusqlite` or
`sqlx`), migration complexity and build tooling for what is fundamentally a
single-blob cache that gets fully replaced on every sync. Overkill.

### 3. Encrypted cache blob — chosen

Serialize all raw API data plus the encrypted keychain strings into one JSON
structure, then encrypt the entire blob using `Argon2(app_password, salt) →
secretbox_seal(json)`. The app password from the system keyring serves as the
encryption key source, so the cache works for all users (with and without CSE).
On offline unlock, the blob is decrypted with the app password, then CSE
decryption runs as usual if needed.

This was chosen because it protects everything (passwords and metadata), uses only
existing primitives, adds no dependencies, and works regardless of whether CSE is
enabled.

## Design

### Cache format

File: `cache.bin` in the config directory, next to `account.json` and
`settings.json`.

Binary layout: `salt (16 bytes) || nonce (24 bytes) || secretbox(json)`.

The JSON payload before encryption:

```json
{
  "version": 1,
  "timestamp": 1732540800,
  "keychains": { "CSEv1r1": "hex-encrypted-keychain-string" },
  "passwords": [ ... raw RawPassword objects ... ],
  "trashed": [ ... raw RawPassword objects ... ],
  "folders": [ ... raw RawFolder objects ... ],
  "tags": [ ... raw RawTag objects ... ]
}
```

The `keychains` field stores the encrypted keychain strings as received from
`session/open`. Without CSE this object is empty. The `version` field allows
future format changes.

### Key derivation

```
key = Argon2id(password = app_password, salt = random_16_bytes,
               ops = 2, mem = 64 MiB, out = 32 bytes)
```

Same parameters as the existing `pwhash` function in `crypto.rs`. On write, a
fresh random salt is generated. The app password comes from the system keyring
(already available via `Account::app_password()`).

### Cache lifecycle

**Writing the cache (online unlock):**

1. `unlock()` succeeds → vault is refreshed with live data.
2. If `settings.offlineCache` is true: serialize the raw API responses and
   encrypted keychains into the JSON payload above.
3. Derive a key from the app password, encrypt the JSON, write `cache.bin`.
4. This happens in a background task (spawned after `unlock` returns the view)
   so it doesn't slow down the unlock.

**Reading the cache (offline unlock):**

1. `unlock()` tries the normal online path (session request + open).
2. On network error (not auth error), if `cache.bin` exists:
   a. Read `cache.bin`, derive key from app password, decrypt.
   b. Deserialize the JSON payload.
   c. If CSE is enabled: prompt for encryption password, decrypt the keychain
      from the cached `keychains` field, then decrypt entries.
   d. If CSE is not enabled: entries are already in plaintext inside the
      decrypted JSON (they were plaintext from the API).
   e. Build the `Vault` with a marker that it's offline (no `Api` for writes).
3. The vault view includes an `offline: bool` flag so the frontend can show an
   indicator and disable write operations.

**Cache invalidation:**

- On logout: delete `cache.bin`.
- On successful online refresh: overwrite `cache.bin` with fresh data.
- If the app password changes (re-login): old cache is unreadable (different
  key), silently deleted and rebuilt on next unlock.

### Offline vault behavior

- All read operations work: list, search, view details, copy password/username,
  reveal password, reveal custom fields.
- Write operations (save, delete, restore, create/rename/delete folder/tag,
  set favorite, empty trash) return an error message ("Not available offline"
  / "Offline nicht verfügbar").
- Server password generator is unavailable; local generator works.
- Favicons are unavailable (no server connection, and they're not cached by
  design — they'd reveal which websites the user has accounts on).
- Refresh button tries to reconnect; on success, switches to online mode.
- The keepalive loop is not started in offline mode.
- Locking drops the decrypted vault as usual but stays on the unlock screen
  (does not require re-downloading from server since the cache exists).

### Settings changes (cache)

`Settings` gains one field:

```rust
pub offline_cache: bool  // default: false
```

Frontend `Settings` interface gains `offlineCache: boolean`.

---

## Quick Unlock (Biometric & PIN)

Quick unlock stores the encryption password behind a gate (biometric prompt or
PIN) so users don't have to type the full password on every unlock. It only
applies to CSE-enabled vaults (without CSE there is no encryption password to
skip). Quick unlock is independent of the offline cache — it works for both
online and offline unlocks.

### Approaches considered

#### A. Biometric-gated keyring entry (chosen for biometric)

Store the encryption password in a separate keyring entry. Access requires a
platform biometric prompt (Windows Hello, polkit/PAM fingerprint on Linux).
The `keyring` crate already supports credential access; the biometric gate is
platform-specific:

- **Windows:** `keyring` with Windows Credential Manager supports Windows Hello
  natively when the credential is stored with the `CRED_FLAGS_REQUIRE_CONFIRMATION`
  flag, or via the `windows` crate's `UserConsentVerifier` API to prompt before
  releasing the credential.
- **Linux:** Use `polkit` (pkexec) or a D-Bus call to `org.freedesktop.PolicyKit1`
  to request user authentication before releasing the keyring entry. Alternatively,
  the Secret Service API already prompts when the keyring is locked.

This is how Bitwarden and 1Password handle it. The encryption password is in
the keyring (protected by OS-level access control + biometric gate), never on
disk unprotected.

#### B. PIN-encrypted local storage (chosen for PIN)

The encryption password is encrypted with a key derived from a short PIN:

```
salt = random 16 bytes (stored alongside)
key  = Argon2id(PIN, salt, ops=2, mem=64MiB, out=32)
blob = secretbox_seal(encryption_password, key)
```

Stored as `pin.bin` in the config directory: `salt || nonce || ciphertext`.

Brute-force protection: a counter file `pin_attempts.json` tracks failed
attempts. After 5 consecutive failures, the PIN data is wiped and the user must
enter the full encryption password and set up PIN again. The counter resets on
successful unlock.

#### C. Store password directly in keyring without gate (rejected)

This makes CSE encryption pointless — any process with keyring access gets all
passwords. Explicitly an anti-goal.

### Quick unlock flow

**Setup (after first successful unlock with full password):**

1. The app detects that biometric hardware is available (or the user opts for PIN).
2. User enables biometric or PIN unlock in settings (or in the setup wizard).
3. For biometric: the encryption password is stored in a separate keyring entry
   (`nextcloud-passwords-desktop:encryption:<user>@<server>`).
4. For PIN: the user chooses a PIN (4–8 digits). The encryption password is
   encrypted with the PIN-derived key and written to `pin.bin`.
5. The setting is saved: `quickUnlock: "off" | "biometric" | "pin"`.

**Unlocking with biometric:**

1. `unlock_requirements` returns `needsPassword: true`.
2. Frontend checks `settings.quickUnlock === "biometric"` and shows a
   "Unlock with biometric" button alongside the password field.
3. User clicks the button (or it triggers automatically).
4. Backend calls the platform biometric API. On success, retrieves the
   encryption password from the keyring entry.
5. Unlock proceeds as normal with the retrieved password.
6. If biometric fails or is cancelled, the user falls back to typing the full
   password.

**Unlocking with PIN:**

1. `unlock_requirements` returns `needsPassword: true`.
2. Frontend checks `settings.quickUnlock === "pin"` and shows a PIN input field
   instead of (but with a fallback link to) the full password field.
3. User enters PIN.
4. Backend reads `pin.bin`, derives key from PIN, decrypts the encryption
   password. If decryption fails (wrong PIN), increment attempt counter.
5. After 5 failures: delete `pin.bin` and `pin_attempts.json`, show error
   "PIN removed after too many attempts", fall back to full password.
6. On success: reset attempt counter, unlock proceeds with the decrypted
   password.

### Quick unlock and offline cache interaction

Quick unlock and offline cache are orthogonal:

- **Online + quick unlock:** biometric/PIN retrieves the encryption password,
  then normal online session opens.
- **Offline + quick unlock:** biometric/PIN retrieves the encryption password,
  then the cached vault is decrypted with it.
- **Online + no quick unlock:** type password as today.
- **Offline + no quick unlock:** type password, decrypt from cache.

### Rust changes (quick unlock)

**New file: `src-tauri/src/quick_unlock.rs`**

- `is_biometric_available() → bool` — checks platform support.
- `store_biometric(server, user, password) → Result<()>` — stores encryption
  password in a biometric-gated keyring entry.
- `retrieve_biometric(server, user) → Result<String>` — prompts biometric,
  returns password.
- `remove_biometric(server, user)` — removes the keyring entry.
- `store_pin(dir, password, pin) → Result<()>` — encrypts password with PIN,
  writes `pin.bin`.
- `retrieve_pin(dir, pin) → Result<String>` — reads `pin.bin`, decrypts,
  manages attempt counter.
- `remove_pin(dir)` — deletes `pin.bin` and `pin_attempts.json`.
- `pin_attempts_remaining(dir) → u32` — returns how many attempts are left.

**`lib.rs`**

- New command `biometric_available() → bool`.
- New command `setup_biometric(password: String) → Result<()>` — called after
  a successful unlock when the user enables biometric.
- New command `setup_pin(password: String, pin: String) → Result<()>`.
- The `unlock` command gains an optional `quickUnlock: Option<QuickUnlockInput>`
  parameter where `QuickUnlockInput` is `Biometric` or `Pin(String)`. When
  present, the backend retrieves the encryption password via the quick unlock
  method instead of using the `password` parameter.
- `logout()`: call `remove_biometric` and `remove_pin`.

### Settings changes (quick unlock)

```rust
pub quick_unlock: String  // "off", "biometric", or "pin" — default: "off"
```

Frontend `Settings` interface gains `quickUnlock: "off" | "biometric" | "pin"`.

### Frontend changes (quick unlock)

**`Unlock.svelte`**

- When `needsPassword && settings.quickUnlock === "biometric"`: show a
  fingerprint/face icon button ("Unlock with biometric"). Clicking it calls
  the biometric unlock command. The password field is still visible as fallback.
- When `needsPassword && settings.quickUnlock === "pin"`: show a PIN input
  (4–8 digit numeric field) instead of the password field, with a "Use password
  instead" link below.
- If biometric/PIN fails, switch to the password field with an error message.

**`SettingsDialog.svelte`**

- New section "Quick unlock" (only shown when `encrypted === true` i.e. CSE is
  active):
  - Radio group: Off / Biometric / PIN
  - Biometric option disabled with explanation when hardware not available.
  - Selecting PIN opens a small inline form to set the PIN (enter + confirm).
  - Selecting Biometric triggers a biometric prompt to verify before storing.
  - Changing quick unlock method requires the encryption password (the user is
    already unlocked, so it can be prompted in a small dialog or we can use
    the fact that the keychain is decrypted to verify).

**Decision (my call):** Setting up quick unlock requires the encryption password.
Since the user just unlocked, we prompt them once ("Enter your encryption
password to enable quick unlock") in a small dialog. The password is stored
behind the gate immediately and then zeroized.

### Localization (quick unlock)

Additional keys for `en.ts` and `de.ts`:

```
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
```

---

### Rust changes

**New file: `src-tauri/src/cache.rs`**

- `write_cache(dir, app_password, payload) → Result<()>` — serialize, encrypt,
  write.
- `read_cache(dir, app_password) → Result<CachePayload>` — read, decrypt,
  deserialize.
- `delete_cache(dir)` — remove `cache.bin` if it exists.
- `CachePayload` struct matching the JSON format above.

**`crypto.rs`** — no new functions needed. The existing `pwhash` and
`secretbox_seal`/`secretbox_open` are already suitable and can be reused by the
cache module directly.

**`vault.rs`**

- Add `Vault::from_cache(payload, keychain, lockable) → Vault` constructor that
  builds a vault from cached data instead of fetching from the API.
- The `Vault` struct gains `pub offline: bool` (default `false`).
- Write methods (`save`, `delete`, `restore`, `set_favorite`, `empty_trash`,
  `create_folder`, `rename_folder`, `delete_folder`, `create_tag`, `delete_tag`)
  check `self.offline` and return an error early if true.

**`lib.rs`**

- `unlock()`: after successful online unlock + refresh, spawn a background task
  to write the cache if the setting is enabled.
- `unlock()`: on network error, attempt `read_cache` + build offline vault.
  If CSE is required, still prompt for the encryption password (the UI already
  does this based on `UnlockRequirements`).
- New command `unlock_offline(password: Option<String>)` for the case where
  the frontend knows it's offline and wants to skip the network attempt.
  Alternatively, the existing `unlock` command handles both paths transparently.
- `lock()`: clearing the vault works the same regardless of online/offline.
- `logout()`: call `delete_cache`.
- `VaultView` gains `pub offline: bool`.
- `refresh()`: if offline, try to reconnect. On success, do a full online
  refresh, update the cache, and clear the offline flag.

**Decision (my call):** The existing `unlock` command handles both online and
offline transparently. No separate `unlock_offline` command. The frontend
doesn't need to know whether it'll be online or offline before unlocking — it
just calls `unlock` and gets back a `VaultView` with an `offline` flag. This
keeps the frontend simple.

**Decision (my call):** `unlock_requirements` currently hits the server. When
offline, it should return cached requirements (whether CSE was enabled last
time). The cache payload stores a `needs_password: bool` field for this.
This avoids a network call that would fail offline.

Updated cache payload:

```json
{
  "version": 1,
  "timestamp": 1732540800,
  "needsPassword": true,
  "keychains": { ... },
  "passwords": [ ... ],
  "trashed": [ ... ],
  "folders": [ ... ],
  "tags": [ ... ]
}
```

### Frontend changes

**`api.ts`**

- `Settings` interface: add `offlineCache: boolean`.
- `VaultView` interface: add `offline: boolean`.

**`App.svelte`**

- After the first successful `onUnlocked`, check `settings.setupDone`. If false,
  show the `SetupWizard` screen instead of going straight to the vault. The
  wizard receives the `VaultView` (to know whether CSE is active) and the
  `Account`.
- New screen state: `"wizard"`.
- When the wizard completes, it transitions to the vault screen with the
  existing `VaultView`.

**New file: `src/lib/SetupWizard.svelte`**

A stepped wizard using the same `AuthShell` layout as Setup and Unlock for
visual consistency. Steps:

1. **Language & Theme** — language dropdown (Auto/English/Deutsch), theme
   segmented control (System/Light/Dark). Both apply live as the user picks.
2. **Security** — auto-lock timer dropdown, clipboard clear time dropdown.
3. **Quick Unlock** — only shown for CSE vaults (`encrypted === true`). Radio:
   Off / Biometric (if available) / PIN. Selecting biometric or PIN triggers
   the setup flow (password prompt + biometric verification or PIN entry).
4. **Offline & Icons** — offline cache toggle with explanation text, favicons
   toggle.
5. **Done** — summary, "Start" button.

Note: the wizard runs after the first successful unlock (not after login),
because it needs to know whether CSE is active to show the quick unlock step.
This means the flow is: login → first unlock (full password) → wizard → vault.

Each step has Back/Next buttons. "Skip" button at the top right sets defaults
and goes straight to unlock.

Settings object: `setupDone: bool` (default `false`), set to `true` when the
wizard completes or is skipped.

**`SettingsDialog.svelte`**

- Add the offline cache toggle under the Security section, with a hint text
  explaining what it does ("Keep an encrypted copy of your vault on this device
  so it's available offline").

**`Vault.svelte`**

- When `vault.offline` is true, show a small banner or badge in the toolbar
  ("Offline — read only" / "Offline — nur Lesen").
- Disable the "New entry" button, hide edit/delete actions, disable
  "Server password" in the generator.
- The refresh button shows "Reconnect" and attempts to go online.

**`Unlock.svelte`**

- When `unlock` returns a network error and no cache exists, show the error as
  today. When a cache exists, the backend handles it transparently — the user
  just sees the normal unlock flow.
- If `unlock_requirements` fails (offline), the backend falls back to cached
  requirements. The frontend doesn't need changes for this.

### Localization

New keys for both `en.ts` and `de.ts`:

```
offline_cache: "Offline cache"
offline_cache_hint: "Keep an encrypted copy of your vault on this device so it's available offline"
offline_banner: "Offline — read only"
offline_unavailable: "Not available offline"
reconnect: "Reconnect"
reconnected: "Back online"
wizard_welcome: "Let's set things up"
wizard_step_appearance: "Appearance"
wizard_step_security: "Security"
wizard_step_data: "Data"
wizard_done: "You're all set"
wizard_start: "Open vault"
wizard_skip: "Skip"
wizard_next: "Next"
wizard_back: "Back"
wizard_step_quick_unlock: "Quick unlock"
```

Quick unlock localization keys are listed in the Quick Unlock section above.

### Reconnect behavior

When the user presses "Reconnect" while offline, the app locks the vault and
returns to the normal unlock screen. The user goes through the standard online
unlock flow (entering encryption password and/or 2FA code as needed). This was
chosen over holding the password in memory or showing an inline dialog because:

- It's consistent with the security posture (passwords are zeroized promptly).
- It reuses the existing unlock UI with no new components.
- Going offline and reconnecting is infrequent enough that re-entering
  credentials is acceptable.

### Additional builder decisions

**Decision (my call):** `RawPassword`, `RawFolder`, `RawTag` need `Serialize`
added (they currently only derive `Deserialize`).

**Decision (my call):** The raw API data must be captured before `refresh()`
decrypts it in place. The fetch and decrypt steps in `Vault::refresh()` need to
be split so the raw data can be returned separately for caching.

**Decision (my call):** Existing users who already have a `settings.json` get
`setupDone` defaulted to `true` (via serde default) so they don't see the
wizard on upgrade. Only truly new users (no `settings.json` yet) see it.

**Decision (my call):** The 15-second connect timeout before the offline
fallback kicks in is acceptable. No separate `unlock_offline` command.

## What's left to the builder

- Exact error handling: which reqwest errors count as "network error" (timeout,
  connection refused, DNS failure) vs. server errors that should not trigger
  offline mode.
- Whether to show a toast when the cache is written/updated.
- Animation/transition details for the wizard steps.
- The exact wording of the offline banner and how it integrates with the
  existing toolbar layout in `Vault.svelte`.
- Whether `refresh` in offline mode should auto-detect connectivity or require
  a manual button press. (Proposal: manual "Reconnect" button, no polling.)
- The exact platform detection logic for biometric availability on Linux (which
  D-Bus interfaces to probe, what to do when polkit is not installed).
- Whether the PIN input in the wizard should be masked (like a password) or
  visible (like a phone PIN). (Proposal: masked with a show/hide toggle,
  consistent with the existing password field style.)
- Whether to allow changing quick unlock method without re-entering the
  encryption password when the user is already unlocked and the keychain is
  in memory. (Proposal: always require the password for security clarity.)
