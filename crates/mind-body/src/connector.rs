//! The plan windows as one line for a Mind: the one rendering both organs put in front of their
//! sessions, from the windows a credential lease granted (`lease_client`).

use chrono::{DateTime, SecondsFormat, Utc};
use model_connector::PlanQuota;

/// A span in the two largest units that fit: `45s`, `12m`, `3h 20m`, `4d 11h`.
fn span(ms: u64) -> String {
    let seconds = ms / 1000;
    match seconds {
        0..=59 => format!("{seconds}s"),
        60..=3599 => format!("{}m", seconds / 60),
        3600..=86399 => format!("{}h {}m", seconds / 3600, seconds % 3600 / 60),
        _ => format!("{}d {}h", seconds / 86400, seconds % 86400 / 3600),
    }
}

/// When a window resets: the UTC time, and how far off it is from `now_ms`.
fn resets(at_unix_ms: u64, now_ms: u64) -> String {
    let at = i64::try_from(at_unix_ms).ok().and_then(DateTime::<Utc>::from_timestamp_millis).map_or_else(|| at_unix_ms.to_string(), |at| at.to_rfc3339_opts(SecondsFormat::Secs, true));
    if at_unix_ms > now_ms { format!("resets at {at} (in {})", span(at_unix_ms - now_ms)) } else { format!("resets at {at} (already past)") }
}

/// The minute a reading was taken, as a UTC time. An instant, not an age: the line stays true when
/// it is read again hours later in a session that cannot refresh it.
fn read_at(at_unix_ms: u64) -> String {
    i64::try_from(at_unix_ms).ok().and_then(DateTime::<Utc>::from_timestamp_millis).map_or_else(|| at_unix_ms.to_string(), |at| at.format("%Y-%m-%dT%H:%MZ").to_string())
}

/// The plan windows as one line for a Mind: each window's percent used, its
/// status, when it resets and the minute the reading was taken. Rendered from the readout
/// and nothing else, so no number in it is ever a paraphrase; the guidance that
/// follows it states the swarm's aim and that nothing enforces it.
pub fn render_plan(windows: &[PlanQuota], now_ms: u64) -> String {
    let guidance = "The swarm aims to have used about 90% of the weekly allowance at reset; nothing enforces it; you and the other Minds set the tempo.";
    if windows.is_empty() {
        return format!("Plan usage: unobserved. {guidance}");
    }
    let rendered: Vec<String> = windows
        .iter()
        .map(|plan| match plan.utilization_permille {
            Some(permille) => format!(
                "{} {}.{}% {}, {}, read at {}",
                plan.window,
                permille / 10,
                permille % 10,
                plan.status,
                resets(plan.resets_at_unix_ms, now_ms),
                read_at(plan.observed_at_unix_ms)
            ),
            None => format!("{} expired, {}", plan.window, resets(plan.resets_at_unix_ms, now_ms)),
        })
        .collect();
    format!("Plan usage: {}. {guidance}", rendered.join("; "))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(permille: Option<u32>, resets_at_unix_ms: u64, observed_at_unix_ms: u64) -> PlanQuota {
        PlanQuota { window: "five_hour".into(), utilization_permille: permille, resets_at_unix_ms, status: "allowed".into(), observed_at_unix_ms }
    }

    #[test]
    fn a_window_reads_as_a_time_and_spans_not_unix_milliseconds() {
        let now = 1_793_307_000_000 - (4 * 86_400 + 11 * 3600 + 30 * 60) * 1000;
        let line = render_plan(&[window(Some(423), 1_793_307_000_000, now - 385_639_000)], now);
        assert!(line.starts_with("Plan usage: five_hour 42.3% allowed, resets at 2026-10-29T20:50:00Z (in 4d 11h), read at 2026-10-20T22:12Z. "), "{line}");
        assert!(!line.contains("unix ms"), "{line}");
    }

    #[test]
    fn spans_use_the_two_largest_units_and_a_passed_reset_says_so() {
        let reset = 1_793_307_000_000;
        assert!(render_plan(&[window(Some(10), reset, reset - 5000)], reset - 5000).contains("(in 5s), read at 2026-10-29T20:49Z"));
        assert!(render_plan(&[window(Some(10), reset, reset - 720_000)], reset - 600_000).contains("(in 10m), read at 2026-10-29T20:38Z"));
        assert!(render_plan(&[window(Some(10), reset, reset)], reset).contains("(already past), read at 2026-10-29T20:50Z"), "a reset at this very moment is past");
        assert!(render_plan(&[window(Some(10), reset, reset)], reset + 1000).contains("(already past), read at 2026-10-29T20:50Z"));
        assert!(render_plan(&[window(None, reset, reset)], reset - 12_000_000).contains("five_hour expired, resets at 2026-10-29T20:50:00Z (in 3h 20m)"));
    }
    #[test]
    fn the_reading_is_dated_by_the_minute_it_was_taken_and_the_line_does_not_age() {
        let reset = 1_793_307_000_000;
        let taken = 1_793_306_999_000; // 20:49:59Z: the minute is cut, not rounded
        let first = render_plan(&[window(Some(10), reset, taken)], taken);
        assert!(first.contains(", read at 2026-10-29T20:49Z. "), "{first}");
        let hours_later = render_plan(&[window(Some(10), reset, taken)], taken + 3 * 3_600_000);
        assert!(hours_later.contains(", read at 2026-10-29T20:49Z. "), "{hours_later}");
        assert!(!first.contains(" ago") && !hours_later.contains(" ago"), "{first} {hours_later}");
        assert!(render_plan(&[window(Some(10), reset, 0)], reset).contains(", read at 1970-01-01T00:00Z. "));
        assert!(render_plan(&[window(Some(10), reset, u64::MAX)], reset).contains(&format!(", read at {}. ", u64::MAX)), "an instant out of range is printed as the number it is");
    }
}
