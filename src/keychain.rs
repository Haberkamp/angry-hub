#![allow(dead_code)]

use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::session::Tokens;

#[derive(Serialize, Deserialize)]
struct Stored {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
}

pub trait SecretStore {
    fn load(&self) -> Option<String>;
    fn save(&self, secret: &str) -> Result<(), String>;
    fn delete(&self);
}

pub struct MemoryStore {
    secret: Mutex<Option<String>>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self {
            secret: Mutex::new(None),
        }
    }
}

impl SecretStore for MemoryStore {
    fn load(&self) -> Option<String> {
        self.secret.lock().expect("memory store").clone()
    }

    fn save(&self, secret: &str) -> Result<(), String> {
        *self.secret.lock().expect("memory store") = Some(secret.to_string());
        Ok(())
    }

    fn delete(&self) {
        *self.secret.lock().expect("memory store") = None;
    }
}

pub fn encode(tokens: &Tokens) -> Result<String, String> {
    serde_json::to_string(&Stored {
        access_token: tokens.access_token.clone(),
        refresh_token: tokens.refresh_token.clone(),
    })
    .map_err(|error| format!("could not encode credentials: {error}"))
}

pub fn decode(secret: &str) -> Option<Tokens> {
    let stored = serde_json::from_str::<Stored>(secret).ok()?;
    if stored.access_token.is_empty() {
        return None;
    }
    Some(Tokens {
        access_token: stored.access_token,
        refresh_token: stored
            .refresh_token
            .filter(|token| !token.is_empty()),
    })
}

#[cfg(target_os = "macos")]
const KEYCHAIN_ACCOUNT: &str = "github-oauth";

#[cfg(all(target_os = "macos", debug_assertions))]
const KEYCHAIN_SERVICE: &str = "dev.haberkamp.angryhub.dev";

#[cfg(all(target_os = "macos", not(debug_assertions)))]
const KEYCHAIN_SERVICE: &str = "dev.haberkamp.angryhub";

/// Login keychain entry. This is the unlocked keychain for the logged-in user,
/// so reading it does not ask for a password.
#[cfg(target_os = "macos")]
pub struct KeychainStore;

#[cfg(target_os = "macos")]
impl SecretStore for KeychainStore {
    fn load(&self) -> Option<String> {
        entry().ok()?.get_password().ok()
    }

    fn save(&self, secret: &str) -> Result<(), String> {
        entry()?
            .set_password(secret)
            .map_err(|error| format!("could not save credentials to Keychain: {error}"))
    }

    fn delete(&self) {
        if let Ok(entry) = entry() {
            let _ = entry.delete_credential();
        }
    }
}

#[cfg(target_os = "macos")]
fn entry() -> Result<keyring_core::Entry, String> {
    use apple_native_keyring_store::keychain::Store;
    use keyring_core::api::CredentialStoreApi;

    let store = Store::new().map_err(|error| format!("keychain unavailable: {error}"))?;
    store
        .build(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT, None)
        .map_err(|error| format!("keychain entry failed: {error}"))
}
