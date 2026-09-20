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
            PrStatus::Open => crate::color::green::s9(),
            PrStatus::Draft => crate::color::gray::s9(),
            PrStatus::Closed => crate::color::red::s9(),
            PrStatus::Merged => crate::color::violet::s11(),
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
            CiStatus::Success => crate::color::green::s9(),
            CiStatus::Failure => crate::color::red::s9(),
            CiStatus::Pending => crate::color::blue::s8(),
            CiStatus::None => crate::color::gray::s9(),
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
    #[serde(default)]
    pub approvals: u32,
    #[serde(default)]
    pub required_approvals: u32,
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
    Closed,
    Reopened,
    Comment,
    Approved,
    ChangesRequested,
}

impl ActivityKind {
    #[allow(dead_code)]
    pub fn label(&self) -> &'static str {
        match self {
            ActivityKind::Merged => "merged",
            ActivityKind::Closed => "closed",
            ActivityKind::Reopened => "reopened",
            ActivityKind::Comment => "commented",
            ActivityKind::Approved => "approved",
            ActivityKind::ChangesRequested => "requested changes",
        }
    }

    pub fn color(&self) -> gpui::Hsla {
        match self {
            ActivityKind::Merged => crate::color::violet::s11(),
            ActivityKind::Closed => crate::color::red::s9(),
            ActivityKind::Reopened => crate::color::green::s9(),
            ActivityKind::Comment => crate::color::blue::s9(),
            ActivityKind::Approved => crate::color::green::s9(),
            ActivityKind::ChangesRequested => crate::color::red::s9(),
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
    pub number: u32,
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
