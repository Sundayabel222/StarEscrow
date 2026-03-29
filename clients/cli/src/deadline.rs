/// Deadline timestamp utilities for the StarEscrow CLI.
///
/// Soroban stores deadlines as Unix timestamps (u64 seconds since epoch).
/// This module handles:
///   - Parsing ISO 8601 strings into Unix timestamps
///   - Formatting Unix timestamps as human-readable ISO 8601 strings
///   - Validation (deadline must be in the future)
use anyhow::{Context, Result};
use chrono::{DateTime, TimeZone, Utc};

/// Parse an ISO 8601 datetime string (e.g. "2026-12-31T23:59:59Z") into a
/// Unix timestamp (seconds since epoch) suitable for use as a ledger deadline.
///
/// Accepts any RFC 3339 / ISO 8601 string that `chrono` can parse, including
/// timezone offsets. The result is always UTC epoch seconds.
pub fn parse_iso8601_to_timestamp(s: &str) -> Result<u64> {
    let dt = DateTime::parse_from_rfc3339(s)
        .with_context(|| format!("invalid ISO 8601 datetime: '{s}' — expected format: 2026-12-31T23:59:59Z"))?;
    let ts = dt.timestamp();
    if ts < 0 {
        anyhow::bail!("deadline must be after Unix epoch (1970-01-01T00:00:00Z)");
    }
    Ok(ts as u64)
}

/// Format a Unix timestamp (u64 seconds) as an ISO 8601 / RFC 3339 string in UTC.
/// Example output: "2026-12-31T23:59:59Z"
pub fn format_timestamp(ts: u64) -> String {
    Utc.timestamp_opt(ts as i64, 0)
        .single()
        .map(|dt| dt.format("%Y-%m-%dT%H:%M:%SZ").to_string())
        .unwrap_or_else(|| format!("<invalid timestamp: {ts}>"))
}

/// Return `true` if the given Unix timestamp is strictly in the future
/// relative to the current UTC time.
pub fn is_future_timestamp(ts: u64) -> bool {
    let now = Utc::now().timestamp();
    (ts as i64) > now
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_iso8601_to_timestamp ──────────────────────────────────────────

    #[test]
    fn test_parse_utc_zulu() {
        let ts = parse_iso8601_to_timestamp("2026-12-31T23:59:59Z").expect("parse");
        assert_eq!(ts, 1_798_761_599);
    }

    #[test]
    fn test_parse_with_positive_offset() {
        // +02:00 means 2 hours ahead of UTC, so UTC time is 21:59:59
        let ts = parse_iso8601_to_timestamp("2026-12-31T23:59:59+02:00").expect("parse offset");
        assert_eq!(ts, 1_798_754_399); // 1_798_761_599 - 7200
    }

    #[test]
    fn test_parse_epoch_zero() {
        let ts = parse_iso8601_to_timestamp("1970-01-01T00:00:00Z").expect("epoch");
        assert_eq!(ts, 0);
    }

    #[test]
    fn test_parse_invalid_string_returns_error() {
        assert!(parse_iso8601_to_timestamp("not-a-date").is_err());
        assert!(parse_iso8601_to_timestamp("2026/12/31").is_err());
        assert!(parse_iso8601_to_timestamp("").is_err());
    }

    // ── format_timestamp ───────────────────────────────────────────────────

    #[test]
    fn test_format_epoch_zero() {
        assert_eq!(format_timestamp(0), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn test_format_known_timestamp() {
        assert_eq!(format_timestamp(1_798_761_599), "2026-12-31T23:59:59Z");
    }

    #[test]
    fn test_format_output_is_iso8601() {
        let s = format_timestamp(1_700_000_000);
        // Must be parseable back by chrono
        assert!(DateTime::parse_from_rfc3339(&s).is_ok(), "output must be valid ISO 8601: {s}");
    }

    // ── roundtrip ──────────────────────────────────────────────────────────

    #[test]
    fn test_roundtrip_parse_then_format() {
        let original = "2027-06-15T12:00:00Z";
        let ts = parse_iso8601_to_timestamp(original).expect("parse");
        let formatted = format_timestamp(ts);
        assert_eq!(formatted, original);
    }

    #[test]
    fn test_roundtrip_format_then_parse() {
        let ts: u64 = 1_800_000_000;
        let formatted = format_timestamp(ts);
        let parsed = parse_iso8601_to_timestamp(&formatted).expect("parse back");
        assert_eq!(parsed, ts);
    }

    // ── is_future_timestamp ────────────────────────────────────────────────

    #[test]
    fn test_past_timestamp_is_not_future() {
        assert!(!is_future_timestamp(0), "epoch is not in the future");
        assert!(!is_future_timestamp(1_000_000_000), "year 2001 is not in the future");
    }

    #[test]
    fn test_far_future_timestamp_is_future() {
        // Year 2100
        assert!(is_future_timestamp(4_102_444_800));
    }
}
