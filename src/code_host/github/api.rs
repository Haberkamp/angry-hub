//! GitHub HTTP client: OAuth device flow, GraphQL, REST. No disk cache.

use std::{thread, time::Duration};

use serde::{Deserialize, Serialize};

use crate::auth::TokenStore;
use crate::code_host::{AuthSuccess, DataSourceError, DataSourceResult};
use crate::models::{ActivityItem, ActivityKind, CiStatus, DeviceCode, PrStatus, PullRequest};

const GITHUB_CLIENT_ID: &str = "Ov23li14mBVzqgdBi3HH";

const DEVICE_CODE_URL: &str = "https://github.com/login/device/code";
const ACCESS_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";

pub struct GithubApi {
    client: reqwest::blocking::Client,
    token_store: TokenStore,
}

impl GithubApi {
    pub fn new() -> Self {
        Self {
            client: reqwest::blocking::Client::new(),
            token_store: TokenStore,
        }
    }

    fn bearer(&self) -> DataSourceResult<String> {
        if let Some(token) = self.token_store.access_token() {
            return Ok(token);
        }
        if self.try_refresh_session("") {
            return self
                .token_store
                .access_token()
                .ok_or_else(|| DataSourceError::new("not logged in"));
        }
        Err(DataSourceError::new("not logged in"))
    }

    fn save_tokens(
        &self,
        access_token: String,
        refresh_token: Option<String>,
    ) -> DataSourceResult<()> {
        self.token_store
            .save_tokens(access_token, refresh_token)
            .map_err(DataSourceError::new)
    }

    fn try_refresh_session(&self, stale_access_token: &str) -> bool {
        let _guard = self.token_store.lock_refresh();
        if let Some(current) = self.token_store.access_token()
            && !stale_access_token.is_empty()
            && current != stale_access_token
        {
            return true;
        }
        let Some(refresh_token) = self.token_store.refresh_token() else {
            return false;
        };

        #[derive(Serialize)]
        struct Request {
            client_id: &'static str,
            grant_type: &'static str,
            refresh_token: String,
        }
        #[derive(Deserialize)]
        struct Response {
            access_token: Option<String>,
            refresh_token: Option<String>,
        }

        let Ok(resp) = self
            .client
            .post(ACCESS_TOKEN_URL)
            .header("Accept", "application/json")
            .json(&Request {
                client_id: GITHUB_CLIENT_ID,
                grant_type: "refresh_token",
                refresh_token: refresh_token.clone(),
            })
            .send()
        else {
            return false;
        };
        let Ok(body) = resp.text() else {
            return false;
        };
        let Ok(resp) = serde_json::from_str::<Response>(&body) else {
            return false;
        };
        if let Some(access_token) = resp.access_token.filter(|token| !token.is_empty()) {
            return self
                .save_tokens(
                    access_token,
                    resp.refresh_token
                        .filter(|token| !token.is_empty())
                        .or(Some(refresh_token)),
                )
                .is_ok();
        }
        false
    }

    fn authed_get(&self, url: &str) -> DataSourceResult<(u16, String)> {
        self.authed_send("GET", url, None::<&()>)
    }

    fn authed_post_json(
        &self,
        url: &str,
        json: &impl Serialize,
    ) -> DataSourceResult<(u16, String)> {
        self.authed_send("POST", url, Some(json))
    }

    fn authed_send(
        &self,
        method: &str,
        url: &str,
        json: Option<&impl Serialize>,
    ) -> DataSourceResult<(u16, String)> {
        let mut did_refresh = false;
        loop {
            let token = self.bearer()?;
            let mut request = match method {
                "GET" => self.client.get(url),
                "POST" => self.client.post(url),
                other => {
                    return Err(DataSourceError::new(format!(
                        "unsupported HTTP method: {other}"
                    )));
                }
            };
            request = request
                .header("Accept", "application/vnd.github+json")
                .header("Authorization", format!("Bearer {token}"))
                .header("User-Agent", "angry-hub");
            if let Some(json) = json {
                request = request.json(json);
            }
            let resp = request
                .send()
                .map_err(|e| DataSourceError::new(format!("request failed: {e}")))?;
            let status = resp.status().as_u16();
            let body = resp
                .text()
                .map_err(|e| DataSourceError::new(format!("failed to read response: {e}")))?;
            if is_expired_auth(status, &body) {
                if did_refresh || !self.try_refresh_session(&token) {
                    self.token_store.clear();
                    return Err(DataSourceError::session_ended());
                }
                did_refresh = true;
                continue;
            }
            return Ok((status, body));
        }
    }

    fn graphql(&self, json: &impl Serialize) -> DataSourceResult<String> {
        let (status, body) = self.authed_post_json("https://api.github.com/graphql", json)?;
        if !(200..300).contains(&status) {
            return Err(DataSourceError::new(format!(
                "graphql request failed ({status}): {body}"
            )));
        }
        Ok(body)
    }

    fn viewer_login(&self) -> DataSourceResult<String> {
        #[derive(Deserialize)]
        struct User {
            login: String,
        }

        let (status, body) = self.authed_get("https://api.github.com/user")?;
        if !(200..300).contains(&status) {
            return Err(DataSourceError::new(format!(
                "failed to load user ({status}): {body}"
            )));
        }
        let user: User = serde_json::from_str(&body)
            .map_err(|e| DataSourceError::new(format!("invalid response: {e}")))?;
        Ok(user.login)
    }

    fn fetch_open_pull_requests(&self) -> DataSourceResult<Vec<PullRequest>> {
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
                    latestOpinionatedReviews(first: 40, writersOnly: true) {
                      nodes { state }
                    }
                    reviewDecision
                    mergeable
                    headRefName
                    baseRef {
                      branchProtectionRule {
                        requiresApprovingReviews
                        requiredApprovingReviewCount
                      }
                      refUpdateRule {
                        requiredApprovingReviewCount
                      }
                      rules(first: 50) {
                        nodes {
                          parameters {
                            ... on PullRequestParameters {
                              requiredApprovingReviewCount
                            }
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
            #[serde(rename = "latestOpinionatedReviews")]
            latest_opinionated_reviews: Option<ReviewConnection>,
            #[serde(rename = "reviewDecision")]
            review_decision: Option<String>,
            mergeable: Option<String>,
            #[serde(rename = "headRefName")]
            head_ref_name: Option<String>,
            #[serde(rename = "baseRef")]
            base_ref: Option<BaseRef>,
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
        #[derive(Deserialize)]
        struct ReviewConnection {
            nodes: Vec<ReviewNode>,
        }
        #[derive(Deserialize)]
        struct ReviewNode {
            state: String,
        }
        #[derive(Deserialize)]
        struct BaseRef {
            #[serde(rename = "branchProtectionRule")]
            branch_protection_rule: Option<BranchProtectionRule>,
            #[serde(rename = "refUpdateRule")]
            ref_update_rule: Option<RefUpdateRule>,
            rules: Option<RuleConnection>,
        }
        #[derive(Deserialize)]
        struct BranchProtectionRule {
            #[serde(rename = "requiresApprovingReviews")]
            requires_approving_reviews: bool,
            #[serde(rename = "requiredApprovingReviewCount")]
            required_approving_review_count: Option<u32>,
        }
        #[derive(Deserialize)]
        struct RefUpdateRule {
            #[serde(rename = "requiredApprovingReviewCount")]
            required_approving_review_count: Option<u32>,
        }
        #[derive(Deserialize)]
        struct RuleConnection {
            nodes: Vec<RepositoryRule>,
        }
        #[derive(Deserialize)]
        struct RepositoryRule {
            parameters: Option<PullRequestRuleParameters>,
        }
        #[derive(Deserialize)]
        struct PullRequestRuleParameters {
            #[serde(rename = "requiredApprovingReviewCount")]
            required_approving_review_count: Option<u32>,
        }

        fn required_approvals(base_ref: Option<&BaseRef>, review_decision: Option<&str>) -> u32 {
            let from_rules = base_ref
                .map(|base_ref| {
                    let from_protection = base_ref
                        .branch_protection_rule
                        .as_ref()
                        .filter(|rule| rule.requires_approving_reviews)
                        .and_then(|rule| rule.required_approving_review_count)
                        .unwrap_or(0);
                    let from_update_rule = base_ref
                        .ref_update_rule
                        .as_ref()
                        .and_then(|rule| rule.required_approving_review_count)
                        .unwrap_or(0);
                    let from_rulesets = base_ref
                        .rules
                        .as_ref()
                        .map(|rules| {
                            rules
                                .nodes
                                .iter()
                                .filter_map(|rule| {
                                    rule.parameters
                                        .as_ref()
                                        .and_then(|p| p.required_approving_review_count)
                                })
                                .max()
                                .unwrap_or(0)
                        })
                        .unwrap_or(0);
                    from_protection.max(from_update_rule).max(from_rulesets)
                })
                .unwrap_or(0);
            if from_rules == 0 && review_decision == Some("REVIEW_REQUIRED") {
                1
            } else {
                from_rules
            }
        }

        let body = self.graphql(&Request {
            query: QUERY,
            variables: Variables { per_page: 100 },
        })?;
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
                let approvals = node
                    .latest_opinionated_reviews
                    .as_ref()
                    .map(|reviews| {
                        reviews
                            .nodes
                            .iter()
                            .filter(|review| review.state == "APPROVED")
                            .count() as u32
                    })
                    .unwrap_or(0);
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
                    approvals,
                    required_approvals: required_approvals(
                        node.base_ref.as_ref(),
                        node.review_decision.as_deref(),
                    ),
                    has_conflicts: node.mergeable.as_deref() == Some("CONFLICTING"),
                    branch: node.head_ref_name.unwrap_or_default(),
                    updated_at: node.updated_at,
                }
            })
            .collect();
        Ok(prs)
    }
}

impl Default for GithubApi {
    fn default() -> Self {
        Self::new()
    }
}

impl GithubApi {
    pub(super) fn has_saved_session(&self) -> bool {
        self.token_store.has_credentials()
    }

    pub(super) fn start_login(&self) -> DataSourceResult<DeviceCode> {
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

    pub(super) fn await_login(&self, code: &DeviceCode) -> DataSourceResult<AuthSuccess> {
        #[derive(Serialize)]
        struct Request {
            client_id: &'static str,
            device_code: String,
            grant_type: &'static str,
        }
        #[derive(Deserialize)]
        struct Response {
            access_token: Option<String>,
            refresh_token: Option<String>,
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
                self.save_tokens(token, resp.refresh_token)?;
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

    pub(super) fn my_pull_requests(&self) -> DataSourceResult<Vec<PullRequest>> {
        self.fetch_open_pull_requests()
    }

    pub(super) fn my_pr_activity(&self) -> DataSourceResult<Vec<ActivityItem>> {
        const QUERY: &str = r#"
            query($perPage: Int!) {
              viewer {
                login
                pullRequests(first: $perPage, states: [OPEN, MERGED, CLOSED], orderBy: { field: UPDATED_AT, direction: DESC }) {
                  nodes {
                    title
                    number
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
                    timelineItems(last: 20, itemTypes: [CLOSED_EVENT, REOPENED_EVENT]) {
                      nodes {
                        __typename
                        ... on ClosedEvent {
                          createdAt
                          actor {
                            login
                            avatarUrl
                            ... on User { name }
                            ... on Organization { name }
                            ... on Mannequin { name }
                          }
                        }
                        ... on ReopenedEvent {
                          createdAt
                          actor {
                            login
                            avatarUrl
                            ... on User { name }
                            ... on Organization { name }
                            ... on Mannequin { name }
                          }
                        }
                      }
                    }
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
            number: u32,
            url: String,
            #[serde(rename = "mergedAt")]
            merged_at: Option<String>,
            #[serde(rename = "mergedBy")]
            merged_by: Option<Actor>,
            repository: Repository,
            #[serde(rename = "timelineItems")]
            timeline_items: TimelineConnection,
            comments: CommentConnection,
            reviews: ReviewConnection,
        }
        #[derive(Deserialize)]
        struct TimelineConnection {
            nodes: Vec<TimelineNode>,
        }
        #[derive(Deserialize)]
        struct TimelineNode {
            #[serde(rename = "__typename")]
            typename: String,
            #[serde(rename = "createdAt")]
            created_at: Option<String>,
            actor: Option<Actor>,
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

        let body = self.graphql(&Request {
            query: QUERY,
            variables: Variables { per_page: 25 },
        })?;
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
            if let Some(merged_at) = pr.merged_at.clone() {
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
                    number: pr.number,
                    url: pr.url.clone(),
                    occurred_at: merged_at,
                });
            }

            for event in pr.timeline_items.nodes {
                let Some(occurred_at) = event.created_at else {
                    continue;
                };
                let kind = match event.typename.as_str() {
                    "ClosedEvent" => {
                        if pr.merged_at.as_ref() == Some(&occurred_at) {
                            continue;
                        }
                        ActivityKind::Closed
                    }
                    "ReopenedEvent" => ActivityKind::Reopened,
                    _ => continue,
                };
                items.push(ActivityItem {
                    kind,
                    actor: event
                        .actor
                        .as_ref()
                        .map(Actor::display_name)
                        .unwrap_or_else(|| "someone".into()),
                    avatar_url: event.actor.as_ref().map(|a| a.avatar_url.clone()),
                    pr_title: pr.title.clone(),
                    repo: pr.repository.name_with_owner.clone(),
                    number: pr.number,
                    url: pr.url.clone(),
                    occurred_at,
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
                    number: pr.number,
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
                            number: pr.number,
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
                        number: pr.number,
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

    pub(super) fn close_pull_request(&self, id: &str) -> DataSourceResult<()> {
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

        let body = self.graphql(&Request {
            query: MUTATION,
            variables: Variables { id },
        })?;
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

    pub(super) fn oauth_app_restricted_from_repo(
        &self,
        name_with_owner: &str,
    ) -> DataSourceResult<bool> {
        let Some((owner, name)) = name_with_owner.split_once('/') else {
            return Ok(false);
        };
        let login = self.viewer_login()?;
        if owner.eq_ignore_ascii_case(&login) {
            return Ok(false);
        }

        let url =
            format!("https://api.github.com/repos/{owner}/{name}/collaborators/{login}/permission");
        let (status, body) = self.authed_get(&url)?;
        if crate::code_host::is_oauth_app_restricted_message(&body) {
            return Ok(true);
        }
        if status == 403 {
            #[derive(Deserialize)]
            struct ErrorBody {
                message: Option<String>,
            }
            if let Ok(error) = serde_json::from_str::<ErrorBody>(&body)
                && error
                    .message
                    .as_deref()
                    .is_some_and(crate::code_host::is_oauth_app_restricted_message)
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub(super) fn logout(&self) {
        self.token_store.clear();
    }
}

fn is_expired_auth(status: u16, body: &str) -> bool {
    if status == 401 {
        return true;
    }
    let lower = body.to_ascii_lowercase();
    lower.contains("bad credentials")
        || lower.contains("session ended")
        || lower.contains("session has expired")
        || lower.contains("session has been invalidated")
        || lower.contains("this api session has expired")
        || lower.contains("token expired")
        || lower.contains("token has expired")
        || lower.contains("token has been revoked")
        || lower.contains("\"type\":\"forbidden\"") && lower.contains("requires authentication")
}
