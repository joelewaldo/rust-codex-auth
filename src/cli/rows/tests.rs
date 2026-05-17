use std::collections::HashMap;

use crate::{storage, usage};

use super::*;
use crate::cli::test_support::{
    FakeUsageFetcher, PanicUsageFetcher, local_usage_snapshot, save_account, usage_snapshot,
};

#[test]
fn rows_surface_usage_errors() {
    let temp = tempfile::TempDir::new().unwrap();
    save_account(&temp, "user@example.com", "user-1", "acct-1");
    let mut registry = storage::load_registry(temp.path()).unwrap();
    registry.accounts[0].last_usage = Some(local_usage_snapshot());
    let fetcher = FakeUsageFetcher {
        results: HashMap::from([(
            "user-1::acct-1".to_string(),
            Err(usage::UsageError::Http(401)),
        )]),
    };

    let (rows, changed) = build_rows(
        temp.path(),
        &mut registry,
        &fetcher,
        RowUsageMode::RefreshDisplay,
    );

    assert_eq!(rows[0].five_hour, "HTTP 401");
    assert_eq!(rows[0].weekly, "HTTP 401");
    assert!(!changed);
}

#[test]
fn rows_use_remote_usage_data() {
    let temp = tempfile::TempDir::new().unwrap();
    save_account(&temp, "user@example.com", "user-1", "acct-1");
    let mut registry = storage::load_registry(temp.path()).unwrap();
    let fetcher = FakeUsageFetcher {
        results: HashMap::from([("user-1::acct-1".to_string(), Ok(usage_snapshot()))]),
    };

    let (rows, changed) = build_rows(
        temp.path(),
        &mut registry,
        &fetcher,
        RowUsageMode::RefreshDisplay,
    );

    assert_eq!(rows[0].plan, "team");
    assert_eq!(rows[0].five_hour, "90%");
    assert_eq!(rows[0].weekly, "80%");
    assert!(changed);
    assert!(registry.accounts[0].last_usage.is_some());
}

#[test]
fn refresh_display_replaces_stale_cached_usage() {
    let temp = tempfile::TempDir::new().unwrap();
    save_account(&temp, "user@example.com", "user-1", "acct-1");
    let mut registry = storage::load_registry(temp.path()).unwrap();
    let fetcher = FakeUsageFetcher {
        results: HashMap::from([("user-1::acct-1".to_string(), Ok(usage_snapshot()))]),
    };

    registry.accounts[0].last_usage = Some(local_usage_snapshot());
    let (rows, changed) = build_rows(
        temp.path(),
        &mut registry,
        &fetcher,
        RowUsageMode::RefreshDisplay,
    );

    assert_eq!(rows[0].plan, "team");
    assert_eq!(rows[0].five_hour, "90%");
    assert_eq!(rows[0].weekly, "80%");
    assert!(changed);
}

#[test]
fn successful_refresh_updates_timestamp_even_when_usage_is_unchanged() {
    let temp = tempfile::TempDir::new().unwrap();
    save_account(&temp, "user@example.com", "user-1", "acct-1");
    let mut registry = storage::load_registry(temp.path()).unwrap();
    registry.accounts[0].last_usage = Some(usage_snapshot());
    registry.accounts[0].last_usage_at = Some(60);
    let fetcher = FakeUsageFetcher {
        results: HashMap::from([("user-1::acct-1".to_string(), Ok(usage_snapshot()))]),
    };

    let (_, changed) = build_rows(
        temp.path(),
        &mut registry,
        &fetcher,
        RowUsageMode::RefreshDisplay,
    );

    assert!(changed);
    assert!(registry.accounts[0].last_usage_at.unwrap() > 60);
}

#[test]
fn same_minute_usage_is_fresh() {
    let temp = tempfile::TempDir::new().unwrap();
    save_account(&temp, "user@example.com", "user-1", "acct-1");
    let mut registry = storage::load_registry(temp.path()).unwrap();
    registry.accounts[0].last_usage = Some(usage_snapshot());
    registry.accounts[0].last_usage_at = Some(120);

    assert!(has_usage_from_same_minute(&registry.accounts[0], 179));
    assert!(!has_usage_from_same_minute(&registry.accounts[0], 180));
}

#[test]
fn active_local_usage_takes_precedence_over_remote_refresh() {
    let temp = tempfile::TempDir::new().unwrap();
    save_account(&temp, "user@example.com", "user-1", "acct-1");
    let mut registry = storage::load_registry(temp.path()).unwrap();
    registry.accounts[0].last_usage = Some(local_usage_snapshot());
    registry.accounts[0].last_local_rollout = Some(storage::RolloutSignature {
        path: "sessions/rollout-test.jsonl".to_string(),
        event_timestamp_ms: 1_778_962_469_101,
    });

    let (rows, changed) = build_rows(
        temp.path(),
        &mut registry,
        &PanicUsageFetcher,
        RowUsageMode::RefreshDisplay,
    );

    assert_eq!(rows[0].plan, "plus");
    assert_eq!(rows[0].five_hour, "73%");
    assert_eq!(rows[0].weekly, "78%");
    assert!(!changed);
}

#[test]
fn cached_only_rows_do_not_fetch_remote_usage() {
    let temp = tempfile::TempDir::new().unwrap();
    save_account(&temp, "user@example.com", "user-1", "acct-1");
    let mut registry = storage::load_registry(temp.path()).unwrap();

    let (rows, changed) = build_rows(
        temp.path(),
        &mut registry,
        &PanicUsageFetcher,
        RowUsageMode::CachedOnly,
    );

    assert_eq!(rows[0].five_hour, "-");
    assert_eq!(rows[0].weekly, "-");
    assert!(!changed);
}
