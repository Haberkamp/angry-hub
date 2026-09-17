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

#[derive(Debug, Clone)]
pub struct AuthSuccess;

pub trait CodeHost: Send + Sync {
    fn has_saved_session(&self) -> bool;

    fn start_login(&self) -> DataSourceResult<DeviceCode>;

    fn await_login(&self, code: &DeviceCode) -> DataSourceResult<AuthSuccess>;

    fn my_pull_requests(&self) -> DataSourceResult<Vec<PullRequest>>;

    fn logout(&self);
}

pub trait AuthStore: Send + Sync {
    fn load_token(&self) -> Option<String>;
    fn save_token(&self, token: &str);
    fn clear(&self);
}
