use serde::{Deserialize, Serialize};

use super::{CiStatus, PrStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequest {
    #[serde(default)]
    pub id: String,
    pub title: String,
    pub repo: String,
    #[serde(default)]
    pub number: u32,
    #[allow(dead_code)]
    pub url: String,
    pub status: PrStatus,
    pub ci: CiStatus,
    #[serde(default)]
    pub approvals: u32,
    #[serde(default)]
    pub required_approvals: u32,
    #[serde(default)]
    pub has_conflicts: bool,
    #[serde(default)]
    pub branch: String,
    pub updated_at: String,
}

impl PullRequest {
    pub fn status(&self) -> &PrStatus {
        &self.status
    }
}
