use std::{collections::HashMap, fs};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde_json::json;
use tempfile::TempDir;

use super::*;

fn jwt(payload: serde_json::Value) -> String {
    let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"none","typ":"JWT"}"#);
    let payload = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&payload).unwrap());
    format!("{header}.{payload}.sig")
}

fn auth_bytes_with_token(
    email: &str,
    user_id: &str,
    account_id: &str,
    access_token: &str,
) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "tokens": {
            "access_token": access_token,
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

fn write_active_auth(temp: &TempDir, email: &str, user_id: &str, account_id: &str) {
    write_active_auth_with_token(
        temp,
        email,
        user_id,
        account_id,
        &format!("access-{account_id}"),
    );
}

fn write_active_auth_with_token(
    temp: &TempDir,
    email: &str,
    user_id: &str,
    account_id: &str,
    access_token: &str,
) {
    fs::create_dir_all(temp.path()).unwrap();
    fs::write(
        active_auth_path(temp.path()),
        auth_bytes_with_token(email, user_id, account_id, access_token),
    )
    .unwrap();
}

#[test]
fn resolves_codex_home_in_expected_order() {
    let temp = TempDir::new().unwrap();
    let codex_home = temp.path().join("custom");
    fs::create_dir_all(&codex_home).unwrap();
    let vars = HashMap::from([
        (
            "CODEX_HOME".to_string(),
            codex_home.clone().into_os_string(),
        ),
        (
            "HOME".to_string(),
            temp.path().join("home").into_os_string(),
        ),
        (
            "USERPROFILE".to_string(),
            temp.path().join("profile").into_os_string(),
        ),
    ]);

    let resolved = resolve_codex_home_from(|name| vars.get(name).cloned()).unwrap();

    assert_eq!(resolved, codex_home);
}

#[test]
fn falls_back_to_home_when_codex_home_is_absent() {
    let temp = TempDir::new().unwrap();
    let vars = HashMap::from([("HOME".to_string(), temp.path().as_os_str().to_os_string())]);

    let resolved = resolve_codex_home_from(|name| vars.get(name).cloned()).unwrap();

    assert_eq!(resolved, temp.path().join(".codex"));
}

#[test]
fn falls_back_to_userprofile_when_home_is_absent() {
    let temp = TempDir::new().unwrap();
    let vars = HashMap::from([(
        "USERPROFILE".to_string(),
        temp.path().as_os_str().to_os_string(),
    )]);

    let resolved = resolve_codex_home_from(|name| vars.get(name).cloned()).unwrap();

    assert_eq!(resolved, temp.path().join(".codex"));
}

#[test]
fn saves_and_loads_snapshot_without_registry() {
    let temp = TempDir::new().unwrap();
    write_active_auth(&temp, "user@example.com", "user-1", "acct-1");
    let info = crate::auth::parse_file(&active_auth_path(temp.path())).unwrap();

    let snapshot_path = save_active_snapshot(temp.path(), &info).unwrap();
    let accounts = load_accounts(temp.path()).unwrap();

    assert!(snapshot_path.exists());
    assert_eq!(accounts.len(), 1);
    assert_eq!(accounts[0].info.record_key(), "user-1::acct-1");
}

#[test]
fn detects_active_account_from_live_auth() {
    let temp = TempDir::new().unwrap();
    write_active_auth(&temp, "user@example.com", "user-1", "acct-1");

    assert_eq!(
        active_key(temp.path()).unwrap().as_deref(),
        Some("user-1::acct-1")
    );
}

#[test]
fn activating_snapshot_replaces_live_auth() {
    let temp = TempDir::new().unwrap();
    write_active_auth(&temp, "first@example.com", "user-1", "acct-1");
    let first = crate::auth::parse_file(&active_auth_path(temp.path())).unwrap();
    save_active_snapshot(temp.path(), &first).unwrap();

    write_active_auth(&temp, "second@example.com", "user-2", "acct-2");
    let second = crate::auth::parse_file(&active_auth_path(temp.path())).unwrap();
    save_active_snapshot(temp.path(), &second).unwrap();

    let accounts = load_accounts(temp.path()).unwrap();
    let first_snapshot = accounts
        .iter()
        .find(|account| account.info.account_id == "acct-1")
        .unwrap();
    activate_account(temp.path(), first_snapshot).unwrap();

    let active = crate::auth::parse_file(&active_auth_path(temp.path())).unwrap();
    assert_eq!(active.email, "first@example.com");
}

#[test]
fn sync_active_account_creates_and_refreshes_managed_snapshot() {
    let temp = TempDir::new().unwrap();
    write_active_auth_with_token(&temp, "user@example.com", "user-1", "acct-1", "access-old");
    let mut registry = load_registry(temp.path()).unwrap();

    assert!(sync_active_account(temp.path(), &mut registry).unwrap());
    assert_eq!(
        load_snapshot_by_key(temp.path(), "user-1::acct-1")
            .unwrap()
            .info
            .access_token,
        "access-old"
    );

    write_active_auth_with_token(&temp, "user@example.com", "user-1", "acct-1", "access-new");

    assert!(sync_active_account(temp.path(), &mut registry).unwrap());
    assert_eq!(
        load_snapshot_by_key(temp.path(), "user-1::acct-1")
            .unwrap()
            .info
            .access_token,
        "access-new"
    );
}

#[test]
fn sync_active_account_clears_active_state_without_live_auth() {
    let temp = TempDir::new().unwrap();
    write_active_auth(&temp, "user@example.com", "user-1", "acct-1");
    let mut registry = load_registry(temp.path()).unwrap();
    fs::remove_file(active_auth_path(temp.path())).unwrap();

    assert!(sync_active_account(temp.path(), &mut registry).unwrap());
    assert!(registry.active_account_key.is_none());
    assert!(registry.active_account_activated_at_ms.is_none());
}
