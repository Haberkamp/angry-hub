use std::sync::{Arc, OnceLock};

use crate::models::{ActivityItem, DeviceCode, PullRequest};

mod github;

pub fn code_host() -> Arc<dyn CodeHost> {
    static HOST: OnceLock<Arc<dyn CodeHost>> = OnceLock::new();
    HOST.get_or_init(|| Arc::new(github::GithubHost::new()))
        .clone()
}

pub type DataSourceResult<T> = Result<T, DataSourceError>;

#[derive(Debug, Clone)]
pub struct DataSourceError {
    pub message: String,
    session_ended: bool,
}

impl DataSourceError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            session_ended: false,
        }
    }

    pub fn session_ended() -> Self {
        Self {
            message: "session ended".into(),
            session_ended: true,
        }
    }

    pub fn is_session_ended(&self) -> bool {
        self.session_ended
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

    fn pull_request_snapshot(&self) -> Option<Vec<PullRequest>> {
        None
    }

    fn my_pull_requests(&self) -> DataSourceResult<Vec<PullRequest>>;

    fn close_pull_request(&self, id: &str) -> DataSourceResult<()>;

    fn set_pull_request_draft(&self, id: &str, draft: bool) -> DataSourceResult<()>;

    fn oauth_app_restricted_from_repo(&self, name_with_owner: &str) -> DataSourceResult<bool>;

    fn activity_snapshot(&self) -> Option<Vec<ActivityItem>> {
        None
    }

    fn my_pr_activity(&self) -> DataSourceResult<Vec<ActivityItem>>;

    fn logout(&self);
}
