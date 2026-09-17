//! Domain data structures. These are UI-agnostic and API-agnostic:
//! they describe *what* the app knows, not *how* it was fetched.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PullRequestState {
    Open,
    Closed,
    Merged,
}

impl PullRequestState {
    pub fn label(&self) -> &'static str {
        match self {
            PullRequestState::Open => "open",
            PullRequestState::Closed => "closed",
            PullRequestState::Merged => "merged",
        }
    }
}

#[derive(Debug, Clone)]
pub struct PullRequest {
    pub title: String,
    pub repo: String,
    #[allow(dead_code)]
    pub url: String,
    pub state: PullRequestState,
    pub draft: bool,
}

#[derive(Debug, Clone)]
pub struct DeviceCode {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub poll_interval_secs: u64,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct User {
    pub login: String,
}