use crate::json_file;
use crate::models::{ActivityItem, PullRequest};

pub struct PrsCache;

impl PrsCache {
    pub fn load() -> Option<Vec<PullRequest>> {
        json_file::load("prs.json")
    }

    pub fn save(prs: &[PullRequest]) {
        json_file::save("prs.json", &prs);
    }

    pub fn clear() {
        json_file::remove("prs.json");
    }
}

pub struct ActivityCache;

impl ActivityCache {
    pub fn load() -> Option<Vec<ActivityItem>> {
        json_file::load("activity.json")
    }

    pub fn save(items: &[ActivityItem]) {
        json_file::save("activity.json", &items);
    }

    pub fn clear() {
        json_file::remove("activity.json");
    }
}
