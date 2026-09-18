use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrStatus {
    Open,
    Draft,
    Closed,
    Merged,
}

impl PrStatus {
    #[allow(dead_code)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CiStatus {
    Success,
    Failure,
    Pending,
    None,
}

impl CiStatus {
    pub fn color(&self) -> gpui::Hsla {
        match self {
            CiStatus::Success => gpui::rgb(0x3fb950).into(),
            CiStatus::Failure => gpui::rgb(0xf85149).into(),
            CiStatus::Pending => gpui::rgb(0xd29922).into(),
            CiStatus::None => gpui::rgb(0x6e7681).into(),
        }
    }
}

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
    pub updated_at: String,
}

impl PullRequest {
    pub fn status(&self) -> &PrStatus {
        &self.status
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivityKind {
    Merged,
    Comment,
    Approved,
    ChangesRequested,
}

impl ActivityKind {
    pub fn label(&self) -> &'static str {
        match self {
            ActivityKind::Merged => "merged",
            ActivityKind::Comment => "commented",
            ActivityKind::Approved => "approved",
            ActivityKind::ChangesRequested => "requested changes",
        }
    }

    pub fn color(&self) -> gpui::Hsla {
        match self {
            ActivityKind::Merged => gpui::rgb(0xa371f7).into(),
            ActivityKind::Comment => gpui::rgb(0x58a6ff).into(),
            ActivityKind::Approved => gpui::rgb(0x3fb950).into(),
            ActivityKind::ChangesRequested => gpui::rgb(0xf85149).into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ActivityItem {
    pub kind: ActivityKind,
    pub actor: String,
    pub avatar_url: Option<String>,
    pub pr_title: String,
    pub repo: String,
    pub url: String,
    pub occurred_at: String,
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
