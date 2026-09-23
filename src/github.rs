#![allow(dead_code)]

use serde::Deserialize;

use crate::session::Tokens;

pub const GITHUB_CLIENT_ID: &str = "Ov23li14mBVzqgdBi3HH";

const DEVICE_CODE_URL: &str = "https://github.com/login/device/code";
const ACCESS_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
const GRAPHQL_URL: &str = "https://api.github.com/graphql";
const SCOPE: &str = "notifications repo read:user";

const AUTHORED_PULLS: &str = r#"
query AuthoredPulls($query: String!, $cursor: String) {
  search(query: $query, type: ISSUE, first: 100, after: $cursor) {
    pageInfo { hasNextPage endCursor }
    nodes {
      ... on PullRequest {
        id
        number
        title
        state
        isDraft
        url
        updatedAt
        headRefName
        author { login }
        repository { nameWithOwner }
        mergeable
        reviewDecision
        statusCheckRollup { state }
        latestOpinionatedReviews(first: 40, writersOnly: true) {
          nodes { state }
        }
        baseRef {
          branchProtectionRule {
            requiresApprovingReviews
            requiredApprovingReviewCount
          }
          refUpdateRule { requiredApprovingReviewCount }
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GithubError {
    message: String,
}

impl GithubError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn is_oauth_app_restricted(&self) -> bool {
        is_oauth_app_restricted_message(&self.message)
    }

    pub fn is_session_ended(&self) -> bool {
        self.message.contains("Bad credentials") || self.message.contains("(401)")
    }
}

pub fn is_oauth_app_restricted_message(message: &str) -> bool {
    message.contains("OAuth App access restrictions")
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceCode {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub interval_secs: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DevicePoll {
    Pending,
    SlowDown,
    Approved(Tokens),
    Denied,
    Expired,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemotePull {
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PullPage {
    pub pulls: Vec<RemotePull>,
    pub has_next_page: bool,
    pub end_cursor: Option<String>,
}

pub trait Github {
    fn start_device_flow(&self) -> Result<DeviceCode, GithubError>;
    fn poll_device_flow(&self, device_code: &str) -> Result<DevicePoll, GithubError>;
    fn refresh(&self, refresh_token: &str) -> Result<Tokens, GithubError>;
    fn authored_pulls(
        &self,
        token: &str,
        query: &str,
        cursor: Option<&str>,
    ) -> Result<PullPage, GithubError>;
    fn close_pull_request(&self, token: &str, id: &str) -> Result<(), GithubError>;
    fn set_pull_request_draft(&self, token: &str, id: &str, draft: bool)
    -> Result<(), GithubError>;
    fn oauth_app_restricted(&self, token: &str, name_with_owner: &str)
    -> Result<bool, GithubError>;
}

pub struct FakeGithub {
    device_code: DeviceCode,
    poll: DevicePoll,
    refreshed: Tokens,
}

impl FakeGithub {
    pub fn with_refresh(tokens: Tokens) -> Self {
        Self {
            device_code: DeviceCode {
                device_code: "device".into(),
                user_code: "ABCD-1234".into(),
                verification_uri: "https://github.com/login/device".into(),
                interval_secs: 5,
            },
            poll: DevicePoll::Pending,
            refreshed: tokens,
        }
    }
}

impl Github for FakeGithub {
    fn start_device_flow(&self) -> Result<DeviceCode, GithubError> {
        Ok(self.device_code.clone())
    }

    fn poll_device_flow(&self, _device_code: &str) -> Result<DevicePoll, GithubError> {
        Ok(self.poll.clone())
    }

    fn refresh(&self, _refresh_token: &str) -> Result<Tokens, GithubError> {
        Ok(self.refreshed.clone())
    }

    fn authored_pulls(
        &self,
        _token: &str,
        _query: &str,
        _cursor: Option<&str>,
    ) -> Result<PullPage, GithubError> {
        Ok(PullPage {
            pulls: Vec::new(),
            has_next_page: false,
            end_cursor: None,
        })
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

pub(crate) trait Http {
    fn post_json(&self, url: &str, body: &str) -> Result<String, GithubError>;
    fn post_bearer(&self, url: &str, token: &str, body: &str) -> Result<String, GithubError>;
    fn get_bearer(&self, url: &str, token: &str) -> Result<(u16, String), GithubError>;
}

impl<H: Http + ?Sized> Http for &H {
    fn post_json(&self, url: &str, body: &str) -> Result<String, GithubError> {
        (*self).post_json(url, body)
    }

    fn post_bearer(&self, url: &str, token: &str, body: &str) -> Result<String, GithubError> {
        (*self).post_bearer(url, token, body)
    }

    fn get_bearer(&self, url: &str, token: &str) -> Result<(u16, String), GithubError> {
        (*self).get_bearer(url, token)
    }
}

pub(crate) struct GithubClient<H> {
    client_id: String,
    http: H,
}

impl<H: Http> GithubClient<H> {
    pub fn new(client_id: impl Into<String>, http: H) -> Self {
        Self {
            client_id: client_id.into(),
            http,
        }
    }
}

impl<H: Http> Github for GithubClient<H> {
    fn start_device_flow(&self) -> Result<DeviceCode, GithubError> {
        let body = serde_json::json!({
            "client_id": self.client_id,
            "scope": SCOPE,
        });
        let response = self.http.post_json(DEVICE_CODE_URL, &body.to_string())?;
        let parsed: DeviceCodeResponse = serde_json::from_str(&response)
            .map_err(|error| GithubError::new(format!("invalid device code response: {error}")))?;
        Ok(DeviceCode {
            device_code: parsed.device_code,
            user_code: parsed.user_code,
            verification_uri: parsed.verification_uri,
            interval_secs: parsed.interval.unwrap_or(5).max(5),
        })
    }

    fn poll_device_flow(&self, device_code: &str) -> Result<DevicePoll, GithubError> {
        let body = serde_json::json!({
            "client_id": self.client_id,
            "device_code": device_code,
            "grant_type": "urn:ietf:params:oauth:grant-type:device_code",
        });
        let response = self.http.post_json(ACCESS_TOKEN_URL, &body.to_string())?;
        parse_token_response(&response).map(|tokens| match tokens {
            TokenResponse::Tokens(tokens) => DevicePoll::Approved(tokens),
            TokenResponse::Error(error) => error,
        })
    }

    fn refresh(&self, refresh_token: &str) -> Result<Tokens, GithubError> {
        let body = serde_json::json!({
            "client_id": self.client_id,
            "grant_type": "refresh_token",
            "refresh_token": refresh_token,
        });
        let response = self.http.post_json(ACCESS_TOKEN_URL, &body.to_string())?;
        match parse_token_response(&response)? {
            TokenResponse::Tokens(tokens) => Ok(tokens),
            TokenResponse::Error(DevicePoll::Pending) => {
                Err(GithubError::new("authorization_pending"))
            }
            TokenResponse::Error(DevicePoll::SlowDown) => Err(GithubError::new("slow_down")),
            TokenResponse::Error(DevicePoll::Denied) => Err(GithubError::new("access_denied")),
            TokenResponse::Error(DevicePoll::Expired) => Err(GithubError::new("expired_token")),
            TokenResponse::Error(DevicePoll::Approved(_)) => {
                Err(GithubError::new("unexpected token response"))
            }
        }
    }

    fn authored_pulls(
        &self,
        token: &str,
        query: &str,
        cursor: Option<&str>,
    ) -> Result<PullPage, GithubError> {
        let body = serde_json::json!({
            "query": AUTHORED_PULLS,
            "variables": {
                "query": query,
                "cursor": cursor,
            },
        });
        let response = self
            .http
            .post_bearer(GRAPHQL_URL, token, &body.to_string())?;
        parse_pull_page(&response)
    }

    fn close_pull_request(&self, token: &str, id: &str) -> Result<(), GithubError> {
        const MUTATION: &str = r#"
            mutation($id: ID!) {
              closePullRequest(input: { pullRequestId: $id }) {
                pullRequest { id state }
              }
            }
        "#;
        self.mutate(token, MUTATION, id)
    }

    fn set_pull_request_draft(
        &self,
        token: &str,
        id: &str,
        draft: bool,
    ) -> Result<(), GithubError> {
        const TO_DRAFT: &str = r#"
            mutation($id: ID!) {
              convertPullRequestToDraft(input: { pullRequestId: $id }) {
                pullRequest { id isDraft }
              }
            }
        "#;
        const READY: &str = r#"
            mutation($id: ID!) {
              markPullRequestReadyForReview(input: { pullRequestId: $id }) {
                pullRequest { id isDraft }
              }
            }
        "#;
        self.mutate(token, if draft { TO_DRAFT } else { READY }, id)
    }

    fn oauth_app_restricted(
        &self,
        token: &str,
        name_with_owner: &str,
    ) -> Result<bool, GithubError> {
        let Some((owner, name)) = name_with_owner.split_once('/') else {
            return Ok(false);
        };
        let login = self.viewer_login(token)?;
        if owner.eq_ignore_ascii_case(&login) {
            return Ok(false);
        }
        let url =
            format!("https://api.github.com/repos/{owner}/{name}/collaborators/{login}/permission");
        let (status, body) = self.http.get_bearer(&url, token)?;
        Ok(status == 403 && is_oauth_app_restricted_message(&body)
            || is_oauth_app_restricted_message(&body))
    }
}

impl<H: Http> GithubClient<H> {
    fn mutate(&self, token: &str, mutation: &str, id: &str) -> Result<(), GithubError> {
        let body = serde_json::json!({
            "query": mutation,
            "variables": { "id": id },
        });
        let response = self
            .http
            .post_bearer(GRAPHQL_URL, token, &body.to_string())?;
        parse_mutation(&response)
    }

    fn viewer_login(&self, token: &str) -> Result<String, GithubError> {
        let body = serde_json::json!({
            "query": "query { viewer { login } }",
        });
        let response = self
            .http
            .post_bearer(GRAPHQL_URL, token, &body.to_string())?;
        #[derive(Deserialize)]
        struct Body {
            data: Option<ViewerData>,
            errors: Option<Vec<GraphqlError>>,
        }
        #[derive(Deserialize)]
        struct ViewerData {
            viewer: Option<AuthorNode>,
        }
        let parsed: Body = serde_json::from_str(&response)
            .map_err(|error| GithubError::new(format!("invalid viewer response: {error}")))?;
        if let Some(errors) = parsed.errors.filter(|errors| !errors.is_empty()) {
            return Err(GithubError::new(
                errors
                    .into_iter()
                    .map(|error| error.message)
                    .collect::<Vec<_>>()
                    .join("; "),
            ));
        }
        parsed
            .data
            .and_then(|data| data.viewer)
            .map(|viewer| viewer.login)
            .filter(|login| !login.is_empty())
            .ok_or_else(|| GithubError::new("viewer response had no login"))
    }
}

enum TokenResponse {
    Tokens(Tokens),
    Error(DevicePoll),
}

fn parse_token_response(body: &str) -> Result<TokenResponse, GithubError> {
    let parsed: TokenBody = serde_json::from_str(body)
        .map_err(|error| GithubError::new(format!("invalid token response: {error}")))?;
    if let Some(access_token) = parsed.access_token.filter(|token| !token.is_empty()) {
        return Ok(TokenResponse::Tokens(Tokens {
            access_token,
            refresh_token: parsed.refresh_token.filter(|token| !token.is_empty()),
        }));
    }
    match parsed.error.as_deref() {
        Some("authorization_pending") => Ok(TokenResponse::Error(DevicePoll::Pending)),
        Some("slow_down") => Ok(TokenResponse::Error(DevicePoll::SlowDown)),
        Some("access_denied") => Ok(TokenResponse::Error(DevicePoll::Denied)),
        Some("expired_token") => Ok(TokenResponse::Error(DevicePoll::Expired)),
        Some(other) => Err(GithubError::new(
            parsed
                .error_description
                .unwrap_or_else(|| other.to_string()),
        )),
        None => Err(GithubError::new("token response had no access token")),
    }
}

#[derive(Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    interval: Option<u64>,
}

#[derive(Deserialize)]
struct TokenBody {
    access_token: Option<String>,
    refresh_token: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

pub(crate) struct ReqwestHttp;

fn parse_pull_page(body: &str) -> Result<PullPage, GithubError> {
    let parsed: GraphqlBody = serde_json::from_str(body)
        .map_err(|error| GithubError::new(format!("invalid pull request response: {error}")))?;
    if let Some(errors) = parsed.errors.filter(|errors| !errors.is_empty()) {
        let message = errors
            .into_iter()
            .map(|error| error.message)
            .collect::<Vec<_>>()
            .join("; ");
        return Err(GithubError::new(message));
    }
    let Some(search) = parsed.data.and_then(|data| data.search) else {
        return Err(GithubError::new("pull request response had no data"));
    };
    let pulls = search
        .nodes
        .into_iter()
        .flatten()
        .map(remote_pull)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(PullPage {
        pulls,
        has_next_page: search.page_info.has_next_page,
        end_cursor: search
            .page_info
            .end_cursor
            .filter(|cursor| !cursor.is_empty()),
    })
}

fn remote_pull(node: PullNode) -> Result<RemotePull, GithubError> {
    let state = match (node.state.as_str(), node.is_draft) {
        ("MERGED", _) => "merged",
        ("CLOSED", _) => "closed",
        ("OPEN", true) => "draft",
        ("OPEN", false) => "open",
        (other, _) => {
            return Err(GithubError::new(format!(
                "unknown pull request state: {other}"
            )));
        }
    };
    Ok(RemotePull {
        id: node.id,
        number: node.number,
        title: node.title,
        state: state.into(),
        url: node.url,
        repository: node
            .repository
            .map(|repo| repo.name_with_owner)
            .unwrap_or_default(),
        author: node.author.map(|author| author.login).unwrap_or_default(),
        updated_at: node.updated_at,
        ci: ci_status(
            node.status_check_rollup
                .as_ref()
                .and_then(|rollup| rollup.state.as_deref()),
        ),
        approvals: node
            .latest_opinionated_reviews
            .as_ref()
            .map(|reviews| {
                reviews
                    .nodes
                    .iter()
                    .flatten()
                    .filter(|review| review.state == "APPROVED")
                    .count() as i64
            })
            .unwrap_or(0),
        required_approvals: required_approvals(
            node.base_ref.as_ref(),
            node.review_decision.as_deref(),
        ),
        has_conflicts: node.mergeable.as_deref() == Some("CONFLICTING"),
        branch: node.head_ref_name,
    })
}

fn parse_mutation(body: &str) -> Result<(), GithubError> {
    #[derive(Deserialize)]
    struct Response {
        errors: Option<Vec<GraphqlError>>,
        message: Option<String>,
    }
    let parsed: Response = serde_json::from_str(body)
        .map_err(|error| GithubError::new(format!("invalid mutation response: {error}")))?;
    if let Some(message) = parsed.message.filter(|message| !message.is_empty()) {
        return Err(GithubError::new(message));
    }
    if let Some(errors) = parsed.errors.filter(|errors| !errors.is_empty()) {
        return Err(GithubError::new(
            errors
                .into_iter()
                .map(|error| error.message)
                .collect::<Vec<_>>()
                .join("; "),
        ));
    }
    Ok(())
}

fn ci_status(state: Option<&str>) -> String {
    match state {
        Some("SUCCESS") => "success",
        Some("FAILURE") | Some("ERROR") => "failure",
        Some("PENDING") | Some("EXPECTED") => "pending",
        _ => "none",
    }
    .into()
}

fn required_approvals(base_ref: Option<&BaseRef>, review_decision: Option<&str>) -> i64 {
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
                        .flatten()
                        .filter_map(|rule| {
                            rule.parameters
                                .as_ref()
                                .and_then(|parameters| parameters.required_approving_review_count)
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

#[derive(Deserialize)]
struct GraphqlBody {
    data: Option<GraphqlData>,
    errors: Option<Vec<GraphqlError>>,
}

#[derive(Deserialize)]
struct GraphqlData {
    search: Option<SearchData>,
}

#[derive(Deserialize)]
struct GraphqlError {
    message: String,
}

#[derive(Deserialize)]
struct SearchData {
    #[serde(rename = "pageInfo")]
    page_info: PageInfo,
    nodes: Vec<Option<PullNode>>,
}

#[derive(Deserialize)]
struct PageInfo {
    #[serde(rename = "hasNextPage")]
    has_next_page: bool,
    #[serde(rename = "endCursor")]
    end_cursor: Option<String>,
}

#[derive(Deserialize)]
struct PullNode {
    id: String,
    number: i64,
    title: String,
    state: String,
    #[serde(rename = "isDraft")]
    is_draft: bool,
    url: String,
    #[serde(rename = "updatedAt")]
    updated_at: String,
    #[serde(default, rename = "headRefName")]
    head_ref_name: String,
    #[serde(default)]
    author: Option<AuthorNode>,
    #[serde(default)]
    repository: Option<RepositoryNode>,
    #[serde(default)]
    mergeable: Option<String>,
    #[serde(default, rename = "reviewDecision")]
    review_decision: Option<String>,
    #[serde(default, rename = "statusCheckRollup")]
    status_check_rollup: Option<StatusCheckRollup>,
    #[serde(default, rename = "latestOpinionatedReviews")]
    latest_opinionated_reviews: Option<ReviewConnection>,
    #[serde(default, rename = "baseRef")]
    base_ref: Option<BaseRef>,
}

#[derive(Deserialize)]
struct StatusCheckRollup {
    state: Option<String>,
}

#[derive(Deserialize)]
struct ReviewConnection {
    nodes: Vec<Option<ReviewNode>>,
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
    required_approving_review_count: Option<i64>,
}

#[derive(Deserialize)]
struct RefUpdateRule {
    #[serde(rename = "requiredApprovingReviewCount")]
    required_approving_review_count: Option<i64>,
}

#[derive(Deserialize)]
struct RuleConnection {
    nodes: Vec<Option<RepositoryRule>>,
}

#[derive(Deserialize)]
struct RepositoryRule {
    parameters: Option<PullRequestRuleParameters>,
}

#[derive(Deserialize)]
struct PullRequestRuleParameters {
    #[serde(rename = "requiredApprovingReviewCount")]
    required_approving_review_count: Option<i64>,
}

#[derive(Deserialize)]
struct AuthorNode {
    login: String,
}

#[derive(Deserialize)]
struct RepositoryNode {
    #[serde(rename = "nameWithOwner")]
    name_with_owner: String,
}

impl Http for ReqwestHttp {
    fn post_json(&self, url: &str, body: &str) -> Result<String, GithubError> {
        let response = reqwest::blocking::Client::new()
            .post(url)
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .body(body.to_string())
            .send()
            .map_err(|error| GithubError::new(format!("request failed: {error}")))?;
        let status = response.status();
        let text = response
            .text()
            .map_err(|error| GithubError::new(format!("failed to read response: {error}")))?;
        if !status.is_success() {
            return Err(GithubError::new(format!(
                "request failed ({status}): {text}"
            )));
        }
        Ok(text)
    }

    fn post_bearer(&self, url: &str, token: &str, body: &str) -> Result<String, GithubError> {
        let response = reqwest::blocking::Client::new()
            .post(url)
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {token}"))
            .header("User-Agent", "angry-hub")
            .body(body.to_string())
            .send()
            .map_err(|error| GithubError::new(format!("request failed: {error}")))?;
        let status = response.status();
        let text = response
            .text()
            .map_err(|error| GithubError::new(format!("failed to read response: {error}")))?;
        if !status.is_success() {
            return Err(GithubError::new(format!(
                "request failed ({status}): {text}"
            )));
        }
        Ok(text)
    }

    fn get_bearer(&self, url: &str, token: &str) -> Result<(u16, String), GithubError> {
        let response = reqwest::blocking::Client::new()
            .get(url)
            .header("Accept", "application/vnd.github+json")
            .header("Authorization", format!("Bearer {token}"))
            .header("User-Agent", "angry-hub")
            .send()
            .map_err(|error| GithubError::new(format!("request failed: {error}")))?;
        let status = response.status().as_u16();
        let text = response
            .text()
            .map_err(|error| GithubError::new(format!("failed to read response: {error}")))?;
        if status >= 400 && status != 403 {
            return Err(GithubError::new(format!(
                "request failed ({status}): {text}"
            )));
        }
        Ok((status, text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockHttp {
        url: std::sync::Mutex<String>,
        body: std::sync::Mutex<String>,
        response: String,
    }

    impl MockHttp {
        fn new(response: &str) -> Self {
            Self {
                url: std::sync::Mutex::new(String::new()),
                body: std::sync::Mutex::new(String::new()),
                response: response.into(),
            }
        }
    }

    impl Http for MockHttp {
        fn post_json(&self, url: &str, body: &str) -> Result<String, GithubError> {
            *self.url.lock().unwrap() = url.into();
            *self.body.lock().unwrap() = body.into();
            Ok(self.response.clone())
        }

        fn post_bearer(&self, url: &str, _token: &str, body: &str) -> Result<String, GithubError> {
            self.post_json(url, body)
        }

        fn get_bearer(&self, url: &str, _token: &str) -> Result<(u16, String), GithubError> {
            *self.url.lock().unwrap() = url.into();
            Ok((200, self.response.clone()))
        }
    }

    #[test]
    fn start_device_flow_parses_the_device_code() {
        let http = MockHttp::new(
            r#"{"device_code":"device","user_code":"ABCD-1234","verification_uri":"https://github.com/login/device","interval":5}"#,
        );
        let client = GithubClient::new("client-id", &http);

        let code = client.start_device_flow().unwrap();

        assert_eq!(*http.url.lock().unwrap(), DEVICE_CODE_URL);
        assert!(http.body.lock().unwrap().contains("client-id"));
        assert_eq!(
            code,
            DeviceCode {
                device_code: "device".into(),
                user_code: "ABCD-1234".into(),
                verification_uri: "https://github.com/login/device".into(),
                interval_secs: 5,
            }
        );
    }

    #[test]
    fn poll_parses_an_access_token_and_refresh_token() {
        let http = MockHttp::new(
            r#"{"access_token":"access","refresh_token":"refresh","token_type":"bearer"}"#,
        );
        let client = GithubClient::new("client-id", &http);

        let poll = client.poll_device_flow("device").unwrap();

        assert_eq!(*http.url.lock().unwrap(), ACCESS_TOKEN_URL);
        assert_eq!(
            poll,
            DevicePoll::Approved(Tokens {
                access_token: "access".into(),
                refresh_token: Some("refresh".into()),
            })
        );
    }

    #[test]
    fn poll_parses_pending_slow_down_denied_and_expired() {
        let cases = [
            ("authorization_pending", DevicePoll::Pending),
            ("slow_down", DevicePoll::SlowDown),
            ("access_denied", DevicePoll::Denied),
            ("expired_token", DevicePoll::Expired),
        ];
        for (error, expected) in cases {
            let http = MockHttp::new(&format!(r#"{{"error":"{error}"}}"#));
            let client = GithubClient::new("client-id", &http);
            assert_eq!(client.poll_device_flow("device").unwrap(), expected);
        }
    }

    #[test]
    fn refresh_parses_the_rotated_tokens() {
        let http = MockHttp::new(
            r#"{"access_token":"access-2","refresh_token":"refresh-2","token_type":"bearer"}"#,
        );
        let client = GithubClient::new("client-id", &http);

        let tokens = client.refresh("refresh").unwrap();

        assert!(http.body.lock().unwrap().contains("refresh_token"));
        assert_eq!(
            tokens,
            Tokens {
                access_token: "access-2".into(),
                refresh_token: Some("refresh-2".into()),
            }
        );
    }

    #[test]
    fn authored_pulls_maps_open_draft_closed_and_merged() {
        let http = MockHttp::new(
            r#"{"data":{"search":{"pageInfo":{"hasNextPage":true,"endCursor":"cursor"},"nodes":[
                {"id":"1","number":1,"title":"Open","state":"OPEN","isDraft":false,"url":"https://github.com/acme/app/pull/1","updatedAt":"2024-01-01T00:00:00Z","author":{"login":"nils"},"repository":{"nameWithOwner":"acme/app"}},
                {"id":"2","number":2,"title":"Draft","state":"OPEN","isDraft":true,"url":"https://github.com/acme/app/pull/2","updatedAt":"2024-01-02T00:00:00Z","author":{"login":"nils"},"repository":{"nameWithOwner":"acme/app"}},
                {"id":"3","number":3,"title":"Closed","state":"CLOSED","isDraft":false,"url":"https://github.com/acme/app/pull/3","updatedAt":"2024-01-03T00:00:00Z","author":{"login":"nils"},"repository":{"nameWithOwner":"acme/app"}},
                {"id":"4","number":4,"title":"Merged","state":"MERGED","isDraft":false,"url":"https://github.com/acme/app/pull/4","updatedAt":"2024-01-04T00:00:00Z","author":{"login":"nils"},"repository":{"nameWithOwner":"acme/app"}}
            ]}}}"#,
        );
        let client = GithubClient::new("client-id", &http);

        let page = client
            .authored_pulls("token", "author:@me is:pr is:open", None)
            .unwrap();

        assert_eq!(*http.url.lock().unwrap(), GRAPHQL_URL);
        assert!(
            http.body
                .lock()
                .unwrap()
                .contains("author:@me is:pr is:open")
        );
        assert!(page.has_next_page);
        assert_eq!(page.end_cursor.as_deref(), Some("cursor"));
        assert_eq!(
            page.pulls
                .iter()
                .map(|pr| pr.state.as_str())
                .collect::<Vec<_>>(),
            vec!["open", "draft", "closed", "merged"]
        );
    }
}
