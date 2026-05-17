use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{
    auth::AuthInfo,
    constants::REGISTRY_SCHEMA_VERSION,
    usage::{LocalRolloutCache, UsageSnapshot},
};

#[derive(Debug, Clone)]
pub struct AccountSnapshot {
    pub info: AuthInfo,
    pub path: PathBuf,
}

impl AccountSnapshot {
    pub fn key(&self) -> String {
        self.info.record_key()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RolloutSignature {
    pub path: String,
    pub event_timestamp_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountRecord {
    pub account_key: String,
    pub chatgpt_account_id: String,
    pub chatgpt_user_id: String,
    pub email: String,
    pub plan: Option<String>,
    pub created_at: i64,
    pub last_used_at: Option<i64>,
    pub last_usage: Option<UsageSnapshot>,
    pub last_usage_at: Option<i64>,
    #[serde(default)]
    pub last_usage_error: Option<crate::usage::UsageError>,
    pub last_local_rollout: Option<RolloutSignature>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Registry {
    pub schema_version: u32,
    pub active_account_key: Option<String>,
    pub active_account_activated_at_ms: Option<i64>,
    #[serde(default)]
    pub latest_local_rollout: Option<LocalRolloutCache>,
    pub accounts: Vec<AccountRecord>,
}

impl Registry {
    pub(super) fn new() -> Self {
        Self {
            schema_version: REGISTRY_SCHEMA_VERSION,
            active_account_key: None,
            active_account_activated_at_ms: None,
            latest_local_rollout: None,
            accounts: Vec::new(),
        }
    }

    pub fn account(&self, key: &str) -> Option<&AccountRecord> {
        self.accounts
            .iter()
            .find(|record| record.account_key == key)
    }

    pub fn account_mut(&mut self, key: &str) -> Option<&mut AccountRecord> {
        self.accounts
            .iter_mut()
            .find(|record| record.account_key == key)
    }
}
