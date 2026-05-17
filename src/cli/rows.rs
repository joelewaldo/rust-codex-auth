use std::{collections::HashMap, path::Path, thread, time::SystemTime};

use crate::{
    auth::AuthInfo,
    storage::{self, AccountRecord, Registry},
    usage::{self, UsageFetcher, UsageSnapshot},
};

use super::select::display_record_label;

#[derive(Debug, Clone)]
pub(super) struct AccountRow {
    active: bool,
    label: String,
    plan: String,
    five_hour: String,
    weekly: String,
    five_hour_reset: String,
    weekly_reset: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RowUsageMode {
    CachedOnly,
    RefreshDisplay,
}

pub(super) fn build_rows(
    codex_home: &Path,
    registry: &mut Registry,
    fetcher: &impl UsageFetcher,
    mode: RowUsageMode,
) -> (Vec<AccountRow>, bool) {
    let duplicates = duplicate_email_counts(&registry.accounts);
    let mut usage_results: Vec<Option<Result<UsageSnapshot, usage::UsageError>>> = registry
        .accounts
        .iter()
        .map(|record| record.last_usage.clone().map(Ok))
        .collect();

    let mut changed = false;
    if mode == RowUsageMode::CachedOnly {
        for (index, record) in registry.accounts.iter().enumerate() {
            usage_results[index].get_or_insert_with(|| Ok(empty_usage_snapshot(record)));
        }
    } else {
        let refresh_started_at = now_seconds();
        let mut pending = Vec::new();
        for (index, record) in registry.accounts.iter().enumerate() {
            if has_active_local_usage(registry, record)
                || has_usage_from_same_minute(record, refresh_started_at)
            {
                continue;
            }
            match storage::load_snapshot_by_key(codex_home, &record.account_key) {
                Ok(snapshot) => pending.push((index, snapshot.info)),
                Err(_) => {
                    usage_results[index] = Some(Err(usage::UsageError::Transport(
                        "missing snapshot".to_string(),
                    )));
                }
            }
        }
        for (index, result) in fetch_usage_concurrently(fetcher, pending) {
            if let Ok(snapshot) = &result {
                let record = &mut registry.accounts[index];
                if record.last_usage.as_ref() != Some(snapshot) {
                    record.last_usage = Some(snapshot.clone());
                    changed = true;
                }
                if record.last_usage_at != Some(refresh_started_at) {
                    record.last_usage_at = Some(refresh_started_at);
                    changed = true;
                }
            }
            usage_results[index] = Some(result);
        }
    }

    let rows = registry
        .accounts
        .iter()
        .zip(usage_results)
        .map(|(record, usage)| {
            let duplicate = duplicates.get(record.email.as_str()).copied().unwrap_or(0) > 1;
            row_for_account(
                record,
                duplicate,
                registry.active_account_key.as_deref(),
                usage.unwrap_or_else(|| Ok(empty_usage_snapshot(record))),
            )
        })
        .collect();
    (rows, changed)
}

fn has_active_local_usage(registry: &Registry, record: &AccountRecord) -> bool {
    registry.active_account_key.as_deref() == Some(record.account_key.as_str())
        && record.last_local_rollout.is_some()
        && record.last_usage.is_some()
}

fn has_usage_from_same_minute(record: &AccountRecord, now_seconds: i64) -> bool {
    record.last_usage.is_some()
        && record
            .last_usage_at
            .is_some_and(|last_usage_at| last_usage_at / 60 == now_seconds / 60)
}

fn empty_usage_snapshot(record: &AccountRecord) -> UsageSnapshot {
    UsageSnapshot {
        plan: record.plan.clone(),
        primary: None,
        secondary: None,
    }
}

fn duplicate_email_counts(accounts: &[AccountRecord]) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for account in accounts {
        *counts.entry(account.email.clone()).or_insert(0) += 1;
    }
    counts
}

fn fetch_usage_concurrently(
    fetcher: &impl UsageFetcher,
    pending: Vec<(usize, AuthInfo)>,
) -> Vec<(usize, Result<UsageSnapshot, usage::UsageError>)> {
    const MAX_CONCURRENT_USAGE_FETCHES: usize = 4;
    let concurrency = thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(1)
        .min(MAX_CONCURRENT_USAGE_FETCHES);
    let mut results = Vec::with_capacity(pending.len());

    for chunk in pending.chunks(concurrency) {
        let chunk_results = thread::scope(|scope| {
            let handles: Vec<_> = chunk
                .iter()
                .cloned()
                .map(|(index, auth)| (index, scope.spawn(move || fetcher.fetch(&auth))))
                .collect();
            handles
                .into_iter()
                .map(|(index, handle)| {
                    let result = handle.join().unwrap_or_else(|_| {
                        Err(usage::UsageError::Transport(
                            "usage fetch worker panicked".to_string(),
                        ))
                    });
                    (index, result)
                })
                .collect::<Vec<_>>()
        });
        results.extend(chunk_results);
    }

    results
}

fn row_for_account(
    account: &AccountRecord,
    duplicate: bool,
    active_key: Option<&str>,
    usage_result: Result<UsageSnapshot, usage::UsageError>,
) -> AccountRow {
    let active = active_key == Some(account.account_key.as_str());
    match usage_result {
        Ok(snapshot) => AccountRow {
            active,
            label: display_record_label(account, duplicate),
            plan: snapshot
                .plan
                .or_else(|| account.plan.clone())
                .unwrap_or_else(|| "-".to_string()),
            five_hour: usage::format_remaining(snapshot.primary.as_ref()),
            weekly: usage::format_remaining(snapshot.secondary.as_ref()),
            five_hour_reset: usage::format_reset(snapshot.primary.as_ref()),
            weekly_reset: usage::format_reset(snapshot.secondary.as_ref()),
        },
        Err(err) => AccountRow {
            active,
            label: display_record_label(account, duplicate),
            plan: account.plan.clone().unwrap_or_else(|| "-".to_string()),
            five_hour: err.to_string(),
            weekly: err.to_string(),
            five_hour_reset: "-".to_string(),
            weekly_reset: "-".to_string(),
        },
    }
}

pub(super) fn print_rows(rows: &[AccountRow]) {
    println!(
        "{:<4} {:<42} {:<10} {:<14} {:<14} {:<14} {:<14}",
        "", "ACCOUNT", "PLAN", "5H LEFT", "5H RESET", "WEEKLY LEFT", "WEEKLY RESET"
    );
    for (index, row) in rows.iter().enumerate() {
        let marker = if row.active { "*" } else { " " };
        println!(
            "{:<4} {:<42} {:<10} {:<14} {:<14} {:<14} {:<14}",
            format!("{marker}{:02}", index + 1),
            row.label,
            row.plan,
            row.five_hour,
            row.five_hour_reset,
            row.weekly,
            row.weekly_reset
        );
    }
}

fn now_seconds() -> i64 {
    SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests;
