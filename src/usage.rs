mod format;
mod local;

use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    auth::AuthInfo,
    constants::{USAGE_ENDPOINT, USER_AGENT},
};

pub use format::{format_remaining, format_reset, format_reset_from};
pub use local::{LocalRolloutCache, LocalUsage, latest_local_usage};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UsageWindow {
    pub used_percent: f64,
    pub window_minutes: Option<i64>,
    pub reset_at: Option<i64>,
}

impl UsageWindow {
    pub fn remaining_percent(&self) -> i64 {
        (100.0 - self.used_percent).clamp(0.0, 100.0).round() as i64
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UsageSnapshot {
    pub plan: Option<String>,
    pub primary: Option<UsageWindow>,
    pub secondary: Option<UsageWindow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UsageError {
    Http(u16),
    Transport(String),
    Io(String),
    InvalidResponse,
}

impl fmt::Display for UsageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Http(code) => write!(f, "HTTP {code}"),
            Self::Transport(message) => write!(f, "{message}"),
            Self::Io(message) => write!(f, "{message}"),
            Self::InvalidResponse => write!(f, "invalid response"),
        }
    }
}

impl std::error::Error for UsageError {}

pub trait UsageFetcher: Sync {
    fn fetch(&self, auth: &AuthInfo) -> Result<UsageSnapshot, UsageError>;
}

pub struct RemoteUsageFetcher;

impl UsageFetcher for RemoteUsageFetcher {
    fn fetch(&self, auth: &AuthInfo) -> Result<UsageSnapshot, UsageError> {
        let response = ureq::get(USAGE_ENDPOINT)
            .header("Authorization", &format!("Bearer {}", auth.access_token))
            .header("ChatGPT-Account-Id", &auth.account_id)
            .header("User-Agent", USER_AGENT)
            .call()
            .map_err(|err| match err {
                ureq::Error::StatusCode(code) => UsageError::Http(code),
                other => UsageError::Transport(other.to_string()),
            })?;
        let mut body = response.into_body();
        let bytes = body
            .read_to_vec()
            .map_err(|_| UsageError::InvalidResponse)?;
        parse_usage_json(&bytes)
    }
}

pub fn parse_usage_json(body: &[u8]) -> Result<UsageSnapshot, UsageError> {
    let value: Value = serde_json::from_slice(body).map_err(|_| UsageError::InvalidResponse)?;
    parse_usage_value(&value).ok_or(UsageError::InvalidResponse)
}

fn parse_usage_value(root: &Value) -> Option<UsageSnapshot> {
    let rate_limit = root.get("rate_limit")?.as_object()?;
    let primary = rate_limit.get("primary_window").and_then(parse_window);
    let secondary = rate_limit.get("secondary_window").and_then(parse_window);
    if primary.is_none() && secondary.is_none() {
        return None;
    }

    Some(UsageSnapshot {
        plan: root
            .get("plan_type")
            .and_then(Value::as_str)
            .map(str::to_owned),
        primary,
        secondary,
    })
}

pub(super) fn parse_rollout_rate_limits(value: &Value) -> Option<UsageSnapshot> {
    let object = value.as_object()?;
    let primary = object.get("primary").and_then(parse_rollout_window);
    let secondary = object.get("secondary").and_then(parse_rollout_window);
    if primary.is_none() && secondary.is_none() {
        return None;
    }
    Some(UsageSnapshot {
        plan: object
            .get("plan_type")
            .and_then(Value::as_str)
            .map(str::to_owned),
        primary,
        secondary,
    })
}

fn parse_window(value: &Value) -> Option<UsageWindow> {
    let object = value.as_object()?;
    let used_percent = object.get("used_percent")?.as_f64()?;
    let window_minutes = object
        .get("limit_window_seconds")
        .and_then(Value::as_i64)
        .map(|seconds| seconds / 60);
    let reset_at = object.get("reset_at").and_then(Value::as_i64);
    Some(UsageWindow {
        used_percent,
        window_minutes,
        reset_at,
    })
}

fn parse_rollout_window(value: &Value) -> Option<UsageWindow> {
    let object = value.as_object()?;
    Some(UsageWindow {
        used_percent: object.get("used_percent")?.as_f64()?,
        window_minutes: object.get("window_minutes").and_then(Value::as_i64),
        reset_at: object.get("resets_at").and_then(Value::as_i64),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_usage_windows() {
        let snapshot = parse_usage_json(
            br#"{
                "plan_type": "team",
                "rate_limit": {
                    "primary_window": {
                        "used_percent": 11,
                        "limit_window_seconds": 18000,
                        "reset_at": 1773491460
                    },
                    "secondary_window": {
                        "used_percent": 94,
                        "limit_window_seconds": 604800,
                        "reset_at": 1773749620
                    }
                }
            }"#,
        )
        .unwrap();

        assert_eq!(snapshot.plan.as_deref(), Some("team"));
        assert_eq!(snapshot.primary.as_ref().unwrap().remaining_percent(), 89);
        assert_eq!(snapshot.primary.as_ref().unwrap().window_minutes, Some(300));
        assert_eq!(
            snapshot.secondary.as_ref().unwrap().window_minutes,
            Some(10080)
        );
    }

    #[test]
    fn rejects_payload_without_usage_windows() {
        assert_eq!(
            parse_usage_json(br#"{"plan_type":"plus","rate_limit":null}"#),
            Err(UsageError::InvalidResponse)
        );
    }
}
