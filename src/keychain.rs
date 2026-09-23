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
        refresh_token: stored.refresh_token.filter(|token| !token.is_empty()),
    })
}

#[cfg(all(target_os = "macos", not(debug_assertions)))]
const KEYCHAIN_ACCOUNT: &str = "github-oauth";

#[cfg(all(target_os = "macos", not(debug_assertions)))]
const KEYCHAIN_SERVICE: &str = "dev.haberkamp.angryhub";

/// Login keychain entry used by release builds. Debug builds keep the session
/// in a file because each `cargo run` re-signs the binary and Keychain would
/// prompt again.
#[cfg(all(target_os = "macos", not(debug_assertions)))]
pub struct CredentialStore;

/// Session file for debug builds. Mode `0600` under Application Support, so
/// `just run` can reload the token without a Keychain prompt.
#[cfg(all(target_os = "macos", debug_assertions))]
pub struct CredentialStore;

#[cfg(all(target_os = "macos", not(debug_assertions)))]
impl SecretStore for CredentialStore {
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

#[cfg(all(target_os = "macos", debug_assertions))]
fn session_path() -> std::path::PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("angry-hub")
        .join("session.json")
}

#[cfg(all(target_os = "macos", debug_assertions))]
impl SecretStore for CredentialStore {
    fn load(&self) -> Option<String> {
        std::fs::read_to_string(session_path()).ok()
    }

    fn save(&self, secret: &str) -> Result<(), String> {
        let path = session_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        std::fs::write(&path, secret).map_err(|error| format!("could not save session: {error}"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }

    fn delete(&self) {
        let _ = std::fs::remove_file(session_path());
    }
}

#[cfg(all(target_os = "macos", not(debug_assertions)))]
fn entry() -> Result<keyring_core::Entry, String> {
    use apple_native_keyring_store::keychain::Store;
    use keyring_core::api::CredentialStoreApi;

    let store = Store::new().map_err(|error| format!("keychain unavailable: {error}"))?;
    store
        .build(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT, None)
        .map_err(|error| format!("keychain entry failed: {error}"))
}
