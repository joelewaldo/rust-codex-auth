use std::time::{SystemTime, UNIX_EPOCH};

use super::UsageWindow;

pub fn format_remaining(window: Option<&UsageWindow>) -> String {
    match window {
        Some(window) => format!("{}%", window.remaining_percent()),
        None => "-".to_string(),
    }
}

pub fn format_reset(window: Option<&UsageWindow>) -> String {
    let Some(reset_at) = window.and_then(|window| window.reset_at) else {
        return "-".to_string();
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0);
    format_reset_from(now, reset_at)
}

pub fn format_reset_from(now: i64, reset_at: i64) -> String {
    if reset_at <= now {
        return "now".to_string();
    }
    let total_minutes = ((reset_at - now) + 59) / 60;
    let days = total_minutes / (24 * 60);
    let hours = (total_minutes % (24 * 60)) / 60;
    let minutes = total_minutes % 60;
    match (days, hours, minutes) {
        (0, 0, minutes) => format!("{minutes}m"),
        (0, hours, minutes) => format!("{hours}h {minutes}m"),
        (days, hours, _) => format!("{days}d {hours}h"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_relative_reset_times() {
        assert_eq!(format_reset_from(1_000, 1_000), "now");
        assert_eq!(format_reset_from(1_000, 1_060), "1m");
        assert_eq!(format_reset_from(1_000, 8_260), "2h 1m");
        assert_eq!(format_reset_from(1_000, 177_400), "2d 1h");
    }
}
