use std::fs;

use crate::{auth, storage};

use super::*;
use crate::cli::test_support::{rollout_line, save_account};

#[test]
fn local_usage_updates_only_after_active_account_activation() {
    let temp = tempfile::TempDir::new().unwrap();
    save_account(&temp, "user@example.com", "user-1", "acct-1");
    let mut registry = storage::load_registry(temp.path()).unwrap();
    registry.active_account_activated_at_ms = Some(1_778_962_469_102);
    let rollout_dir = temp.path().join("sessions/2026/05/16");
    fs::create_dir_all(&rollout_dir).unwrap();
    fs::write(
        rollout_dir.join("rollout-test.jsonl"),
        rollout_line("2026-05-16T20:14:29.101Z", 27.0, 22.0),
    )
    .unwrap();

    refresh_active_local_usage(temp.path(), &mut registry);

    assert!(registry.accounts[0].last_usage.is_none());

    registry.active_account_activated_at_ms = Some(0);
    refresh_active_local_usage(temp.path(), &mut registry);

    assert_eq!(
        registry.accounts[0]
            .last_usage
            .as_ref()
            .unwrap()
            .primary
            .as_ref()
            .unwrap()
            .remaining_percent(),
        73
    );
}

#[test]
fn removing_active_account_promotes_first_remaining_snapshot() {
    let temp = tempfile::TempDir::new().unwrap();
    save_account(&temp, "a@example.com", "user-a", "acct-a");
    save_account(&temp, "b@example.com", "user-b", "acct-b");
    let accounts = storage::load_accounts(temp.path()).unwrap();
    storage::activate_account(temp.path(), &accounts[0]).unwrap();

    remove_account(temp.path(), "1").unwrap();

    let active = auth::parse_file(&storage::active_auth_path(temp.path())).unwrap();
    assert_eq!(active.email, "b@example.com");
}

#[test]
fn removing_last_active_account_deletes_live_auth() {
    let temp = tempfile::TempDir::new().unwrap();
    save_account(&temp, "a@example.com", "user-a", "acct-a");
    let accounts = storage::load_accounts(temp.path()).unwrap();
    storage::activate_account(temp.path(), &accounts[0]).unwrap();

    remove_account(temp.path(), "1").unwrap();

    assert!(!storage::active_auth_path(temp.path()).exists());
}
