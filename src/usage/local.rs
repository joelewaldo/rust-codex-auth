use std::{
    cmp::Reverse,
    fs, io,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{UsageError, UsageSnapshot, parse_rollout_rate_limits};

#[derive(Debug, Clone, PartialEq)]
pub struct LocalUsage {
    pub path: PathBuf,
    pub event_timestamp_ms: i64,
    pub snapshot: UsageSnapshot,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalRolloutCache {
    pub path: String,
    pub modified_at_ms: i64,
    pub len: u64,
    pub event_timestamp_ms: Option<i64>,
    pub snapshot: Option<UsageSnapshot>,
}

pub fn latest_local_usage(
    codex_home: &Path,
    cached: Option<&LocalRolloutCache>,
) -> Result<(Option<LocalUsage>, Option<LocalRolloutCache>), UsageError> {
    let Some(path) = newest_rollout_path(&codex_home.join("sessions"))? else {
        return Ok((None, None));
    };
    let (modified_at_ms, len) = rollout_file_state(&path)?;
    if let Some(cached) = cached
        && cached.path == path.to_string_lossy()
        && cached.modified_at_ms == modified_at_ms
        && cached.len == len
    {
        return Ok((local_usage_from_cache(cached), None));
    }

    let latest = parse_latest_rollout_usage(&path)?;
    let cache = LocalRolloutCache {
        path: path.to_string_lossy().into_owned(),
        modified_at_ms,
        len,
        event_timestamp_ms: latest.as_ref().map(|(timestamp, _)| *timestamp),
        snapshot: latest.as_ref().map(|(_, snapshot)| snapshot.clone()),
    };
    Ok((
        latest.map(|(event_timestamp_ms, snapshot)| LocalUsage {
            path,
            event_timestamp_ms,
            snapshot,
        }),
        Some(cache),
    ))
}

fn newest_rollout_path(sessions_dir: &Path) -> Result<Option<PathBuf>, UsageError> {
    for year_dir in descending_numeric_dirs(sessions_dir)? {
        for month_dir in descending_numeric_dirs(&year_dir)? {
            for day_dir in descending_numeric_dirs(&month_dir)? {
                if let Some(path) = newest_rollout_in_dir(&day_dir)? {
                    return Ok(Some(path));
                }
            }
        }
    }
    Ok(None)
}

fn descending_numeric_dirs(path: &Path) -> Result<Vec<PathBuf>, UsageError> {
    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(UsageError::Io(err.to_string())),
    };
    let mut dirs = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|err| UsageError::Io(err.to_string()))?;
        if !entry
            .file_type()
            .map_err(|err| UsageError::Io(err.to_string()))?
            .is_dir()
        {
            continue;
        }
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if name.bytes().all(|byte| byte.is_ascii_digit()) {
            dirs.push((Reverse(name), entry.path()));
        }
    }
    dirs.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(dirs.into_iter().map(|(_, path)| path).collect())
}

fn newest_rollout_in_dir(path: &Path) -> Result<Option<PathBuf>, UsageError> {
    let entries = fs::read_dir(path).map_err(|err| UsageError::Io(err.to_string()))?;
    let mut files = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|err| UsageError::Io(err.to_string()))?;
        if !entry
            .file_type()
            .map_err(|err| UsageError::Io(err.to_string()))?
            .is_file()
        {
            continue;
        }
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if name.starts_with("rollout-") && name.ends_with(".jsonl") {
            files.push((name, entry.path()));
        }
    }
    files.sort_by(|left, right| right.0.cmp(&left.0));
    Ok(files.into_iter().next().map(|(_, path)| path))
}

fn rollout_file_state(path: &Path) -> Result<(i64, u64), UsageError> {
    let metadata = fs::metadata(path).map_err(|err| UsageError::Io(err.to_string()))?;
    let modified_at_ms = metadata
        .modified()
        .map_err(|err| UsageError::Io(err.to_string()))?
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0);
    Ok((modified_at_ms, metadata.len()))
}

fn local_usage_from_cache(cache: &LocalRolloutCache) -> Option<LocalUsage> {
    Some(LocalUsage {
        path: PathBuf::from(&cache.path),
        event_timestamp_ms: cache.event_timestamp_ms?,
        snapshot: cache.snapshot.clone()?,
    })
}

fn parse_latest_rollout_usage(path: &Path) -> Result<Option<(i64, UsageSnapshot)>, UsageError> {
    let bytes = fs::read(path).map_err(|err| UsageError::Io(err.to_string()))?;
    for line in bytes.rsplit(|byte| *byte == b'\n') {
        if line.is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_slice::<Value>(line) else {
            continue;
        };
        if value.get("type").and_then(Value::as_str) != Some("event_msg") {
            continue;
        }
        let Some(payload) = value.get("payload") else {
            continue;
        };
        if payload.get("type").and_then(Value::as_str) != Some("token_count") {
            continue;
        }
        let Some(event_timestamp_ms) = value
            .get("timestamp")
            .and_then(Value::as_str)
            .and_then(parse_rfc3339_utc_millis)
        else {
            continue;
        };
        if let Some(snapshot) = payload
            .get("rate_limits")
            .and_then(parse_rollout_rate_limits)
        {
            return Ok(Some((event_timestamp_ms, snapshot)));
        }
    }
    Ok(None)
}

fn parse_rfc3339_utc_millis(timestamp: &str) -> Option<i64> {
    let bytes = timestamp.as_bytes();
    if bytes.len() < 20
        || bytes.get(4) != Some(&b'-')
        || bytes.get(7) != Some(&b'-')
        || bytes.get(10) != Some(&b'T')
        || bytes.get(13) != Some(&b':')
        || bytes.get(16) != Some(&b':')
        || !timestamp.ends_with('Z')
    {
        return None;
    }
    let year = timestamp.get(0..4)?.parse::<i32>().ok()?;
    let month = timestamp.get(5..7)?.parse::<u32>().ok()?;
    let day = timestamp.get(8..10)?.parse::<u32>().ok()?;
    let hour = timestamp.get(11..13)?.parse::<i64>().ok()?;
    let minute = timestamp.get(14..16)?.parse::<i64>().ok()?;
    let second = timestamp.get(17..19)?.parse::<i64>().ok()?;
    let millis = match timestamp.get(19..timestamp.len() - 1)? {
        "" => 0,
        fraction if fraction.starts_with('.') => {
            let digits = fraction[1..].chars().take(3).collect::<String>();
            format!("{digits:0<3}").parse::<i64>().ok()?
        }
        _ => return None,
    };
    let days = days_from_civil(year, month, day)?;
    Some((((days * 24 + hour) * 60 + minute) * 60 + second) * 1_000 + millis)
}

fn days_from_civil(year: i32, month: u32, day: u32) -> Option<i64> {
    if !(1..=12).contains(&month) || day == 0 || day > days_in_month(year, month) {
        return None;
    }
    let adjusted_year = year - i32::from(month <= 2);
    let era = if adjusted_year >= 0 {
        adjusted_year
    } else {
        adjusted_year - 399
    } / 400;
    let year_of_era = adjusted_year - era * 400;
    let month = month as i32;
    let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day as i32 - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    Some((era * 146_097 + day_of_era - 719_468) as i64)
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

#[cfg(test)]
mod tests;
