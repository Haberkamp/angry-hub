//! Credential store: access token in process memory, refresh token in the
//! macOS **login** keychain (generic password).
//!
//! The Data Protection (“protected”) keychain is for sandboxed apps. This
//! product is a Developer ID `.app` without App Sandbox, so writes there are
//! silently dropped and a restart looks logged out. The login keychain is
//! available to all apps and shows up in Keychain Access.

use std::path::PathBuf;
use std::sync::Mutex;

#[cfg(target_os = "macos")]
use serde::{Deserialize, Serialize};

static ACCESS_TOKEN: Mutex<Option<String>> = Mutex::new(None);
static REFRESH_LOCK: Mutex<()> = Mutex::new(());

#[cfg(target_os = "macos")]
const KEYCHAIN_SERVICE: &str = "dev.haberkamp.angryhub";
#[cfg(target_os = "macos")]
const KEYCHAIN_ACCOUNT: &str = "github-oauth";

pub struct TokenStore;

impl TokenStore {
    pub fn has_credentials(&self) -> bool {
        delete_legacy_token_file();
        persisted().is_some() || access_token().is_some()
    }

    pub fn access_token(&self) -> Option<String> {
        access_token()
    }

    pub fn refresh_token(&self) -> Option<String> {
        refresh_token()
    }

    pub fn lock_refresh(&self) -> std::sync::MutexGuard<'static, ()> {
        REFRESH_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Keep the access token in memory. Persist a refresh token when GitHub
    /// issues one; otherwise persist the access token so the next launch can
    /// restore a session.
    pub fn save_tokens(
        &self,
        access_token: String,
        refresh_token: Option<String>,
    ) -> Result<(), String> {
        set_access_token(Some(access_token.clone()));
        if let Some(refresh_token) = refresh_token.filter(|token| !token.is_empty()) {
            persist(Persisted::refresh(refresh_token))?;
        } else if self.refresh_token().is_none() {
            persist(Persisted::access_only(access_token))?;
        }
        Ok(())
    }

    pub fn clear(&self) {
        set_access_token(None);
        persist_clear();
    }
}

fn access_token() -> Option<String> {
    ACCESS_TOKEN
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

fn set_access_token(token: Option<String>) {
    *ACCESS_TOKEN
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = token;
}

fn refresh_token() -> Option<String> {
    persisted().and_then(|stored| stored.refresh_token.filter(|token| !token.is_empty()))
}

#[derive(Clone, Default)]
#[cfg_attr(target_os = "macos", derive(Deserialize, Serialize))]
struct Persisted {
    #[cfg_attr(
        target_os = "macos",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    refresh_token: Option<String>,
    #[cfg_attr(
        target_os = "macos",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    access_token: Option<String>,
}

impl Persisted {
    fn refresh(refresh_token: String) -> Self {
        Self {
            refresh_token: Some(refresh_token),
            access_token: None,
        }
    }

    fn access_only(access_token: String) -> Self {
        Self {
            refresh_token: None,
            access_token: Some(access_token),
        }
    }
}

#[cfg(target_os = "macos")]
fn login_keychain_entry() -> Result<keyring_core::Entry, String> {
    use apple_native_keyring_store::keychain::Store;
    use keyring_core::api::CredentialStoreApi;

    let store = Store::new().map_err(|e| format!("keychain unavailable: {e}"))?;
    store
        .build(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT, None)
        .map_err(|e| format!("keychain entry failed: {e}"))
}

static MEMORY: Mutex<Option<Persisted>> = Mutex::new(None);

fn memory() -> Option<Persisted> {
    MEMORY
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

fn set_memory(stored: Option<Persisted>) {
    *MEMORY
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = stored;
}

fn persisted() -> Option<Persisted> {
    if let Some(stored) = keychain_load() {
        return Some(stored);
    }
    memory()
}

fn persist(stored: Persisted) -> Result<(), String> {
    keychain_save(&stored)?;
    set_memory(Some(stored));
    delete_legacy_token_file();
    Ok(())
}

fn persist_clear() {
    set_memory(None);
    keychain_clear();
    delete_legacy_token_file();
}

fn delete_legacy_token_file() {
    let path = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("angry-hub")
        .join("token.json");
    let _ = std::fs::remove_file(path);
}

#[cfg(target_os = "macos")]
fn keychain_load() -> Option<Persisted> {
    let secret = login_keychain_entry().ok()?.get_password().ok()?;
    if let Ok(stored) = serde_json::from_str::<Persisted>(&secret)
        && (stored.refresh_token.as_ref().is_some_and(|t| !t.is_empty())
            || stored.access_token.as_ref().is_some_and(|t| !t.is_empty()))
    {
        if stored.access_token.is_some() && access_token().is_none() {
            set_access_token(stored.access_token.clone());
        }
        return Some(stored);
    }
    let refresh_token = secret.trim();
    if refresh_token.is_empty() {
        return None;
    }
    Some(Persisted::refresh(refresh_token.to_string()))
}

#[cfg(not(target_os = "macos"))]
fn keychain_load() -> Option<Persisted> {
    None
}

#[cfg(target_os = "macos")]
fn keychain_save(stored: &Persisted) -> Result<(), String> {
    let entry = login_keychain_entry()?;
    let json =
        serde_json::to_string(stored).map_err(|e| format!("could not encode credentials: {e}"))?;
    entry
        .set_password(&json)
        .map_err(|e| format!("could not save credentials to Keychain: {e}"))?;
    if entry.get_password().ok().as_deref() != Some(json.as_str()) {
        return Err("Keychain write did not stick".into());
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn keychain_save(_stored: &Persisted) -> Result<(), String> {
    Ok(())
}

#[cfg(target_os = "macos")]
fn keychain_clear() {
    if let Ok(entry) = login_keychain_entry() {
        let _ = entry.delete_credential();
    }
}

#[cfg(not(target_os = "macos"))]
fn keychain_clear() {}
