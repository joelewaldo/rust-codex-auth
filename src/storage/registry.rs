use std::{
    cmp::Ordering,
    fs, io,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::auth::AuthInfo;

use super::{
    StorageError,
    accounts::{active_info, load_accounts},
    model::{AccountRecord, Registry},
    private_fs::{ensure_private_dir, write_private_file},
    registry_path,
};

pub fn load_registry(codex_home: &Path) -> Result<Registry, StorageError> {
    let path = registry_path(codex_home);
    match fs::read(&path) {
        Ok(bytes) => {
            let mut registry: Registry = serde_json::from_slice(&bytes)?;
            registry.accounts.sort_by(account_record_order);
            Ok(registry)
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => bootstrap_registry(codex_home),
        Err(err) => Err(StorageError::Io(err)),
    }
}

pub fn save_registry(codex_home: &Path, registry: &Registry) -> Result<(), StorageError> {
    let path = registry_path(codex_home);
    let parent = path.parent().expect("registry path has parent");
    ensure_private_dir(parent)?;
    write_private_file(&path, &serde_json::to_vec_pretty(registry)?)?;
    Ok(())
}

pub fn sync_active_account(
    codex_home: &Path,
    registry: &mut Registry,
) -> Result<bool, StorageError> {
    let Some(info) = active_info(codex_home)? else {
        let active_key_changed = registry.active_account_key.take().is_some();
        let activation_changed = registry.active_account_activated_at_ms.take().is_some();
        let changed = active_key_changed || activation_changed;
        return Ok(changed);
    };
    let key = info.record_key();
    let mut changed = upsert_account(registry, &info);
    changed |= super::accounts::sync_active_snapshot(codex_home, &info)?;
    if registry.active_account_key.as_deref() != Some(key.as_str()) {
        registry.active_account_key = Some(key);
        registry.active_account_activated_at_ms = Some(now_millis());
        changed = true;
    }
    Ok(changed)
}

pub fn upsert_account(registry: &mut Registry, info: &AuthInfo) -> bool {
    let key = info.record_key();
    if let Some(record) = registry.account_mut(&key) {
        let changed = record.chatgpt_account_id != info.account_id
            || record.chatgpt_user_id != info.user_id
            || record.email != info.email
            || record.plan != info.plan;
        record.chatgpt_account_id = info.account_id.clone();
        record.chatgpt_user_id = info.user_id.clone();
        record.email = info.email.clone();
        record.plan = info.plan.clone();
        return changed;
    }

    registry.accounts.push(AccountRecord {
        account_key: key,
        chatgpt_account_id: info.account_id.clone(),
        chatgpt_user_id: info.user_id.clone(),
        email: info.email.clone(),
        plan: info.plan.clone(),
        created_at: now_seconds(),
        last_used_at: None,
        last_usage: None,
        last_usage_at: None,
        last_usage_error: None,
        last_local_rollout: None,
    });
    registry.accounts.sort_by(account_record_order);
    true
}

pub fn set_active(registry: &mut Registry, key: &str) {
    registry.active_account_key = Some(key.to_string());
    registry.active_account_activated_at_ms = Some(now_millis());
    if let Some(record) = registry.account_mut(key) {
        record.last_used_at = Some(now_seconds());
    }
}

pub fn remove_record(registry: &mut Registry, key: &str) {
    registry.accounts.retain(|record| record.account_key != key);
    if registry.active_account_key.as_deref() == Some(key) {
        registry.active_account_key = None;
        registry.active_account_activated_at_ms = None;
    }
}

fn bootstrap_registry(codex_home: &Path) -> Result<Registry, StorageError> {
    let mut registry = Registry::new();
    for account in load_accounts(codex_home)? {
        upsert_account(&mut registry, &account.info);
    }
    if let Some(info) = active_info(codex_home)? {
        upsert_account(&mut registry, &info);
        registry.active_account_key = Some(info.record_key());
        registry.active_account_activated_at_ms = Some(0);
    }
    Ok(registry)
}

fn account_record_order(left: &AccountRecord, right: &AccountRecord) -> Ordering {
    left.email
        .cmp(&right.email)
        .then_with(|| left.chatgpt_account_id.cmp(&right.chatgpt_account_id))
}

fn now_seconds() -> i64 {
    now_millis() / 1_000
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}
