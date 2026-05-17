use std::fs;

use super::*;

fn rollout_line(primary_used: f64, secondary_used: f64) -> String {
    format!(
        r#"{{"timestamp":"2026-05-16T20:14:29.101Z","type":"event_msg","payload":{{"type":"token_count","rate_limits":{{"limit_id":"codex","primary":{{"used_percent":{primary_used},"window_minutes":300,"resets_at":1778978811}},"secondary":{{"used_percent":{secondary_used},"window_minutes":10080,"resets_at":1779320299}},"plan_type":"plus"}}}}}}"#
    )
}

#[test]
fn parses_latest_rollout_usage() {
    let temp = tempfile::TempDir::new().unwrap();
    let path = temp.path().join("rollout-test.jsonl");
    fs::write(
        &path,
        concat!(
            r#"{"timestamp":"2026-05-16T20:14:28.101Z","type":"event_msg","payload":{"type":"token_count","rate_limits":null}}"#,
            "\n",
            r#"{"timestamp":"2026-05-16T20:14:29.101Z","type":"event_msg","payload":{"type":"token_count","rate_limits":{"limit_id":"codex","primary":{"used_percent":27.0,"window_minutes":300,"resets_at":1778978811},"secondary":{"used_percent":22.0,"window_minutes":10080,"resets_at":1779320299},"plan_type":"plus"}}}"#
        ),
    )
    .unwrap();

    let (_, snapshot) = parse_latest_rollout_usage(&path).unwrap().unwrap();

    assert_eq!(snapshot.plan.as_deref(), Some("plus"));
    assert_eq!(snapshot.primary.as_ref().unwrap().remaining_percent(), 73);
    assert_eq!(snapshot.secondary.as_ref().unwrap().remaining_percent(), 78);
}

#[test]
fn latest_local_usage_reads_newest_rollout() {
    let temp = tempfile::TempDir::new().unwrap();
    let rollout_dir = temp.path().join("sessions/2026/05/16");
    fs::create_dir_all(&rollout_dir).unwrap();
    fs::write(
        rollout_dir.join("rollout-test.jsonl"),
        rollout_line(27.0, 22.0),
    )
    .unwrap();

    let (local, cache) = latest_local_usage(temp.path(), None).unwrap();
    let local = local.unwrap();

    assert_eq!(
        local.snapshot.primary.as_ref().unwrap().remaining_percent(),
        73
    );
    assert_eq!(
        local
            .snapshot
            .secondary
            .as_ref()
            .unwrap()
            .remaining_percent(),
        78
    );
    assert!(cache.is_some());
}

#[test]
fn latest_local_usage_reuses_unchanged_cached_rollout() {
    let temp = tempfile::TempDir::new().unwrap();
    let rollout_dir = temp.path().join("sessions/2026/05/16");
    fs::create_dir_all(&rollout_dir).unwrap();
    fs::write(
        rollout_dir.join("rollout-test.jsonl"),
        rollout_line(27.0, 22.0),
    )
    .unwrap();

    let (_, cache) = latest_local_usage(temp.path(), None).unwrap();
    let cache = cache.unwrap();
    let (local, updated_cache) = latest_local_usage(temp.path(), Some(&cache)).unwrap();

    assert_eq!(
        local
            .unwrap()
            .snapshot
            .primary
            .as_ref()
            .unwrap()
            .remaining_percent(),
        73
    );
    assert!(updated_cache.is_none());
}

#[test]
fn parses_rollout_event_timestamp() {
    assert_eq!(
        parse_rfc3339_utc_millis("2026-05-16T20:14:29.101Z"),
        Some(1_778_962_469_101)
    );
}
