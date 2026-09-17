use std::{thread, time::Duration};

use serde::{Deserialize, Serialize};

const GITHUB_CLIENT_ID: &str = "Ov23li14mBVzqgdBi3HH";

const DEVICE_CODE_URL: &str = "https://github.com/login/device/code";
const ACCESS_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";

#[derive(Serialize)]
struct DeviceCodeRequest {
    client_id: &'static str,
    scope: &'static str,
}

#[derive(Debug)]
pub struct DeviceCode {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    interval: u64,
}

#[derive(Serialize)]
struct AccessTokenRequest {
    client_id: &'static str,
    device_code: String,
    grant_type: &'static str,
}

#[derive(Deserialize)]
struct AccessTokenResponse {
    access_token: Option<String>,
    error: Option<String>,
    #[serde(rename = "error_description")]
    error_description: Option<String>,
}

#[derive(Deserialize, Serialize)]
struct StoredToken {
    access_token: String,
}

pub fn token_path() -> std::path::PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("angry-hub")
        .join("token.json")
}

pub fn load_saved_token() -> Option<String> {
    let path = token_path();
    let contents = std::fs::read_to_string(path).ok()?;
    let stored: StoredToken = serde_json::from_str(&contents).ok()?;
    Some(stored.access_token)
}

pub fn save_token(access_token: &str) {
    let path = token_path();
    let _ = std::fs::create_dir_all(path.parent().unwrap());
    let stored = StoredToken {
        access_token: access_token.to_string(),
    };
    if let Ok(json) = serde_json::to_string(&stored) {
        let _ = std::fs::write(path, json);
    }
}

pub enum LoginState {
    Success,
    Failure(String),
}

pub fn logout() {
    let _ = std::fs::remove_file(token_path());
}

pub fn request_device_code() -> Result<DeviceCode, String> {
    let client = reqwest::blocking::Client::new();
    let resp = client
        .post(DEVICE_CODE_URL)
        .header("Accept", "application/json")
        .json(&DeviceCodeRequest {
            client_id: GITHUB_CLIENT_ID,
            scope: "notifications repo read:user",
        })
        .send()
        .map_err(|e| format!("request failed: {e}"))?;
    let status = resp.status();
    let body = resp
        .text()
        .map_err(|e| format!("failed to read response: {e}"))?;
    if !status.is_success() {
        return Err(format!("device code request failed ({status}): {body}"));
    }
    let resp: DeviceCodeResponse = serde_json::from_str(&body)
        .map_err(|e| format!("invalid response: {e} (body: {body})"))?;
    Ok(DeviceCode {
        device_code: resp.device_code,
        user_code: resp.user_code,
        verification_uri: resp.verification_uri,
        interval: resp.interval.unwrap_or(5).max(5),
    })
}

pub fn poll_for_token(code: &DeviceCode) -> Result<LoginState, String> {
    let client = reqwest::blocking::Client::new();
    loop {
        thread::sleep(Duration::from_secs(code.interval));

        let resp = client
            .post(ACCESS_TOKEN_URL)
            .header("Accept", "application/json")
            .json(&AccessTokenRequest {
                client_id: GITHUB_CLIENT_ID,
                device_code: code.device_code.clone(),
                grant_type: "urn:ietf:params:oauth:grant-type:device_code",
            })
            .send()
            .map_err(|e| format!("request failed: {e}"))?;
        let status = resp.status();
        let body = resp
            .text()
            .map_err(|e| format!("failed to read response: {e}"))?;
        if !status.is_success() {
            return Err(format!("token request failed ({status}): {body}"));
        }
        let resp: AccessTokenResponse = serde_json::from_str(&body)
            .map_err(|e| format!("invalid response: {e} (body: {body})"))?;

        if let Some(token) = resp.access_token {
            save_token(&token);
            return Ok(LoginState::Success);
        }

        match resp.error.as_deref() {
            Some("authorization_pending") => continue,
            Some("slow_down") => {
                // GitHub requires backing off; handled by next iteration's interval
                continue;
            }
            Some("expired_token") => return Err("device code expired".into()),
            Some("access_denied") => return Err("user denied access".into()),
            Some(other) => {
                return Err(resp
                    .error_description
                    .unwrap_or_else(|| other.to_string()))
            }
            None => return Err("unknown error".into()),
        }
    }
}

#[derive(Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    interval: Option<u64>,
}
