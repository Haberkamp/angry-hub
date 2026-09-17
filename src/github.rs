//! Concrete implementation of the `datasource` traits for GitHub:
//! OAuth device flow + REST search API, backed by a filesystem token store.

use std::{thread, time::Duration};

use serde::{Deserialize, Serialize};

use crate::datasource::{
    AuthStore, AuthSuccess, CodeHost, DataSourceError, DataSourceResult,
};
use crate::model::{DeviceCode, PullRequest, PullRequestState};

const GITHUB_CLIENT_ID: &str = "Ov23li14mBVzqgdBi3HH";

const DEVICE_CODE_URL: &str = "https://github.com/login/device/code";
const ACCESS_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";

pub struct GithubApi {
    client: reqwest::blocking::Client,
    token_store: FileTokenStore,
}

impl GithubApi {
    pub fn new() -> Self {
        Self {
            client: reqwest::blocking::Client::new(),
            token_store: FileTokenStore,
        }
    }

    fn token(&self) -> Option<String> {
        self.token_store.load_token()
    }

    fn bearer(&self) -> DataSourceResult<String> {
        self.token()
            .ok_or_else(|| DataSourceError::new("not logged in"))
    }
}

impl Default for GithubApi {
    fn default() -> Self {
        Self::new()
    }
}

impl CodeHost for GithubApi {
    fn has_saved_session(&self) -> bool {
        self.token().is_some()
    }

    fn start_login(&self) -> DataSourceResult<DeviceCode> {
        #[derive(Serialize)]
        struct Request {
            client_id: &'static str,
            scope: &'static str,
        }
        #[derive(Deserialize)]
        struct Response {
            device_code: String,
            user_code: String,
            verification_uri: String,
            interval: Option<u64>,
        }

        let resp = self
            .client
            .post(DEVICE_CODE_URL)
            .header("Accept", "application/json")
            .json(&Request {
                client_id: GITHUB_CLIENT_ID,
                scope: "notifications repo read:user",
            })
            .send()
            .map_err(|e| DataSourceError::new(format!("request failed: {e}")))?;
        let status = resp.status();
        let body = resp
            .text()
            .map_err(|e| DataSourceError::new(format!("failed to read response: {e}")))?;
        if !status.is_success() {
            return Err(DataSourceError::new(format!(
                "device code request failed ({status}): {body}"
            )));
        }
        let resp: Response = serde_json::from_str(&body)
            .map_err(|e| DataSourceError::new(format!("invalid response: {e}")))?;

        Ok(DeviceCode {
            device_code: resp.device_code,
            user_code: resp.user_code,
            verification_uri: resp.verification_uri,
            poll_interval_secs: resp.interval.unwrap_or(5).max(5),
        })
    }

    fn await_login(&self, code: &DeviceCode) -> DataSourceResult<AuthSuccess> {
        #[derive(Serialize)]
        struct Request {
            client_id: &'static str,
            device_code: String,
            grant_type: &'static str,
        }
        #[derive(Deserialize)]
        struct Response {
            access_token: Option<String>,
            error: Option<String>,
            #[serde(rename = "error_description")]
            error_description: Option<String>,
        }

        loop {
            thread::sleep(Duration::from_secs(code.poll_interval_secs));

            let resp = self
                .client
                .post(ACCESS_TOKEN_URL)
                .header("Accept", "application/json")
                .json(&Request {
                    client_id: GITHUB_CLIENT_ID,
                    device_code: code.device_code.clone(),
                    grant_type: "urn:ietf:params:oauth:grant-type:device_code",
                })
                .send()
                .map_err(|e| DataSourceError::new(format!("request failed: {e}")))?;
            let status = resp.status();
            let body = resp
                .text()
                .map_err(|e| {
                    DataSourceError::new(format!("failed to read response: {e}"))
                })?;
            if !status.is_success() {
                return Err(DataSourceError::new(format!(
                    "token request failed ({status}): {body}"
                )));
            }
            let resp: Response = serde_json::from_str(&body)
                .map_err(|e| DataSourceError::new(format!("invalid response: {e}")))?;

            if let Some(token) = resp.access_token {
                self.token_store.save_token(&token);
                return Ok(AuthSuccess);
            }

            match resp.error.as_deref() {
                Some("authorization_pending") => continue,
                Some("slow_down") => {
                    // GitHub requires backing off; handled by next iteration's interval
                    continue;
                }
                Some("expired_token") => {
                    return Err(DataSourceError::new("device code expired"))
                }
                Some("access_denied") => {
                    return Err(DataSourceError::new("user denied access"))
                }
                Some(other) => {
                    return Err(DataSourceError::new(
                        resp.error_description.unwrap_or_else(|| other.to_string()),
                    ))
                }
                None => return Err(DataSourceError::new("unknown error")),
            }
        }
    }

    fn my_pull_requests(&self) -> DataSourceResult<Vec<PullRequest>> {
        #[derive(Deserialize)]
        struct SearchResponse {
            items: Vec<SearchItem>,
        }
        #[derive(Deserialize)]
        struct SearchItem {
            title: String,
            html_url: String,
            state: String,
            draft: Option<bool>,
            pull_request: Option<PullRequestMarker>,
            repository_url: String,
        }
        #[derive(Deserialize)]
        struct PullRequestMarker {}

        let token = self.bearer()?;
        let resp = self
            .client
            .get("https://api.github.com/search/issues")
            .query(&[
                ("q", "author:@me type:pr"),
                ("per_page", "100"),
                ("sort", "updated"),
                ("order", "desc"),
            ])
            .header("Accept", "application/vnd.github+json")
            .header("Authorization", format!("Bearer {token}"))
            .header("User-Agent", "angry-hub")
            .send()
            .map_err(|e| DataSourceError::new(format!("request failed: {e}")))?;
        let status = resp.status();
        let body = resp
            .text()
            .map_err(|e| DataSourceError::new(format!("failed to read response: {e}")))?;
        if !status.is_success() {
            return Err(DataSourceError::new(format!(
                "search request failed ({status}): {body}"
            )));
        }
        let resp: SearchResponse = serde_json::from_str(&body)
            .map_err(|e| DataSourceError::new(format!("invalid response: {e}")))?;

        Ok(resp
            .items
            .into_iter()
            .map(|item| {
                let is_pr = item.pull_request.is_some();
                PullRequest {
                    title: item.title,
                    repo: repo_name_from_url(&item.repository_url),
                    url: item.html_url,
                    state: if is_pr && item.state == "closed" {
                        PullRequestState::Merged
                    } else if item.state == "open" {
                        PullRequestState::Open
                    } else {
                        PullRequestState::Closed
                    },
                    draft: item.draft.unwrap_or(false),
                }
            })
            .collect())
    }

    fn logout(&self) {
        self.token_store.clear();
    }
}

fn repo_name_from_url(url: &str) -> String {
    let url = url.trim_end_matches('/');
    let segments: Vec<&str> = url.rsplitn(3, '/').collect();
    match segments.as_slice() {
        [name, owner, ..] => format!("{owner}/{name}"),
        _ => url.to_string(),
    }
}

/// Stores the OAuth token as JSON in the user's config directory.
pub struct FileTokenStore;

#[derive(Deserialize, Serialize)]
struct StoredToken {
    access_token: String,
}

fn token_path() -> std::path::PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("angry-hub")
        .join("token.json")
}

impl AuthStore for FileTokenStore {
    fn load_token(&self) -> Option<String> {
        let contents = std::fs::read_to_string(token_path()).ok()?;
        let stored: StoredToken = serde_json::from_str(&contents).ok()?;
        Some(stored.access_token)
    }

    fn save_token(&self, token: &str) {
        let path = token_path();
        let _ = std::fs::create_dir_all(path.parent().unwrap());
        let stored = StoredToken {
            access_token: token.to_string(),
        };
        if let Ok(json) = serde_json::to_string(&stored) {
            let _ = std::fs::write(path, json);
        }
    }

    fn clear(&self) {
        let _ = std::fs::remove_file(token_path());
    }
}