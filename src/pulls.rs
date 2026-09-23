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
    pub branch: String,
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
    crate::paths::support_dir().join("homestead.db")
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

/// Home list: open and draft pull requests, plus anything merged in the last
/// five minutes. Merged, then open, then draft; newest update first inside
/// each group. `updated_at` is compared in SQLite so the window moves on
/// every refresh of this query.
pub fn visible() -> homestead::Select<PullRequest> {
    PullRequest::query()
        .where_raw(
            "\"state\" IN ('open', 'draft') OR (\"state\" = 'merged' AND \"updated_at\" >= strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-5 minutes'))",
            std::iter::empty::<&str>(),
        )
        .order_by_raw(
            "CASE \"state\" WHEN 'merged' THEN 0 WHEN 'open' THEN 1 WHEN 'draft' THEN 2 ELSE 3 END, \"updated_at\" DESC",
        )
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

    fn rfc3339(unix: i64) -> String {
        let days = unix.div_euclid(86_400);
        let secs = unix.rem_euclid(86_400);
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z.rem_euclid(146_097);
        let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
        let mut y = yoe + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = mp + if mp < 10 { 3 } else { -9 };
        if m <= 2 {
            y += 1;
        }
        let h = secs / 3_600;
        let min = (secs % 3_600) / 60;
        let s = secs % 60;
        format!("{y:04}-{m:02}-{d:02}T{h:02}:{min:02}:{s:02}Z")
    }

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
            branch: "feature".into(),
        }
    }

    #[test]
    fn visible_query_keeps_open_draft_and_recently_merged() {
        let dir = std::env::temp_dir().join(format!(
            "angry-hub-visible-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let mut store = Store::open(dir.join("db"), migrations_dir(), PullMutator).unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        for row in [
            pr("open", "open", &rfc3339(now - 3_600)),
            pr("draft", "draft", &rfc3339(now - 7_200)),
            pr("just-merged", "merged", &rfc3339(now - 30)),
            pr("old-merged", "merged", &rfc3339(now - 600)),
            pr("closed", "closed", &rfc3339(now - 10)),
        ] {
            store.commit(Event::PrOpened(row)).unwrap();
        }

        let rows = store.watch(visible()).unwrap().rows();
        let ids: Vec<_> = rows.iter().map(|row| row.id.as_str()).collect();
        assert_eq!(ids, vec!["just-merged", "open", "draft"]);
        let _ = fs::remove_dir_all(&dir);
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
