use chrono::{DateTime, Utc, Duration};

/// Get current UTC timestamp
pub fn now() -> DateTime<Utc> {
    Utc::now()
}

/// Format timestamp for logging
pub fn format_timestamp(dt: DateTime<Utc>) -> String {
    dt.format("%Y-%m-%d %H:%M:%S UTC").to_string()
}

/// Add duration to timestamp
pub fn add_duration(dt: DateTime<Utc>, duration: Duration) -> DateTime<Utc> {
    dt + duration
}

/// Check if timestamp is expired
pub fn is_expired(dt: DateTime<Utc>) -> bool {
    dt < Utc::now()
}

/// Create duration from seconds
pub fn duration_from_seconds(seconds: i64) -> Duration {
    Duration::seconds(seconds)
}

/// Create duration from minutes
pub fn duration_from_minutes(minutes: i64) -> Duration {
    Duration::minutes(minutes)
}

/// Create duration from hours
pub fn duration_from_hours(hours: i64) -> Duration {
    Duration::hours(hours)
}
