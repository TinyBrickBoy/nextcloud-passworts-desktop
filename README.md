# Passwords Desktop

Minimalistischer Desktop Client für die [Nextcloud Passwords](https://apps.nextcloud.com/apps/passwords) App.
Läuft unter Linux und Windows.

## Funktionen

- Anmeldung per Nextcloud Login Flow (im Browser bestätigen) oder mit App Passwort
- Ende zu Ende Verschlüsselung (CSEv1) inklusive Verschlüsselungspasswort
- Zweiter Faktor per Code beim Entsperren
- Suchen, Ordner, Favoriten
- Einträge anlegen, bearbeiten, in den Papierkorb verschieben
- Passwort Generator des Servers
- Kopieren mit automatischem Leeren der Zwischenablage nach 30 Sekunden
- Automatische Sperre nach 5 Minuten ohne Aktivität
- Helles und dunkles Design nach Systemeinstellung

### Tastenkürzel

| Kürzel | Aktion |
|---|---|
| Strg+F | Suchen |
| Strg+N | Neuer Eintrag |
| Strg+C | Passwort des gewählten Eintrags kopieren |
| Strg+B | Benutzername kopieren |
| Strg+L | Sperren |
| ↑ ↓ | Eintrag wählen |
| Enter (in der Suche) | Passwort kopieren |

## Techstack

| Teil | Wahl | Grund |
|---|---|---|
| App Hülle | [Tauri 2](https://tauri.app) | Kleine Binaries (wenige MB), nutzt das System Webview, native Installer für Linux und Windows |
| Backend | Rust | Netzwerk, Kryptografie und Zwischenablage laufen außerhalb des Webviews |
| Kryptografie | `argon2`, `blake2`, `crypto_secretbox` | Reine Rust Nachbildung der libsodium Funktionen, gegen echte libsodium Vektoren getestet |
| Zugangsdaten | `keyring` | App Passwort liegt im Schlüsselbund des Systems (Secret Service / Windows Anmeldeinformationen) |
| Oberfläche | Svelte 5 + TypeScript + Vite | Wenig Code, keine schwere UI Bibliothek |
| Design | shadcn artige HSL Tokens, Inter | Schlicht, ohne Farbverläufe |

## Sicherheit

- Das App Passwort wird nie in einer Datei gespeichert, nur im Schlüsselbund des Systems.
- Das Verschlüsselungspasswort bleibt nur so lange im Speicher, wie das Entsperren dauert.
- Passwörter gehen nur an die Oberfläche, wenn du auf „Anzeigen“ klickst. Beim Kopieren gehen sie direkt vom Backend in die Zwischenablage.
- Beim Sperren wird die Session auf dem Server geschlossen und der entschlüsselte Tresor verworfen.
- Strikte Content Security Policy, das Webview darf keine externen Verbindungen aufbauen.

## Entwickeln

Voraussetzungen: Node.js 22, Rust (stable) und die [Tauri Systemabhängigkeiten](https://tauri.app/start/prerequisites/).
Unter Debian/Ubuntu:

```sh
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libxdo-dev
```

```sh
npm install
npm run tauri dev      # App mit Hot Reload starten
npm run tauri build    # Installer bauen (.deb, .rpm, .AppImage bzw. .msi, .exe)
cd src-tauri && cargo test --release   # Krypto Tests
```

Der Workflow `Build` testet jeden Pull Request und baut die Installer als Artefakte.

## Release erstellen

1. Auf GitHub unter **Releases → Draft a new release** einen neuen Tag anlegen, z. B. `v0.2.0`
   (`v1.2.3`, kurze Tags wie `v1.0` oder `2` werden zu `1.0.0` bzw. `2.0.0` ergänzt).
2. **Publish release** klicken.
3. Der Workflow `Release` übernimmt die Version aus dem Tag, baut für Linux und Windows und hängt
   `.deb`, `.rpm`, `.AppImage`, `.msi` und `.exe` an den Release. Das dauert etwa 10 bis 15 Minuten.

## Aufbau

```
src/                  Oberfläche (Svelte)
  App.svelte          Ablauf: Einrichten → Entsperren → Tresor
  lib/Setup.svelte    Server verbinden
  lib/Unlock.svelte   Verschlüsselungspasswort und zweiter Faktor
  lib/Vault.svelte    Seitenleiste, Liste, Details
  lib/Editor.svelte   Anlegen und Bearbeiten
  lib/api.ts          Aufrufe ans Rust Backend
src-tauri/src/
  api.rs              HTTP Client für die Passwords API
  account.rs          Login Flow v2, Konto und Schlüsselbund
  crypto.rs           PWDv1 Challenge, CSEv1 Keychain und Feldverschlüsselung
  vault.rs            Entschlüsselter Tresor, Speichern und Löschen
  clipboard.rs        Zwischenablage mit automatischem Leeren
  lib.rs              Tauri Commands
```

## Notizen zur Passwords API

Dokumentation: [Developer Handbook](https://git.mdns.eu/nextcloud/passwords/-/wikis/Developers/Index)

- Basis: `https://<server>/index.php/apps/passwords/api/1.0/`, Basic Auth mit Benutzer und App Passwort, nur HTTPS.
- Session: `GET session/request` liefert, was zum Öffnen nötig ist (`challenge`, `token`).
  `POST session/open` öffnet die Session und liefert bei aktiver Verschlüsselung die verschlüsselten Keychains.
  Der Header `X-API-SESSION` muss bei jeder Anfrage mit dem zuletzt erhaltenen Wert mitgeschickt werden.
  `GET session/keepalive` hält sie offen. Nach fünf falschen Versuchen sperrt der Server den Client.
- PWDv1 Challenge: `generichash(64, passwort + salt0, key = salt1)` → `pwhash(32, hash, salt2, INTERACTIVE, Argon2id13)` → hex.
- CSEv1 Keychain: hex(`salt(16) + nonce(24) + secretbox(json)`), Schlüssel = `pwhash(32, passwort, salt)`.
  Inhalt `{ keys: { uuid: hexKey }, current: uuid }`.
- CSEv1 Felder: hex(`nonce + secretbox(wert)`) mit dem Schlüssel aus `cseKey`.
  Verschlüsselt werden bei Passwörtern `label, username, password, url, notes, customFields`, bei Ordnern `label`, bei Tags `label, color`.
- Passwörter: `password/list|show|find|create|update|delete|restore`. Beim Update die bekannte `revision` mitsenden,
  dann lehnt der Server veraltete Stände ab. Bei CSE muss der Client den SHA1 `hash` selbst berechnen.
- Ordner: `folder/list|create|update|delete`, Basisordner ist `00000000-0000-0000-0000-000000000000`.

## Lizenz

MIT
