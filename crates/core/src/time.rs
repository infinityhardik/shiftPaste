//! Timestamps are stored as UTC Unix milliseconds and only formatted for display.

use chrono::{DateTime, Datelike, Local, TimeZone};

pub const SECOND: i64 = 1_000;
pub const MINUTE: i64 = 60 * SECOND;
pub const HOUR: i64 = 60 * MINUTE;
pub const DAY: i64 = 24 * HOUR;

/// Current time in UTC Unix milliseconds.
pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

fn local(ts_ms: i64) -> Option<DateTime<Local>> {
    Local.timestamp_millis_opt(ts_ms).single()
}

/// Short relative time for list rows: "Just now", "5 min ago", "3 h ago",
/// "Yesterday", "4 days ago", then a date ("12 Mar", or "12 Mar 2024" for other years).
pub fn time_ago(ts_ms: i64, now_ms: i64) -> String {
    let diff = now_ms - ts_ms;
    if diff < MINUTE {
        return "Just now".into();
    }
    if diff < HOUR {
        return format!("{} min ago", diff / MINUTE);
    }
    if diff < DAY {
        return format!("{} h ago", diff / HOUR);
    }
    let (Some(then), Some(now)) = (local(ts_ms), local(now_ms)) else {
        return String::new();
    };
    let days = (now.date_naive() - then.date_naive()).num_days();
    match days {
        ..=1 => "Yesterday".into(),
        2..=6 => format!("{days} days ago"),
        _ if then.year() == now.year() => then.format("%-d %b").to_string(),
        _ => then.format("%-d %b %Y").to_string(),
    }
}

/// Local date-time for exports, e.g. "2026-11-16 09:54".
pub fn format_local(ts_ms: i64) -> String {
    local(ts_ms)
        .map(|d| d.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_times() {
        let now = 1_763_280_000_000; // a fixed instant
        assert_eq!(time_ago(now - 5 * SECOND, now), "Just now");
        assert_eq!(time_ago(now - 5 * MINUTE, now), "5 min ago");
        assert_eq!(time_ago(now - 3 * HOUR, now), "3 h ago");
        assert_eq!(time_ago(now - 4 * DAY, now), "4 days ago");
        assert!(!time_ago(now - 40 * DAY, now).is_empty());
        // Clock skew: future timestamps read as "Just now", never negative.
        assert_eq!(time_ago(now + 10 * MINUTE, now), "Just now");
    }
}
