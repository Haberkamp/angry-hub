//! The interface between the app and any code-hosting provider.
//! The UI depends only on this trait, never on a concrete
//! implementation, so GitHub could in principle be swapped for
//! GitLab, Gitea, etc. by implementing `CodeHost` and providing a
//! different `AuthStore`.

use crate::model::{DeviceCode, PullRequest};

pub type DataSourceResult<T> = Result<T, DataSourceError>;

#[derive(Debug, Clone)]
pub struct DataSourceError {
    pub message: String,
}

impl DataSourceError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for DataSourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

/// Result of successfully completing device-flow authentication.
#[derive(Debug, Clone)]
pub struct AuthSuccess;

/// The provider half of the interface: everything the app needs from a
/// code-hosting service. Implementations are expected to be callable
/// from a background thread.
pub trait CodeHost: Send + Sync {
    /// Whether a previously-established session exists (e.g. saved token).
    fn has_saved_session(&self) -> bool;

    /// Begin interactive device-flow login. Returns a code the user
    /// must enter at `DeviceCode::verification_uri`.
    fn start_login(&self) -> DataSourceResult<DeviceCode>;

    /// Block, polling until the user authorizes (or the flow fails).
    fn await_login(&self, code: &DeviceCode) -> DataSourceResult<AuthSuccess>;

    /// Fetch all pull requests authored by the logged-in user.
    fn my_pull_requests(&self) -> DataSourceResult<Vec<PullRequest>>;

    /// Forget the stored session, if any.
    fn logout(&self);
}

/// The session-storage half of the interface: token persistence,
/// independent of which provider issued the token.
pub trait AuthStore: Send + Sync {
    fn load_token(&self) -> Option<String>;
    fn save_token(&self, token: &str);
    fn clear(&self);
}