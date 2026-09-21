use serde::{Deserialize, Serialize};

use super::ActivityKind;

#[derive(Debug, Clone, Serialize, Deserialize)]
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
