use homestead::Table;

use crate::github::{Github, GithubError, RemotePull};
use crate::pulls::{self, Event, PullRequest, SYNC_ROW, SyncState};

pub struct FetchedPulls {
    pub pulls: Vec<PullRequest>,
    pub complete: bool,
}

/// Pages authored pull requests one request at a time until `limit` or the
/// last page. `complete` is false when the cap cut the result short.
pub fn fetch_authored(
    github: &impl Github,
    token: &str,
    query: &str,
    limit: usize,
) -> Result<FetchedPulls, GithubError> {
    let mut pulls = Vec::new();
    let mut cursor = None;
    let mut complete = true;
    while pulls.len() < limit {
        let page = github.authored_pulls(token, query, cursor.as_deref())?;
        if page.pulls.is_empty() {
            break;
        }
        let room = limit - pulls.len();
        let page_len = page.pulls.len();
        pulls.extend(page.pulls.into_iter().take(room).map(into_pull));
        if page_len > room || (page.has_next_page && pulls.len() >= limit) {
            complete = false;
            break;
        }
        if !page.has_next_page {
            break;
        }
        let Some(next) = page.end_cursor else {
            break;
        };
        cursor = Some(next);
    }
    Ok(FetchedPulls { pulls, complete })
}

fn into_pull(pull: RemotePull) -> PullRequest {
    PullRequest {
        id: pull.id,
        number: pull.number,
        title: pull.title,
        state: pull.state,
        url: pull.url,
        repository: pull.repository,
        author: pull.author,
        updated_at: pull.updated_at,
        ci: pull.ci,
        approvals: pull.approvals,
        required_approvals: pull.required_approvals,
        has_conflicts: pull.has_conflicts,
        branch: pull.branch,
    }
}

/// Applies a fetched page. On a later sync (a watermark already exists), returns
/// pull requests that just transitioned to merged so the caller can notify.
pub fn apply(
    store: &mut homestead::Store<Event>,
    remote: &[PullRequest],
    complete: bool,
) -> homestead::Result<Vec<PullRequest>> {
    let watermark = store
        .watch(SyncState::where_eq("id", SYNC_ROW))?
        .rows()
        .into_iter()
        .next()
        .map(|state| state.watermark);
    let local = store.watch(PullRequest::query())?.rows();
    let announced = if watermark.is_some() {
        newly_merged(&local, remote)
    } else {
        Vec::new()
    };
    let mut events = pulls::changeset(&local, remote);
    let next = pulls::next_watermark(watermark.as_deref(), remote, complete);
    if watermark.as_deref() != Some(next.as_str()) {
        events.push(Event::Watermark { at: next });
    }
    for event in events {
        store.commit(event)?;
    }
    Ok(announced)
}

/// Local row was not merged, and this watermark page says it is now.
fn newly_merged(local: &[PullRequest], remote: &[PullRequest]) -> Vec<PullRequest> {
    remote
        .iter()
        .filter(|pr| pr.state == "merged")
        .filter(|pr| {
            local
                .iter()
                .any(|row| row.id == pr.id && row.state != "merged")
        })
        .cloned()
        .collect()
}

pub fn notify_merged(pull: &PullRequest) {
    let body = format!("{} #{} {}", pull.repository, pull.number, pull.title);
    if let Err(error) = notify_rust::Notification::new()
        .summary("Pull request merged")
        .body(&body)
        .show()
    {
        eprintln!("failed to notify about merged pull request: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::{DeviceCode, DevicePoll, PullPage};
    use crate::session::Tokens;
    use std::sync::Mutex;

    struct PagedGithub {
        pages: Mutex<Vec<PullPage>>,
        queries: Mutex<Vec<String>>,
    }

    impl Github for PagedGithub {
        fn start_device_flow(&self) -> Result<DeviceCode, GithubError> {
            unreachable!()
        }
        fn poll_device_flow(&self, _: &str) -> Result<DevicePoll, GithubError> {
            unreachable!()
        }
        fn refresh(&self, _: &str) -> Result<Tokens, GithubError> {
            unreachable!()
        }
        fn authored_pulls(
            &self,
            _: &str,
            query: &str,
            _: Option<&str>,
        ) -> Result<PullPage, GithubError> {
            self.queries.lock().unwrap().push(query.into());
            Ok(self.pages.lock().unwrap().remove(0))
        }
        fn close_pull_request(&self, _: &str, _: &str) -> Result<(), GithubError> {
            Ok(())
        }
        fn set_pull_request_draft(&self, _: &str, _: &str, _: bool) -> Result<(), GithubError> {
            Ok(())
        }
        fn oauth_app_restricted(&self, _: &str, _: &str) -> Result<bool, GithubError> {
            Ok(false)
        }
    }

    fn remote(id: &str, state: &str) -> RemotePull {
        RemotePull {
            id: id.into(),
            number: 1,
            title: id.into(),
            state: state.into(),
            url: format!("https://github.com/acme/app/pull/{id}"),
            repository: "acme/app".into(),
            author: "nils".into(),
            updated_at: "2024-01-01T00:00:00Z".into(),
            ci: "none".into(),
            approvals: 0,
            required_approvals: 0,
            has_conflicts: false,
            branch: "feature".into(),
        }
    }

    fn page(pulls: Vec<RemotePull>, has_next_page: bool) -> PullPage {
        PullPage {
            pulls,
            has_next_page,
            end_cursor: has_next_page.then(|| "next".into()),
        }
    }

    #[test]
    fn fetch_walks_pages_sequentially_and_stops_at_the_limit() {
        let github = PagedGithub {
            pages: Mutex::new(vec![
                page(vec![remote("1", "open"), remote("2", "draft")], true),
                page(vec![remote("3", "open")], true),
                page(vec![remote("4", "open")], false),
            ]),
            queries: Mutex::new(Vec::new()),
        };

        let fetched = fetch_authored(&github, "token", "author:@me", 3).unwrap();

        assert_eq!(fetched.pulls.len(), 3);
        assert!(!fetched.complete);
        assert_eq!(github.queries.lock().unwrap().len(), 2);
        assert_eq!(github.pages.lock().unwrap().len(), 1);
    }

    #[test]
    fn apply_writes_opened_and_merged_events() {
        let dir = tempfile_dir();
        std::fs::write(
            dir.join("001.sql"),
            include_str!("../migrations/001_pull_requests.sql"),
        )
        .unwrap();
        std::fs::write(
            dir.join("002.sql"),
            include_str!("../migrations/002_branch.sql"),
        )
        .unwrap();
        let mut store =
            homestead::Store::open(&dir.join("db"), &dir, crate::pulls::PullMutator).unwrap();

        let open = into_pull(remote("1", "open"));
        let first = apply(&mut store, &[open.clone()], true).unwrap();
        assert!(first.is_empty());
        let mut merged = open.clone();
        merged.state = "merged".into();
        merged.updated_at = "2024-02-01T00:00:00Z".into();
        let announced = apply(&mut store, &[merged.clone()], true).unwrap();
        assert_eq!(announced, vec![merged.clone()]);
        let again = apply(&mut store, &[merged], true).unwrap();
        assert!(again.is_empty());

        let rows = store.watch(PullRequest::query()).unwrap().rows();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].state, "merged");
        let mark = store.watch(SyncState::query()).unwrap().rows();
        assert_eq!(mark[0].watermark, "2024-02-01T00:00:00Z");
        let _ = store;
    }

    fn tempfile_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "angry-hub-sync-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
}
