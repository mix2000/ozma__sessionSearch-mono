//! Timestamps: milliseconds in storage, JS-style ISO strings in the API.

use chrono::{DateTime, SecondsFormat, TimeZone, Utc};

pub fn to_ms(d: Option<DateTime<Utc>>) -> Option<i64> {
    d.map(|d| d.timestamp_millis())
}

/// ISO string exactly like JavaScript `Date#toISOString`.
pub fn iso(ms: i64) -> String {
    Utc.timestamp_millis_opt(ms)
        .single()
        .map(|d| d.to_rfc3339_opts(SecondsFormat::Millis, true))
        .unwrap_or_default()
}

pub fn iso_opt(ms: Option<i64>) -> Option<String> {
    ms.map(iso)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_matches_js() {
        let d: DateTime<Utc> = "2026-11-10T08:55:37.856Z".parse().unwrap();
        assert_eq!(iso(d.timestamp_millis()), "2026-11-10T08:55:37.856Z");
        assert_eq!(iso(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(to_ms(Some(d)), Some(d.timestamp_millis()));
        assert_eq!(iso_opt(None), None);
    }
}
