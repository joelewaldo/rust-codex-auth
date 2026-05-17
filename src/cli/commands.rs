use std::{
    io::{self, Write},
    path::Path,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{
    auth,
    storage::{self, Registry, RolloutSignature},
    usage::{self, RemoteUsageFetcher, UsageFetcher},
};

use super::{
    CliError,
    rows::{RowUsageMode, build_rows, print_rows},
    select::{display_label, display_record_label, duplicate_record_email, resolve_selector},
};

pub(super) fn run_codex_login(device_auth: bool) -> Result<std::path::PathBuf, CliError> {
    let mut command = Command::new("codex");
    command.arg("login");
    if device_auth {
        command.arg("--device-auth");
    }
    let status = command
        .status()
        .map_err(|err| CliError::Message(format!("failed to run `codex login`: {err}")))?;
    if !status.success() {
        return Err(CliError::Message(format!(
            "`codex login` exited with status {status}"
        )));
    }
    storage::resolve_codex_home().map_err(CliError::from)
}

pub(super) fn complete_login(codex_home: &Path) -> Result<(), CliError> {
    let auth_path = storage::active_auth_path(codex_home);
    let info = auth::parse_file(&auth_path)?;
    storage::save_active_snapshot(codex_home, &info)?;
    let mut registry = storage::load_registry(codex_home)?;
    storage::upsert_account(&mut registry, &info);
    storage::set_active(&mut registry, &info.record_key());
    storage::save_registry(codex_home, &registry)?;
    println!("Saved {}", display_label(&info, false));
    Ok(())
}

fn load_registry_state(codex_home: &Path) -> Result<(Registry, bool), CliError> {
    let mut registry = storage::load_registry(codex_home)?;
    let changed = storage::sync_active_account(codex_home, &mut registry)?;
    Ok((registry, changed))
}

fn refresh_active_local_usage(codex_home: &Path, registry: &mut Registry) -> bool {
    let Some(active_key) = registry.active_account_key.clone() else {
        return false;
    };
    let Ok((local, updated_cache)) =
        usage::latest_local_usage(codex_home, registry.latest_local_rollout.as_ref())
    else {
        return false;
    };
    let mut changed = false;
    if let Some(cache) = updated_cache {
        registry.latest_local_rollout = Some(cache);
        changed = true;
    }
    let Some(local) = local else {
        return changed;
    };
    let activated_at_ms = registry.active_account_activated_at_ms.unwrap_or(0);
    if local.event_timestamp_ms < activated_at_ms {
        return changed;
    }
    let signature = RolloutSignature {
        path: local.path.to_string_lossy().into_owned(),
        event_timestamp_ms: local.event_timestamp_ms,
    };
    let Some(record) = registry.account_mut(&active_key) else {
        return changed;
    };
    if record.last_local_rollout.as_ref() == Some(&signature) {
        return changed || record.last_usage_error.take().is_some();
    }
    record.last_usage = Some(local.snapshot);
    record.last_usage_at = Some(now_seconds());
    record.last_usage_error = None;
    record.last_local_rollout = Some(signature);
    true
}

fn now_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

pub(super) fn list_accounts(
    codex_home: &Path,
    fetcher: &impl UsageFetcher,
) -> Result<(), CliError> {
    let (mut registry, mut changed) = load_registry_state(codex_home)?;
    if registry.accounts.is_empty() {
        if changed {
            storage::save_registry(codex_home, &registry)?;
        }
        println!("No saved accounts.");
        return Ok(());
    }
    changed |= refresh_active_local_usage(codex_home, &mut registry);
    let (rows, usage_changed) = build_rows(
        codex_home,
        &mut registry,
        fetcher,
        RowUsageMode::RefreshDisplay,
    );
    changed |= usage_changed;
    if changed {
        storage::save_registry(codex_home, &registry)?;
    }
    print_rows(&rows);
    Ok(())
}

pub(super) fn switch_account(codex_home: &Path, selector: &str) -> Result<(), CliError> {
    let (mut registry, _) = load_registry_state(codex_home)?;
    refresh_active_local_usage(codex_home, &mut registry);
    let index = resolve_selector(&registry.accounts, selector)?;
    let record = registry.accounts[index].clone();
    let snapshot = storage::load_snapshot_by_key(codex_home, &record.account_key)?;
    storage::activate_account(codex_home, &snapshot)?;
    storage::set_active(&mut registry, &record.account_key);
    storage::save_registry(codex_home, &registry)?;
    println!(
        "Switched to {}",
        display_record_label(&record, duplicate_record_email(&registry.accounts, index))
    );
    Ok(())
}

pub(super) fn interactive_switch(codex_home: &Path) -> Result<(), CliError> {
    let (mut registry, mut changed) = load_registry_state(codex_home)?;
    if registry.accounts.is_empty() {
        return Err(CliError::Message("no saved accounts".to_string()));
    }

    changed |= refresh_active_local_usage(codex_home, &mut registry);
    if changed {
        storage::save_registry(codex_home, &registry)?;
    }
    let fetcher = RemoteUsageFetcher;
    let (rows, _) = build_rows(
        codex_home,
        &mut registry,
        &fetcher,
        RowUsageMode::CachedOnly,
    );
    print_rows(&rows);
    print!("Select account number: ");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let selector = input.trim();
    if selector.is_empty() {
        return Err(CliError::Message("no account selected".to_string()));
    }
    switch_account(codex_home, selector)
}

pub(super) fn remove_account(codex_home: &Path, selector: &str) -> Result<(), CliError> {
    let (mut registry, _) = load_registry_state(codex_home)?;
    refresh_active_local_usage(codex_home, &mut registry);
    let index = resolve_selector(&registry.accounts, selector)?;
    let removed = registry.accounts[index].clone();
    let removed_label =
        display_record_label(&removed, duplicate_record_email(&registry.accounts, index));
    let active_removed = registry.active_account_key.as_deref() == Some(&removed.account_key);
    let removed_snapshot = storage::load_snapshot_by_key(codex_home, &removed.account_key)?;

    storage::remove_snapshot(&removed_snapshot)?;
    storage::remove_record(&mut registry, &removed.account_key);
    if active_removed {
        if let Some(next) = registry.accounts.first().cloned() {
            let next_snapshot = storage::load_snapshot_by_key(codex_home, &next.account_key)?;
            storage::activate_account(codex_home, &next_snapshot)?;
            storage::set_active(&mut registry, &next.account_key);
        } else {
            storage::remove_active_auth(codex_home)?;
        }
    }
    storage::save_registry(codex_home, &registry)?;

    println!("Removed {removed_label}");
    Ok(())
}

#[cfg(test)]
mod tests;
