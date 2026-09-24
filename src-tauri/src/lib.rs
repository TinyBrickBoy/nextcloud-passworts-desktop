pub mod account;
pub mod api;
pub mod clipboard;
pub mod crypto;
pub mod vault;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::async_runtime::JoinHandle;
use tauri::{Manager, State};
use tauri_plugin_opener::OpenerExt;
use tokio::sync::Mutex;
use zeroize::Zeroizing;

use account::Account;
use api::{Api, TokenInfo};
use clipboard::Clipboard;
use crypto::{Keychain, CSE_TYPE};
use vault::{SaveInput, Vault, VaultView};

type CmdResult<T> = Result<T, String>;

const KEEPALIVE_EVERY: Duration = Duration::from_secs(4 * 60);

struct AppState {
    config_dir: PathBuf,
    vault: Mutex<Option<Vault>>,
    login_cancelled: AtomicBool,
    clipboard: Clipboard,
    keepalive: StdMutex<Option<JoinHandle<()>>>,
}

impl AppState {
    fn account(&self) -> CmdResult<Account> {
        Account::load(&self.config_dir).ok_or_else(|| "Kein Konto eingerichtet".to_string())
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
}

#[tauri::command]
async fn status(state: State<'_, AppState>) -> CmdResult<Status> {
    Ok(Status {
        account: Account::load(&state.config_dir),
        unlocked: state.vault.lock().await.is_some(),
    })
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
        .ok_or_else(|| "Anmeldung abgebrochen".to_string())?;

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
                return Err(format!("Challenge Typ {} wird nicht unterstützt", challenge.kind));
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
        (Some(_), None) => return Err("Verschlüsselungspasswort erforderlich".into()),
        (None, _) => None,
    };

    let opened = api
        .session_open(solution, token.map(|t| (t.id, t.code.trim().to_string())))
        .await
        .map_err(|e| match e {
            api::ApiError::Server(message) => format!("Entsperren fehlgeschlagen: {message}"),
            other => other.to_string(),
        })?;
    if !opened.success {
        return Err("Entsperren fehlgeschlagen".into());
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

    let mut vault = Vault::new(api.clone(), keychain);
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
    Ok(())
}

async fn with_vault<T>(state: &State<'_, AppState>, f: impl FnOnce(&Vault) -> CmdResult<T>) -> CmdResult<T> {
    let guard = state.vault.lock().await;
    f(guard.as_ref().ok_or_else(|| "Tresor ist gesperrt".to_string())?)
}

#[tauri::command]
async fn refresh(state: State<'_, AppState>) -> CmdResult<VaultView> {
    let mut guard = state.vault.lock().await;
    let vault = guard.as_mut().ok_or_else(|| "Tresor ist gesperrt".to_string())?;
    vault.refresh().await?;
    Ok(vault.view())
}

#[tauri::command]
async fn reveal(state: State<'_, AppState>, id: String) -> CmdResult<String> {
    with_vault(&state, |v| Ok(v.entry(&id)?.password.to_string())).await
}

#[tauri::command]
async fn reveal_field(state: State<'_, AppState>, id: String, index: usize) -> CmdResult<String> {
    with_vault(&state, |v| v.entry(&id)?.custom_value(index).ok_or_else(|| "Feld nicht gefunden".into())).await
}

#[tauri::command]
async fn copy(state: State<'_, AppState>, id: String, field: String) -> CmdResult<()> {
    let value = with_vault(&state, |v| {
        let entry = v.entry(&id)?;
        match field.as_str() {
            "password" => Ok(entry.password.to_string()),
            "username" => Ok(entry.username.clone()),
            "url" => Ok(entry.url.clone()),
            _ => Err("Unbekanntes Feld".into()),
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
struct Saved {
    id: String,
    vault: VaultView,
}

#[tauri::command]
async fn save(state: State<'_, AppState>, input: SaveInput) -> CmdResult<Saved> {
    let mut guard = state.vault.lock().await;
    let vault = guard.as_mut().ok_or_else(|| "Tresor ist gesperrt".to_string())?;
    let id = vault.save(input).await?;
    Ok(Saved { id, vault: vault.view() })
}

#[tauri::command]
async fn trash(state: State<'_, AppState>, id: String) -> CmdResult<VaultView> {
    let mut guard = state.vault.lock().await;
    let vault = guard.as_mut().ok_or_else(|| "Tresor ist gesperrt".to_string())?;
    vault.trash(&id).await?;
    Ok(vault.view())
}

#[tauri::command]
async fn generate(state: State<'_, AppState>) -> CmdResult<String> {
    let api = with_vault(&state, |v| Ok(v.api.clone())).await?;
    api.generate_password().await.map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            app.manage(AppState {
                config_dir,
                vault: Mutex::new(None),
                login_cancelled: AtomicBool::new(false),
                clipboard: Clipboard::new(),
                keepalive: StdMutex::new(None),
            });
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
            trash,
            generate,
        ])
        .run(tauri::generate_context!())
        .expect("Fehler beim Starten der Anwendung");
}
