#![allow(dead_code)]

use serde::Deserialize;

use crate::session::Tokens;

const DEVICE_CODE_URL: &str = "https://github.com/login/device/code";
const ACCESS_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
const SCOPE: &str = "notifications repo read:user";

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

pub trait Github {
    fn start_device_flow(&self) -> Result<DeviceCode, GithubError>;
    fn poll_device_flow(&self, device_code: &str) -> Result<DevicePoll, GithubError>;
    fn refresh(&self, refresh_token: &str) -> Result<Tokens, GithubError>;
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
}

pub(crate) trait Http {
    fn post_json(&self, url: &str, body: &str) -> Result<String, GithubError>;
}

impl<H: Http + ?Sized> Http for &H {
    fn post_json(&self, url: &str, body: &str) -> Result<String, GithubError> {
        (*self).post_json(url, body)
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
            parsed.error_description.unwrap_or_else(|| other.to_string()),
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
}
