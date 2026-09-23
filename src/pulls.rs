use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use homestead::rusqlite::{self, Transaction};
use homestead::{Mutator, Store, Table};

pub const SYNC_ROW: &str = "github";
pub const INITIAL_LIMIT: usize = 500;

#[derive(Clone, Debug, PartialEq, Eq, Table)]
pub struct PullRequest {
    pub id: String,
    pub number: i64,
    pub title: String,
    pub state: String,
    pub url: String,
    pub repository: String,
    pub author: String,
    pub updated_at: String,
    pub ci: String,
    pub approvals: i64,
    pub required_approvals: i64,
    pub has_conflicts: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Table)]
pub struct SyncState {
    pub id: String,
    pub watermark: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    PrOpened(PullRequest),
    PrUpdated(PullRequest),
    PrClosed(PullRequest),
    PrMerged(PullRequest),
    Watermark { at: String },
}

pub struct PullMutator;

impl Mutator<Event> for PullMutator {
    fn apply(&self, tx: &Transaction<'_>, event: &Event) -> rusqlite::Result<()> {
        match event {
            Event::PrOpened(pr) | Event::PrUpdated(pr) | Event::PrClosed(pr) | Event::PrMerged(pr) => {
                upsert_pull(tx, pr)?;
            }
            Event::Watermark { at } => {
                SyncState::where_eq("id", SYNC_ROW).delete(tx)?;
                SyncState::create(
                    tx,
                    &SyncState {
                        id: SYNC_ROW.into(),
                        watermark: at.clone(),
                    },
                )?;
            }
        }
        Ok(())
    }
}

fn upsert_pull(tx: &Transaction<'_>, pr: &PullRequest) -> rusqlite::Result<()> {
    PullRequest::where_eq("id", pr.id.as_str()).delete(tx)?;
    PullRequest::create(tx, pr)?;
    Ok(())
}

pub fn open() -> homestead::Result<Store<Event>> {
    let path = database_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Store::open(path, migrations_dir(), PullMutator)
}

fn database_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("angry-hub")
        .join("homestead.db")
}

fn migrations_dir() -> PathBuf {
    if let Ok(exe) = std::env::current_exe()
        && let Some(resources) = exe.parent().map(|dir| dir.join("../Resources/migrations"))
        && resources.is_dir()
    {
        return resources;
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations")
}

/// Diff a GitHub page against the local rows and emit one event per change.
pub fn changeset(local: &[PullRequest], remote: &[PullRequest]) -> Vec<Event> {
    let local: HashMap<&str, &PullRequest> = local.iter().map(|pr| (pr.id.as_str(), pr)).collect();
    let mut events = Vec::new();
    for pr in remote {
        let Some(event) = classify(local.get(pr.id.as_str()).copied(), pr) else {
            continue;
        };
        events.push(event);
    }
    events
}

fn classify(previous: Option<&PullRequest>, next: &PullRequest) -> Option<Event> {
    match previous {
        None if next.state == "merged" => Some(Event::PrMerged(next.clone())),
        None if next.state == "closed" => Some(Event::PrClosed(next.clone())),
        None => Some(Event::PrOpened(next.clone())),
        Some(previous) if previous == next => None,
        Some(previous) if next.state == "merged" && previous.state != "merged" => {
            Some(Event::PrMerged(next.clone()))
        }
        Some(previous) if next.state == "closed" && previous.state != "closed" => {
            Some(Event::PrClosed(next.clone()))
        }
        Some(_) => Some(Event::PrUpdated(next.clone())),
    }
}

/// Initial sync runs only when no watermark has been stored.
pub fn search_query(watermark: Option<&str>) -> String {
    match watermark {
        None => "author:@me is:pr sort:updated-desc".into(),
        Some(watermark) => {
            format!("author:@me is:pr updated:>={watermark} sort:updated-desc")
        }
    }
}

/// Advance the watermark only after a complete fetch. A truncated fetch keeps
/// the previous mark so the next sync still sees the rest of the window.
pub fn next_watermark(previous: Option<&str>, remote: &[PullRequest], complete: bool) -> String {
    if !complete {
        return previous.unwrap_or("1970-01-01T00:00:00Z").to_string();
    }
    let newest = remote.iter().map(|pr| pr.updated_at.as_str()).max();
    match (previous, newest) {
        (Some(previous), Some(newest)) if previous > newest => previous.to_string(),
        (_, Some(newest)) => newest.to_string(),
        (Some(previous), None) => previous.to_string(),
        (None, None) => "1970-01-01T00:00:00Z".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pr(id: &str, state: &str, updated_at: &str) -> PullRequest {
        PullRequest {
            id: id.into(),
            number: 1,
            title: "title".into(),
            state: state.into(),
            url: "https://github.com/acme/app/pull/1".into(),
            repository: "acme/app".into(),
            author: "nils".into(),
            updated_at: updated_at.into(),
            ci: "none".into(),
            approvals: 0,
            required_approvals: 0,
            has_conflicts: false,
        }
    }

    #[test]
    fn initial_query_is_open_pulls_sorted_by_update() {
        assert_eq!(
            search_query(None),
            "author:@me is:pr sort:updated-desc"
        );
    }

    #[test]
    fn later_query_filters_on_the_watermark() {
        assert_eq!(
            search_query(Some("2024-01-02T00:00:00Z")),
            "author:@me is:pr updated:>=2024-01-02T00:00:00Z sort:updated-desc"
        );
    }

    #[test]
    fn changeset_emits_opened_updated_closed_and_merged() {
        let local = vec![
            pr("open", "open", "2024-01-01T00:00:00Z"),
            pr("closing", "open", "2024-01-01T00:00:00Z"),
            pr("merging", "open", "2024-01-01T00:00:00Z"),
        ];
        let mut updated = pr("open", "open", "2024-01-02T00:00:00Z");
        updated.title = "renamed".into();
        let remote = vec![
            updated,
            pr("closing", "closed", "2024-01-02T00:00:00Z"),
            pr("merging", "merged", "2024-01-02T00:00:00Z"),
            pr("fresh", "open", "2024-01-02T00:00:00Z"),
        ];

        let events = changeset(&local, &remote);

        assert!(matches!(events[0], Event::PrUpdated(_)));
        assert!(matches!(events[1], Event::PrClosed(_)));
        assert!(matches!(events[2], Event::PrMerged(_)));
        assert!(matches!(events[3], Event::PrOpened(_)));
    }

    #[test]
    fn unchanged_rows_emit_nothing() {
        let row = pr("same", "open", "2024-01-01T00:00:00Z");
        assert!(changeset(&[row.clone()], &[row]).is_empty());
    }

    #[test]
    fn watermark_stays_put_when_the_fetch_is_truncated() {
        let remote = vec![pr("a", "open", "2024-06-01T00:00:00Z")];
        assert_eq!(
            next_watermark(Some("2024-01-01T00:00:00Z"), &remote, false),
            "2024-01-01T00:00:00Z"
        );
    }

    #[test]
    fn watermark_moves_to_the_newest_complete_update() {
        let remote = vec![
            pr("a", "open", "2024-06-01T00:00:00Z"),
            pr("b", "merged", "2024-05-01T00:00:00Z"),
        ];
        assert_eq!(
            next_watermark(Some("2024-01-01T00:00:00Z"), &remote, true),
            "2024-06-01T00:00:00Z"
        );
    }
}
