use std::{collections::HashMap, fs};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde_json::json;
use tempfile::TempDir;

use crate::{
    auth::{self, AuthInfo},
    storage,
    usage::{self, UsageFetcher, UsageSnapshot},
};

fn jwt(payload: serde_json::Value) -> String {
    let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"none","typ":"JWT"}"#);
    let payload = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&payload).unwrap());
    format!("{header}.{payload}.sig")
}

fn auth_bytes(email: &str, user_id: &str, account_id: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "tokens": {
            "access_token": format!("access-{account_id}"),
            "account_id": account_id,
            "id_token": jwt(json!({
                "email": email,
                "https://api.openai.com/auth": {
                    "chatgpt_account_id": account_id,
                    "chatgpt_user_id": user_id,
                    "chatgpt_plan_type": "plus"
                }
            }))
        }
    }))
    .unwrap()
}

pub(super) fn save_account(temp: &TempDir, email: &str, user_id: &str, account_id: &str) {
    fs::create_dir_all(temp.path()).unwrap();
    fs::write(
        storage::active_auth_path(temp.path()),
        auth_bytes(email, user_id, account_id),
    )
    .unwrap();
    let info = auth::parse_file(&storage::active_auth_path(temp.path())).unwrap();
    storage::save_active_snapshot(temp.path(), &info).unwrap();
}

#[derive(Default)]
pub(super) struct FakeUsageFetcher {
    pub(super) results: HashMap<String, Result<UsageSnapshot, usage::UsageError>>,
}

impl UsageFetcher for FakeUsageFetcher {
    fn fetch(&self, auth: &AuthInfo) -> Result<UsageSnapshot, usage::UsageError> {
        self.results
            .get(&auth.record_key())
            .cloned()
            .unwrap_or_else(|| Err(usage::UsageError::Transport("missing fake".to_string())))
    }
}

pub(super) struct PanicUsageFetcher;

impl UsageFetcher for PanicUsageFetcher {
    fn fetch(&self, _auth: &AuthInfo) -> Result<UsageSnapshot, usage::UsageError> {
        panic!("cached-only rows should not fetch remote usage");
    }
}

pub(super) fn usage_snapshot() -> UsageSnapshot {
    UsageSnapshot {
        plan: Some("team".to_string()),
        primary: Some(usage::UsageWindow {
            used_percent: 10.0,
            window_minutes: Some(300),
            reset_at: Some(1_700_000_000),
        }),
        secondary: Some(usage::UsageWindow {
            used_percent: 20.0,
            window_minutes: Some(10_080),
            reset_at: Some(1_700_100_000),
        }),
    }
}

pub(super) fn local_usage_snapshot() -> UsageSnapshot {
    UsageSnapshot {
        plan: Some("plus".to_string()),
        primary: Some(usage::UsageWindow {
            used_percent: 27.0,
            window_minutes: Some(300),
            reset_at: Some(1_700_000_000),
        }),
        secondary: Some(usage::UsageWindow {
            used_percent: 22.0,
            window_minutes: Some(10_080),
            reset_at: Some(1_700_100_000),
        }),
    }
}

pub(super) fn rollout_line(timestamp: &str, primary_used: f64, secondary_used: f64) -> String {
    format!(
        r#"{{"timestamp":"{timestamp}","type":"event_msg","payload":{{"type":"token_count","rate_limits":{{"limit_id":"codex","primary":{{"used_percent":{primary_used},"window_minutes":300,"resets_at":1778978811}},"secondary":{{"used_percent":{secondary_used},"window_minutes":10080,"resets_at":1779320299}},"plan_type":"plus"}}}}}}"#
    )
}
