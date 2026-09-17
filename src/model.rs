//! Domain data structures. These are UI-agnostic and API-agnostic:
//! they describe *what* the app knows, not *how* it was fetched.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrStatus {
    Open,
    Draft,
    Closed,
    Merged,
}

impl PrStatus {
    pub fn label(&self) -> &'static str {
        match self {
            PrStatus::Open => "Open",
            PrStatus::Draft => "Draft",
            PrStatus::Closed => "Closed",
            PrStatus::Merged => "Merged",
        }
    }

    pub fn color(&self) -> gpui::Hsla {
        match self {
            PrStatus::Open => gpui::rgb(0x3fb950).into(),
            PrStatus::Draft => gpui::rgb(0x6e7681).into(),
            PrStatus::Closed => gpui::rgb(0x8b949e).into(),
            PrStatus::Merged => gpui::rgb(0xa371f7).into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequest {
    pub title: String,
    pub repo: String,
    #[allow(dead_code)]
    pub url: String,
    pub status: PrStatus,
    pub updated_at: String,
}

impl PullRequest {
    pub fn status(&self) -> &PrStatus {
        &self.status
    }
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