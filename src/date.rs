use chrono::{Local, TimeZone};

pub fn format_date(unix_secs: i64) -> Option<String> {
    Local
        .timestamp_opt(unix_secs, 0)
        .single()
        .map(|dt| dt.format("%Y-%m-%d").to_string())
}

pub fn format_datetime(unix_secs: i64) -> String {
    Local
        .timestamp_opt(unix_secs, 0)
        .single()
        .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| "unknown".to_string())
}
