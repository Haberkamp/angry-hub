//! Credential store: access token in process memory, refresh token in the
//! macOS Data Protection keychain.
//!
//! Accessibility is `kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly` via the
//! keyring `access-policy` modifier. That gates on device unlock after reboot,
//! not on Touch ID or the login password. Do not set `RequireUserPresence` or
//! other `kSecAccessControl*` user-presence flags — those prompt on every read.

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
    pub fn save_tokens(&self, access_token: String, refresh_token: Option<String>) {
        set_access_token(Some(access_token.clone()));
        if let Some(refresh_token) = refresh_token.filter(|token| !token.is_empty()) {
            persist(Persisted::refresh(refresh_token));
        } else if self.refresh_token().is_none() {
            persist(Persisted::access_only(access_token));
        }
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
fn data_protection_entry() -> Option<keyring_core::Entry> {
    use apple_native_keyring_store::protected::Store;
    use keyring_core::api::CredentialStoreApi;

    let store = Store::new().ok()?;
    let modifiers =
        std::collections::HashMap::from([("access-policy", "after-first-unlock-this-device-only")]);
    store
        .build(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT, Some(&modifiers))
        .ok()
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

fn persist(stored: Persisted) {
    set_memory(Some(stored.clone()));
    keychain_save(&stored);
    delete_legacy_token_file();
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
    let secret = data_protection_entry()?.get_password().ok()?;
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
fn keychain_save(stored: &Persisted) {
    let Some(entry) = data_protection_entry() else {
        return;
    };
    let Ok(json) = serde_json::to_string(stored) else {
        return;
    };
    let _ = entry.set_password(&json);
}

#[cfg(not(target_os = "macos"))]
fn keychain_save(_stored: &Persisted) {}

#[cfg(target_os = "macos")]
fn keychain_clear() {
    if let Some(entry) = data_protection_entry() {
        let _ = entry.delete_credential();
    }
}

#[cfg(not(target_os = "macos"))]
fn keychain_clear() {}
