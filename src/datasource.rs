use crate::model::{ActivityItem, DeviceCode, PullRequest};

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

    pub fn is_oauth_app_restricted(&self) -> bool {
        is_oauth_app_restricted_message(&self.message)
    }
}

pub fn is_oauth_app_restricted_message(message: &str) -> bool {
    message.contains("OAuth App access restrictions")
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

    fn close_pull_request(&self, id: &str) -> DataSourceResult<()>;

    fn oauth_app_restricted_from_repo(&self, name_with_owner: &str) -> DataSourceResult<bool>;

    fn my_pr_activity(&self) -> DataSourceResult<Vec<ActivityItem>>;

    fn logout(&self);
}

pub trait AuthStore: Send + Sync {
    fn load_token(&self) -> Option<String>;
    fn save_token(&self, token: &str);
    fn clear(&self);
}
