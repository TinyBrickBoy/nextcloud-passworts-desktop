pub mod account;
pub mod api;
pub mod clipboard;
pub mod crypto;
pub mod generator;
pub mod i18n;
pub mod settings;
pub mod vault;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::async_runtime::JoinHandle;
use tauri::{Manager, State};
use tauri_plugin_opener::OpenerExt;
use base64::Engine;
use tokio::sync::Mutex;
use zeroize::Zeroizing;

use account::Account;
use api::{Api, TokenInfo};
use clipboard::Clipboard;
use crypto::{Keychain, CSE_TYPE};
use i18n::t;
use settings::Settings;
use vault::{SaveInput, Tag, Vault, VaultView};

type CmdResult<T> = Result<T, String>;

const KEEPALIVE_EVERY: Duration = Duration::from_secs(4 * 60);
/// Der Server erlaubt 15 Favicon Abrufe in 15 Sekunden.
const FAVICON_INTERVAL: Duration = Duration::from_millis(1100);

struct AppState {
    config_dir: PathBuf,
    vault: Mutex<Option<Vault>>,
    login_cancelled: AtomicBool,
    clipboard: Clipboard,
    keepalive: StdMutex<Option<JoinHandle<()>>>,
    settings: StdMutex<Settings>,
    /// Favicons nur im Arbeitsspeicher, damit keine Liste der Webseiten auf der Platte landet.
    favicons: StdMutex<HashMap<String, Option<String>>>,
    favicon_gate: Mutex<Option<Instant>>,
}

impl AppState {
    fn account(&self) -> CmdResult<Account> {
        Account::load(&self.config_dir).ok_or_else(|| t("no_account").to_string())
    }

    fn api(&self) -> CmdResult<Api> {
        let account = self.account()?;
        let password = Zeroizing::new(account.app_password()?);
        Api::new(&account.server, &account.user, &password).map_err(|e| e.to_string())
    }

    fn stop_keepalive(&self) {
        if let Some(handle) = self.keepalive.lock().unwrap().take() {
            handle.abort();
        }
    }
}

#[derive(Serialize)]
struct Status {
    account: Option<Account>,
    unlocked: bool,
    settings: Settings,
}

#[tauri::command]
async fn status(state: State<'_, AppState>) -> CmdResult<Status> {
    Ok(Status {
        account: Account::load(&state.config_dir),
        unlocked: state.vault.lock().await.is_some(),
        settings: state.settings.lock().unwrap().clone(),
    })
}

fn apply_settings(state: &AppState, settings: &Settings, system_lang: Option<&str>) {
    let lang = if settings.language == "auto" { system_lang.unwrap_or("en") } else { settings.language.as_str() };
    i18n::set_lang(lang);
    state.clipboard.set_clear_after(settings.clipboard_seconds);
}

/// Die Oberfläche meldet die Systemsprache, damit auch Backend Meldungen passen.
#[tauri::command]
fn set_system_language(state: State<'_, AppState>, lang: String) {
    let settings = state.settings.lock().unwrap().clone();
    apply_settings(&state, &settings, Some(&lang));
}

#[tauri::command]
fn set_settings(state: State<'_, AppState>, settings: Settings, system_lang: Option<String>) -> CmdResult<Settings> {
    settings.save(&state.config_dir)?;
    apply_settings(&state, &settings, system_lang.as_deref());
    *state.settings.lock().unwrap() = settings.clone();
    Ok(settings)
}

#[tauri::command]
async fn login_browser(app: tauri::AppHandle, state: State<'_, AppState>, server: String) -> CmdResult<Account> {
    let server = account::normalize_server(&server)?;
    state.login_cancelled.store(false, Ordering::SeqCst);
    let flow = account::flow_start(&server).await.map_err(|e| e.to_string())?;
    app.opener().open_url(&flow.login, None::<&str>).map_err(|e| e.to_string())?;

    let result = account::flow_poll(&flow.poll, || state.login_cancelled.load(Ordering::SeqCst))
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| t("login_cancelled").to_string())?;

    let app_password = Zeroizing::new(result.app_password);
    let account = Account { server: account::normalize_server(&result.server)?, user: result.login_name };
    account.save(&state.config_dir, &app_password)?;
    Ok(account)
}

#[tauri::command]
fn login_cancel(state: State<'_, AppState>) {
    state.login_cancelled.store(true, Ordering::SeqCst);
}

#[tauri::command]
async fn login_manual(state: State<'_, AppState>, server: String, user: String, app_password: String) -> CmdResult<Account> {
    let server = account::normalize_server(&server)?;
    let app_password = Zeroizing::new(app_password);
    let api = Api::new(&server, user.trim(), &app_password).map_err(|e| e.to_string())?;
    // Prüft die Zugangsdaten und ob die Passwords App installiert ist.
    api.session_request().await.map_err(|e| e.to_string())?;
    let account = Account { server, user: user.trim().to_string() };
    account.save(&state.config_dir, &app_password)?;
    Ok(account)
}

#[tauri::command]
async fn logout(state: State<'_, AppState>) -> CmdResult<()> {
    lock(state.clone()).await?;
    if let Some(account) = Account::load(&state.config_dir) {
        account.remove(&state.config_dir);
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UnlockRequirements {
    needs_password: bool,
    tokens: Vec<TokenInfo>,
}

#[tauri::command]
async fn unlock_requirements(state: State<'_, AppState>) -> CmdResult<UnlockRequirements> {
    let requirements = state.api()?.session_request().await.map_err(|e| e.to_string())?;
    Ok(UnlockRequirements { needs_password: requirements.challenge.is_some(), tokens: requirements.token })
}

#[derive(Deserialize)]
struct TokenInput {
    id: String,
    code: String,
}

#[tauri::command]
async fn unlock(state: State<'_, AppState>, password: Option<String>, token: Option<TokenInput>) -> CmdResult<VaultView> {
    let password = password.map(Zeroizing::new);
    let api = Arc::new(state.api()?);
    let requirements = api.session_request().await.map_err(|e| e.to_string())?;

    let solution = match (&requirements.challenge, &password) {
        (Some(challenge), Some(password)) => {
            if challenge.kind != "PWDv1r1" {
                return Err(format!("{} ({})", t("challenge_unsupported"), challenge.kind));
            }
            let password = password.clone();
            let salts = challenge.salts.clone();
            // Argon2 braucht 64 MiB und etwas Zeit, also nicht im async Thread rechnen.
            Some(
                tauri::async_runtime::spawn_blocking(move || crypto::solve_pwdv1(&password, &salts))
                    .await
                    .map_err(|e| e.to_string())?
                    .map_err(|e| e.to_string())?,
            )
        }
        (Some(_), None) => return Err(t("password_required").into()),
        (None, _) => None,
    };

    let opened = api
        .session_open(solution, token.map(|t| (t.id, t.code.trim().to_string())))
        .await
        .map_err(|e| match e {
            api::ApiError::Server(message) => format!("{}: {message}", t("unlock_failed")),
            other => other.to_string(),
        })?;
    if !opened.success {
        return Err(t("unlock_failed").into());
    }

    let keychain = match (opened.keys.get(CSE_TYPE), password) {
        (Some(encrypted), Some(password)) => {
            let encrypted = encrypted.clone();
            Some(
                tauri::async_runtime::spawn_blocking(move || Keychain::decrypt(&encrypted, &password))
                    .await
                    .map_err(|e| e.to_string())?
                    .map_err(|e| e.to_string())?,
            )
        }
        _ => None,
    };

    let lockable = requirements.challenge.is_some() || !requirements.token.is_empty();
    let mut vault = Vault::new(api.clone(), keychain, lockable);
    vault.refresh().await?;
    let view = vault.view();
    *state.vault.lock().await = Some(vault);

    state.stop_keepalive();
    let handle = tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(KEEPALIVE_EVERY).await;
            let _ = api.keepalive().await;
        }
    });
    *state.keepalive.lock().unwrap() = Some(handle);
    Ok(view)
}

#[tauri::command]
async fn lock(state: State<'_, AppState>) -> CmdResult<()> {
    state.stop_keepalive();
    if let Some(vault) = state.vault.lock().await.take() {
        vault.api.session_close().await;
    }
    state.favicons.lock().unwrap().clear();
    Ok(())
}

async fn with_vault<T>(state: &State<'_, AppState>, f: impl FnOnce(&Vault) -> CmdResult<T>) -> CmdResult<T> {
    let guard = state.vault.lock().await;
    f(guard.as_ref().ok_or_else(|| t("locked").to_string())?)
}

/// Führt eine ändernde Aktion auf dem Tresor aus und liefert danach die neue Ansicht.
macro_rules! mutate {
    ($state:expr, |$vault:ident| $body:expr) => {{
        let mut guard = $state.vault.lock().await;
        let $vault = guard.as_mut().ok_or_else(|| t("locked").to_string())?;
        let result = $body;
        (result, $vault.view())
    }};
}

#[tauri::command]
async fn refresh(state: State<'_, AppState>) -> CmdResult<VaultView> {
    let (result, view) = mutate!(state, |vault| vault.refresh().await);
    result.map(|_| view)
}

#[tauri::command]
async fn reveal(state: State<'_, AppState>, id: String) -> CmdResult<String> {
    with_vault(&state, |v| Ok(v.entry(&id)?.password.to_string())).await
}

#[tauri::command]
async fn reveal_field(state: State<'_, AppState>, id: String, index: usize) -> CmdResult<String> {
    with_vault(&state, |v| v.entry(&id)?.custom_value(index).ok_or_else(|| t("field_not_found").into())).await
}

#[tauri::command]
async fn copy(state: State<'_, AppState>, id: String, field: String) -> CmdResult<()> {
    let value = with_vault(&state, |v| {
        let entry = v.entry(&id)?;
        match field.as_str() {
            "password" => Ok(entry.password.to_string()),
            "username" => Ok(entry.username.clone()),
            "url" => Ok(entry.url.clone()),
            _ => Err(t("unknown_field").into()),
        }
    })
    .await?;
    state.clipboard.copy(value);
    Ok(())
}

#[tauri::command]
async fn copy_field(state: State<'_, AppState>, id: String, index: usize) -> CmdResult<()> {
    let value = reveal_field(state.clone(), id, index).await?;
    state.clipboard.copy(value);
    Ok(())
}

#[tauri::command]
fn copy_text(state: State<'_, AppState>, text: String) {
    state.clipboard.copy(text);
}

#[derive(Serialize)]
struct Saved<T> {
    id: T,
    vault: VaultView,
}

#[tauri::command]
async fn save(state: State<'_, AppState>, input: SaveInput) -> CmdResult<Saved<String>> {
    let (result, vault) = mutate!(state, |vault| vault.save(input).await);
    Ok(Saved { id: result?, vault })
}

#[tauri::command]
async fn set_favorite(state: State<'_, AppState>, id: String, favorite: bool) -> CmdResult<VaultView> {
    let (result, view) = mutate!(state, |vault| vault.set_favorite(&id, favorite).await);
    result.map(|_| view)
}

/// Aktive Einträge landen im Papierkorb, Einträge im Papierkorb werden endgültig gelöscht.
#[tauri::command]
async fn delete(state: State<'_, AppState>, id: String) -> CmdResult<VaultView> {
    let (result, view) = mutate!(state, |vault| vault.delete(&id).await);
    result.map(|_| view)
}

#[tauri::command]
async fn restore(state: State<'_, AppState>, id: String) -> CmdResult<VaultView> {
    let (result, view) = mutate!(state, |vault| vault.restore(&id).await);
    result.map(|_| view)
}

#[tauri::command]
async fn empty_trash(state: State<'_, AppState>) -> CmdResult<VaultView> {
    let (result, view) = mutate!(state, |vault| vault.empty_trash().await);
    result.map(|_| view)
}

#[tauri::command]
async fn create_folder(state: State<'_, AppState>, label: String, parent: Option<String>) -> CmdResult<Saved<String>> {
    let (result, vault) = mutate!(state, |vault| vault.create_folder(&label, parent).await);
    Ok(Saved { id: result?, vault })
}

#[tauri::command]
async fn rename_folder(state: State<'_, AppState>, id: String, label: String) -> CmdResult<VaultView> {
    let (result, view) = mutate!(state, |vault| vault.rename_folder(&id, &label).await);
    result.map(|_| view)
}

#[tauri::command]
async fn delete_folder(state: State<'_, AppState>, id: String) -> CmdResult<VaultView> {
    let (result, view) = mutate!(state, |vault| vault.delete_folder(&id).await);
    result.map(|_| view)
}

#[tauri::command]
async fn create_tag(state: State<'_, AppState>, label: String, color: String) -> CmdResult<Saved<Tag>> {
    let (result, vault) = mutate!(state, |vault| vault.create_tag(&label, &color).await);
    Ok(Saved { id: result?, vault })
}

#[tauri::command]
async fn delete_tag(state: State<'_, AppState>, id: String) -> CmdResult<VaultView> {
    let (result, view) = mutate!(state, |vault| vault.delete_tag(&id).await);
    result.map(|_| view)
}

#[tauri::command]
fn generate(options: Option<generator::Options>) -> String {
    generator::generate(&options.unwrap_or_default())
}

/// Passwort aus dem Generator des Servers (Wortpasswörter, gegen Datenlecks geprüft).
#[tauri::command]
async fn generate_server(state: State<'_, AppState>) -> CmdResult<String> {
    let api = with_vault(&state, |v| Ok(v.api.clone())).await?;
    api.generate_password().await.map_err(|e| e.to_string())
}

fn valid_domain(domain: &str) -> bool {
    !domain.is_empty()
        && domain.len() <= 253
        && domain.contains('.')
        && domain.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
}

/// Favicon einer Domain als data URL, `None` wenn keins geladen werden konnte.
#[tauri::command]
async fn favicon(state: State<'_, AppState>, domain: String) -> CmdResult<Option<String>> {
    let domain = domain.trim().to_lowercase();
    if !valid_domain(&domain) || !state.settings.lock().unwrap().favicons {
        return Ok(None);
    }
    if let Some(cached) = state.favicons.lock().unwrap().get(&domain) {
        return Ok(cached.clone());
    }
    let api = with_vault(&state, |v| Ok(v.api.clone())).await?;

    // Abrufe nacheinander und mit Abstand, damit das Rate Limit des Servers hält.
    let mut gate = state.favicon_gate.lock().await;
    if let Some(cached) = state.favicons.lock().unwrap().get(&domain) {
        return Ok(cached.clone());
    }
    if let Some(last) = *gate {
        let elapsed = last.elapsed();
        if elapsed < FAVICON_INTERVAL {
            tokio::time::sleep(FAVICON_INTERVAL - elapsed).await;
        }
    }
    let result = api.favicon(&domain, 64).await;
    *gate = Some(Instant::now());
    drop(gate);

    let data = result
        .ok()
        .filter(|bytes| bytes.starts_with(b"\x89PNG") || bytes.starts_with(b"<svg") || bytes.starts_with(b"\xff\xd8"))
        .map(|bytes| {
            let mime = if bytes.starts_with(b"\x89PNG") {
                "image/png"
            } else if bytes.starts_with(b"<svg") {
                "image/svg+xml"
            } else {
                "image/jpeg"
            };
            format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes))
        });
    state.favicons.lock().unwrap().insert(domain, data.clone());
    Ok(data)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            let settings = Settings::load(&config_dir);
            let state = AppState {
                config_dir,
                vault: Mutex::new(None),
                login_cancelled: AtomicBool::new(false),
                clipboard: Clipboard::new(),
                keepalive: StdMutex::new(None),
                settings: StdMutex::new(settings.clone()),
                favicons: StdMutex::new(HashMap::new()),
                favicon_gate: Mutex::new(None),
            };
            apply_settings(&state, &settings, None);
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            status,
            login_browser,
            login_cancel,
            login_manual,
            logout,
            unlock_requirements,
            unlock,
            lock,
            refresh,
            reveal,
            reveal_field,
            copy,
            copy_field,
            copy_text,
            save,
            set_favorite,
            delete,
            restore,
            empty_trash,
            create_folder,
            rename_folder,
            delete_folder,
            create_tag,
            delete_tag,
            generate,
            generate_server,
            favicon,
            set_settings,
            set_system_language,
        ])
        .run(tauri::generate_context!())
        .expect("failed to start the application");
}
