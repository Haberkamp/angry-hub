//! Concrete implementation of the `datasource` traits for GitHub:
//! OAuth device flow + REST search API, backed by a filesystem token store.

use std::{thread, time::Duration};

use serde::{Deserialize, Serialize};

use crate::datasource::{AuthStore, AuthSuccess, CodeHost, DataSourceError, DataSourceResult};
use crate::model::{ActivityItem, ActivityKind, CiStatus, DeviceCode, PrStatus, PullRequest};
use std::path::PathBuf;

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

    fn viewer_login(&self) -> DataSourceResult<String> {
        #[derive(Deserialize)]
        struct User {
            login: String,
        }

        let token = self.bearer()?;
        let resp = self
            .client
            .get("https://api.github.com/user")
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
                "failed to load user ({status}): {body}"
            )));
        }
        let user: User = serde_json::from_str(&body)
            .map_err(|e| DataSourceError::new(format!("invalid response: {e}")))?;
        Ok(user.login)
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
                .map_err(|e| DataSourceError::new(format!("failed to read response: {e}")))?;
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
                Some("expired_token") => return Err(DataSourceError::new("device code expired")),
                Some("access_denied") => return Err(DataSourceError::new("user denied access")),
                Some(other) => {
                    return Err(DataSourceError::new(
                        resp.error_description.unwrap_or_else(|| other.to_string()),
                    ));
                }
                None => return Err(DataSourceError::new("unknown error")),
            }
        }
    }

    fn my_pull_requests(&self) -> DataSourceResult<Vec<PullRequest>> {
        const QUERY: &str = r#"
            query($perPage: Int!) {
              viewer {
                pullRequests(first: $perPage, states: OPEN, orderBy: { field: UPDATED_AT, direction: DESC }) {
                  nodes {
                    id
                    title
                    number
                    url
                    isDraft
                    updatedAt
                    repository { nameWithOwner }
                    statusCheckRollup { state }
                  }
                }
              }
            }
        "#;

        #[derive(Serialize)]
        struct Request {
            query: &'static str,
            variables: Variables,
        }
        #[derive(Serialize)]
        struct Variables {
            #[serde(rename = "perPage")]
            per_page: u32,
        }
        #[derive(Deserialize)]
        struct Response {
            data: Option<Data>,
            message: Option<String>,
            errors: Option<Vec<GraphQLError>>,
        }
        #[derive(Deserialize)]
        struct GraphQLError {
            message: String,
        }
        #[derive(Deserialize)]
        struct Data {
            viewer: Viewer,
        }
        #[derive(Deserialize)]
        struct Viewer {
            #[serde(rename = "pullRequests")]
            pull_requests: PullRequestConnection,
        }
        #[derive(Deserialize)]
        struct PullRequestConnection {
            nodes: Vec<PullRequestNode>,
        }
        #[derive(Deserialize)]
        struct PullRequestNode {
            id: String,
            title: String,
            number: u32,
            url: String,
            #[serde(rename = "isDraft")]
            is_draft: bool,
            #[serde(rename = "updatedAt")]
            updated_at: String,
            repository: Repository,
            #[serde(rename = "statusCheckRollup")]
            status_check_rollup: Option<StatusCheckRollup>,
        }
        #[derive(Deserialize)]
        struct Repository {
            #[serde(rename = "nameWithOwner")]
            name_with_owner: String,
        }
        #[derive(Deserialize)]
        struct StatusCheckRollup {
            state: Option<String>,
        }

        let token = self.bearer()?;
        let resp = self
            .client
            .post("https://api.github.com/graphql")
            .header("Accept", "application/vnd.github+json")
            .header("Authorization", format!("Bearer {token}"))
            .header("User-Agent", "angry-hub")
            .json(&Request {
                query: QUERY,
                variables: Variables { per_page: 100 },
            })
            .send()
            .map_err(|e| DataSourceError::new(format!("request failed: {e}")))?;
        let status = resp.status();
        let body = resp
            .text()
            .map_err(|e| DataSourceError::new(format!("failed to read response: {e}")))?;
        if !status.is_success() {
            return Err(DataSourceError::new(format!(
                "graphql request failed ({status}): {body}"
            )));
        }
        let resp: Response = serde_json::from_str(&body)
            .map_err(|e| DataSourceError::new(format!("invalid response: {e}")))?;
        if let Some(errors) = &resp.errors {
            let messages: Vec<String> = errors.iter().map(|e| e.message.clone()).collect();
            return Err(DataSourceError::new(messages.join("; ")));
        }
        let nodes = resp
            .data
            .map(|d| d.viewer.pull_requests.nodes)
            .ok_or_else(|| {
                DataSourceError::new(resp.message.unwrap_or_else(|| "empty response".into()))
            })?;

        let prs: Vec<PullRequest> = nodes
            .into_iter()
            .map(|node| {
                let ci = match node
                    .status_check_rollup
                    .as_ref()
                    .and_then(|r| r.state.as_deref())
                {
                    Some("SUCCESS") => CiStatus::Success,
                    Some("FAILURE") | Some("ERROR") => CiStatus::Failure,
                    Some("PENDING") | Some("EXPECTED") => CiStatus::Pending,
                    _ => CiStatus::None,
                };
                PullRequest {
                    id: node.id,
                    title: node.title,
                    repo: node.repository.name_with_owner,
                    number: node.number,
                    url: node.url,
                    status: if node.is_draft {
                        PrStatus::Draft
                    } else {
                        PrStatus::Open
                    },
                    ci,
                    updated_at: node.updated_at,
                }
            })
            .collect();
        PrsCache::save(&prs);
        Ok(prs)
    }

    fn merged_pull_requests(&self, urls: &[String]) -> DataSourceResult<Vec<PullRequest>> {
        if urls.is_empty() {
            return Ok(Vec::new());
        }

        let fields = "title number url merged state repository { nameWithOwner }";
        let mut query = String::from("query {");
        for (i, url) in urls.iter().enumerate() {
            let escaped = url.replace('\\', "\\\\").replace('"', "\\\"");
            query.push_str(&format!(
                " r{i}: resource(url: \"{escaped}\") {{ ... on PullRequest {{ {fields} }} }}"
            ));
        }
        query.push_str(" }");

        #[derive(Serialize)]
        struct Request {
            query: String,
        }
        #[derive(Deserialize)]
        struct Response {
            data: Option<serde_json::Map<String, serde_json::Value>>,
            message: Option<String>,
            errors: Option<Vec<GraphQLError>>,
        }
        #[derive(Deserialize)]
        struct GraphQLError {
            message: String,
        }
        #[derive(Deserialize)]
        struct Resource {
            title: String,
            number: u32,
            url: String,
            merged: bool,
            repository: Repository,
        }
        #[derive(Deserialize)]
        struct Repository {
            #[serde(rename = "nameWithOwner")]
            name_with_owner: String,
        }

        let token = self.bearer()?;
        let resp = self
            .client
            .post("https://api.github.com/graphql")
            .header("Accept", "application/vnd.github+json")
            .header("Authorization", format!("Bearer {token}"))
            .header("User-Agent", "angry-hub")
            .json(&Request { query })
            .send()
            .map_err(|e| DataSourceError::new(format!("request failed: {e}")))?;
        let status = resp.status();
        let body = resp
            .text()
            .map_err(|e| DataSourceError::new(format!("failed to read response: {e}")))?;
        if !status.is_success() {
            return Err(DataSourceError::new(format!(
                "graphql request failed ({status}): {body}"
            )));
        }
        let resp: Response = serde_json::from_str(&body)
            .map_err(|e| DataSourceError::new(format!("invalid response: {e}")))?;
        if let Some(errors) = &resp.errors {
            let messages: Vec<String> = errors.iter().map(|e| e.message.clone()).collect();
            if resp.data.is_none() {
                return Err(DataSourceError::new(messages.join("; ")));
            }
        }
        let data = resp.data.ok_or_else(|| {
            DataSourceError::new(resp.message.unwrap_or_else(|| "empty response".into()))
        })?;

        let mut merged = Vec::new();
        for value in data.values() {
            if value.is_null() {
                continue;
            }
            let Ok(resource) = serde_json::from_value::<Resource>(value.clone()) else {
                continue;
            };
            if resource.merged {
                merged.push(PullRequest {
                    id: String::new(),
                    title: resource.title,
                    repo: resource.repository.name_with_owner,
                    number: resource.number,
                    url: resource.url,
                    status: PrStatus::Merged,
                    ci: CiStatus::None,
                    updated_at: String::new(),
                });
            }
        }
        Ok(merged)
    }

    fn my_pr_activity(&self) -> DataSourceResult<Vec<ActivityItem>> {
        const QUERY: &str = r#"
            query($perPage: Int!) {
              viewer {
                login
                pullRequests(first: $perPage, states: [OPEN, MERGED, CLOSED], orderBy: { field: UPDATED_AT, direction: DESC }) {
                  nodes {
                    title
                    url
                    mergedAt
                    mergedBy {
                      login
                      avatarUrl
                      ... on User { name }
                      ... on Organization { name }
                      ... on Mannequin { name }
                    }
                    repository { nameWithOwner }
                    comments(last: 20) {
                      nodes {
                        author {
                          login
                          avatarUrl
                          ... on User { name }
                          ... on Organization { name }
                          ... on Mannequin { name }
                        }
                        createdAt
                        url
                      }
                    }
                    reviews(last: 20) {
                      nodes {
                        author {
                          login
                          avatarUrl
                          ... on User { name }
                          ... on Organization { name }
                          ... on Mannequin { name }
                        }
                        state
                        submittedAt
                        url
                        comments(last: 10) {
                          nodes {
                            author {
                              login
                              avatarUrl
                              ... on User { name }
                              ... on Organization { name }
                              ... on Mannequin { name }
                            }
                            createdAt
                            url
                          }
                        }
                      }
                    }
                  }
                }
              }
            }
        "#;

        #[derive(Serialize)]
        struct Request {
            query: &'static str,
            variables: Variables,
        }
        #[derive(Serialize)]
        struct Variables {
            #[serde(rename = "perPage")]
            per_page: u32,
        }
        #[derive(Deserialize)]
        struct Response {
            data: Option<Data>,
            message: Option<String>,
            errors: Option<Vec<GraphQLError>>,
        }
        #[derive(Deserialize)]
        struct GraphQLError {
            message: String,
        }
        #[derive(Deserialize)]
        struct Data {
            viewer: Viewer,
        }
        #[derive(Deserialize)]
        struct Viewer {
            login: String,
            #[serde(rename = "pullRequests")]
            pull_requests: PullRequestConnection,
        }
        #[derive(Deserialize)]
        struct PullRequestConnection {
            nodes: Vec<PullRequestNode>,
        }
        #[derive(Deserialize)]
        struct PullRequestNode {
            title: String,
            url: String,
            #[serde(rename = "mergedAt")]
            merged_at: Option<String>,
            #[serde(rename = "mergedBy")]
            merged_by: Option<Actor>,
            repository: Repository,
            comments: CommentConnection,
            reviews: ReviewConnection,
        }
        #[derive(Deserialize)]
        struct Repository {
            #[serde(rename = "nameWithOwner")]
            name_with_owner: String,
        }
        #[derive(Deserialize)]
        struct Actor {
            login: String,
            #[serde(rename = "avatarUrl")]
            avatar_url: String,
            name: Option<String>,
        }

        impl Actor {
            fn display_name(&self) -> String {
                self.name
                    .as_deref()
                    .map(str::trim)
                    .filter(|name| !name.is_empty())
                    .unwrap_or(&self.login)
                    .to_string()
            }
        }
        #[derive(Deserialize)]
        struct CommentConnection {
            nodes: Vec<CommentNode>,
        }
        #[derive(Deserialize)]
        struct CommentNode {
            author: Option<Actor>,
            #[serde(rename = "createdAt")]
            created_at: String,
            url: String,
        }
        #[derive(Deserialize)]
        struct ReviewConnection {
            nodes: Vec<ReviewNode>,
        }
        #[derive(Deserialize)]
        struct ReviewNode {
            author: Option<Actor>,
            state: String,
            #[serde(rename = "submittedAt")]
            submitted_at: Option<String>,
            url: String,
            comments: CommentConnection,
        }

        let token = self.bearer()?;
        let resp = self
            .client
            .post("https://api.github.com/graphql")
            .header("Accept", "application/vnd.github+json")
            .header("Authorization", format!("Bearer {token}"))
            .header("User-Agent", "angry-hub")
            .json(&Request {
                query: QUERY,
                variables: Variables { per_page: 25 },
            })
            .send()
            .map_err(|e| DataSourceError::new(format!("request failed: {e}")))?;
        let status = resp.status();
        let body = resp
            .text()
            .map_err(|e| DataSourceError::new(format!("failed to read response: {e}")))?;
        if !status.is_success() {
            return Err(DataSourceError::new(format!(
                "graphql request failed ({status}): {body}"
            )));
        }
        let resp: Response = serde_json::from_str(&body)
            .map_err(|e| DataSourceError::new(format!("invalid response: {e}")))?;
        if let Some(errors) = &resp.errors {
            let messages: Vec<String> = errors.iter().map(|e| e.message.clone()).collect();
            return Err(DataSourceError::new(messages.join("; ")));
        }
        let viewer = resp.data.map(|d| d.viewer).ok_or_else(|| {
            DataSourceError::new(resp.message.unwrap_or_else(|| "empty response".into()))
        })?;
        let me = viewer.login;

        let mut items = Vec::new();
        for pr in viewer.pull_requests.nodes {
            if let Some(merged_at) = pr.merged_at {
                items.push(ActivityItem {
                    kind: ActivityKind::Merged,
                    actor: pr
                        .merged_by
                        .as_ref()
                        .map(Actor::display_name)
                        .unwrap_or_else(|| "someone".into()),
                    avatar_url: pr.merged_by.as_ref().map(|a| a.avatar_url.clone()),
                    pr_title: pr.title.clone(),
                    repo: pr.repository.name_with_owner.clone(),
                    url: pr.url.clone(),
                    occurred_at: merged_at,
                });
            }

            for comment in pr.comments.nodes {
                let Some(author) = comment.author else {
                    continue;
                };
                if author.login == me {
                    continue;
                }
                items.push(ActivityItem {
                    kind: ActivityKind::Comment,
                    actor: author.display_name(),
                    avatar_url: Some(author.avatar_url),
                    pr_title: pr.title.clone(),
                    repo: pr.repository.name_with_owner.clone(),
                    url: comment.url,
                    occurred_at: comment.created_at,
                });
            }

            for review in pr.reviews.nodes {
                let is_self = review
                    .author
                    .as_ref()
                    .is_some_and(|author| author.login == me);

                if !is_self && let Some(submitted_at) = review.submitted_at.clone() {
                    let kind = match review.state.as_str() {
                        "APPROVED" => Some(ActivityKind::Approved),
                        "CHANGES_REQUESTED" => Some(ActivityKind::ChangesRequested),
                        _ => None,
                    };
                    if let Some(kind) = kind {
                        items.push(ActivityItem {
                            kind,
                            actor: review
                                .author
                                .as_ref()
                                .map(Actor::display_name)
                                .unwrap_or_else(|| "someone".into()),
                            avatar_url: review.author.as_ref().map(|a| a.avatar_url.clone()),
                            pr_title: pr.title.clone(),
                            repo: pr.repository.name_with_owner.clone(),
                            url: review.url.clone(),
                            occurred_at: submitted_at,
                        });
                    }
                }

                for comment in review.comments.nodes {
                    let Some(author) = comment.author else {
                        continue;
                    };
                    if author.login == me {
                        continue;
                    }
                    items.push(ActivityItem {
                        kind: ActivityKind::Comment,
                        actor: author.display_name(),
                        avatar_url: Some(author.avatar_url),
                        pr_title: pr.title.clone(),
                        repo: pr.repository.name_with_owner.clone(),
                        url: comment.url,
                        occurred_at: comment.created_at,
                    });
                }
            }
        }

        items.sort_by(|a, b| b.occurred_at.cmp(&a.occurred_at));
        items.truncate(80);
        Ok(items)
    }

    fn close_pull_request(&self, id: &str) -> DataSourceResult<()> {
        const MUTATION: &str = r#"
            mutation($id: ID!) {
              closePullRequest(input: { pullRequestId: $id }) {
                pullRequest { id state }
              }
            }
        "#;

        #[derive(Serialize)]
        struct Request<'a> {
            query: &'static str,
            variables: Variables<'a>,
        }
        #[derive(Serialize)]
        struct Variables<'a> {
            id: &'a str,
        }
        #[derive(Deserialize)]
        struct Response {
            data: Option<serde_json::Value>,
            message: Option<String>,
            errors: Option<Vec<GraphQLError>>,
        }
        #[derive(Deserialize)]
        struct GraphQLError {
            message: String,
        }

        let token = self.bearer()?;
        let resp = self
            .client
            .post("https://api.github.com/graphql")
            .header("Accept", "application/vnd.github+json")
            .header("Authorization", format!("Bearer {token}"))
            .header("User-Agent", "angry-hub")
            .json(&Request {
                query: MUTATION,
                variables: Variables { id },
            })
            .send()
            .map_err(|e| DataSourceError::new(format!("request failed: {e}")))?;
        let status = resp.status();
        let body = resp
            .text()
            .map_err(|e| DataSourceError::new(format!("failed to read response: {e}")))?;
        if !status.is_success() {
            return Err(DataSourceError::new(format!(
                "graphql request failed ({status}): {body}"
            )));
        }
        let resp: Response = serde_json::from_str(&body)
            .map_err(|e| DataSourceError::new(format!("invalid response: {e}")))?;
        if let Some(errors) = &resp.errors {
            let messages: Vec<String> = errors.iter().map(|e| e.message.clone()).collect();
            return Err(DataSourceError::new(messages.join("; ")));
        }
        if resp.data.is_none() {
            return Err(DataSourceError::new(
                resp.message.unwrap_or_else(|| "empty response".into()),
            ));
        }
        Ok(())
    }

    fn oauth_app_restricted_from_repo(&self, name_with_owner: &str) -> DataSourceResult<bool> {
        let Some((owner, name)) = name_with_owner.split_once('/') else {
            return Ok(false);
        };
        let login = self.viewer_login()?;
        if owner.eq_ignore_ascii_case(&login) {
            return Ok(false);
        }

        let token = self.bearer()?;
        let url =
            format!("https://api.github.com/repos/{owner}/{name}/collaborators/{login}/permission");
        let resp = self
            .client
            .get(&url)
            .header("Accept", "application/vnd.github+json")
            .header("Authorization", format!("Bearer {token}"))
            .header("User-Agent", "angry-hub")
            .send()
            .map_err(|e| DataSourceError::new(format!("request failed: {e}")))?;
        let status = resp.status();
        let body = resp
            .text()
            .map_err(|e| DataSourceError::new(format!("failed to read response: {e}")))?;
        if crate::datasource::is_oauth_app_restricted_message(&body) {
            return Ok(true);
        }
        if status.as_u16() == 403 {
            #[derive(Deserialize)]
            struct ErrorBody {
                message: Option<String>,
            }
            if let Ok(error) = serde_json::from_str::<ErrorBody>(&body)
                && error
                    .message
                    .as_deref()
                    .is_some_and(crate::datasource::is_oauth_app_restricted_message)
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn logout(&self) {
        self.token_store.clear();
        PrsCache::clear();
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

fn prs_cache_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("angry-hub")
        .join("prs.json")
}

pub struct PrsCache;

impl PrsCache {
    pub fn load() -> Option<Vec<PullRequest>> {
        let contents = std::fs::read_to_string(prs_cache_path()).ok()?;
        serde_json::from_str(&contents).ok()
    }

    pub fn save(prs: &[PullRequest]) {
        let path = prs_cache_path();
        let _ = std::fs::create_dir_all(path.parent().unwrap());
        if let Ok(json) = serde_json::to_string(prs) {
            let _ = std::fs::write(path, json);
        }
    }

    pub fn clear() {
        let _ = std::fs::remove_file(prs_cache_path());
    }
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
