mod api;
mod cache;

use crate::code_host::{AuthSuccess, CodeHost, DataSourceResult};
use crate::models::{ActivityItem, DeviceCode, PullRequest};

use api::GithubApi;
use cache::{ActivityCache, PrsCache};

pub struct GithubHost {
    api: GithubApi,
}

impl GithubHost {
    pub fn new() -> Self {
        Self {
            api: GithubApi::new(),
        }
    }
}

impl CodeHost for GithubHost {
    fn has_saved_session(&self) -> bool {
        self.api.has_saved_session()
    }

    fn start_login(&self) -> DataSourceResult<DeviceCode> {
        self.api.start_login()
    }

    fn await_login(&self, code: &DeviceCode) -> DataSourceResult<AuthSuccess> {
        self.api.await_login(code)
    }

    fn pull_request_snapshot(&self) -> Option<Vec<PullRequest>> {
        PrsCache::load().filter(|prs| !prs.is_empty())
    }

    fn my_pull_requests(&self) -> DataSourceResult<Vec<PullRequest>> {
        match self.api.my_pull_requests() {
            Ok(prs) => {
                PrsCache::save(&prs);
                Ok(prs)
            }
            Err(error) if error.is_session_ended() => Err(error),
            Err(error) => PrsCache::load().filter(|prs| !prs.is_empty()).ok_or(error),
        }
    }

    fn close_pull_request(&self, id: &str) -> DataSourceResult<()> {
        self.api.close_pull_request(id)
    }

    fn set_pull_request_draft(&self, id: &str, draft: bool) -> DataSourceResult<()> {
        self.api.set_pull_request_draft(id, draft)
    }

    fn oauth_app_restricted_from_repo(&self, name_with_owner: &str) -> DataSourceResult<bool> {
        self.api.oauth_app_restricted_from_repo(name_with_owner)
    }

    fn my_pr_activity(&self) -> DataSourceResult<Vec<ActivityItem>> {
        match self.api.my_pr_activity() {
            Ok(items) => {
                ActivityCache::save(&items);
                Ok(items)
            }
            Err(error) if error.is_session_ended() => Err(error),
            Err(error) => ActivityCache::load()
                .filter(|items| !items.is_empty())
                .ok_or(error),
        }
    }

    fn activity_snapshot(&self) -> Option<Vec<ActivityItem>> {
        ActivityCache::load().filter(|items| !items.is_empty())
    }

    fn logout(&self) {
        self.api.logout();
        PrsCache::clear();
        ActivityCache::clear();
    }
}
