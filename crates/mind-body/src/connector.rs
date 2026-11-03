//! The plan windows as one line for a Mind: the one rendering both organs put in front of their
//! sessions, from the windows a credential lease granted (`lease_client`).

use chrono::{DateTime, SecondsFormat, Utc};
use model_connector::PlanQuota;

/// When a window resets: the UTC time alone. An instant is true when read again hours later; "in 4d 11h"
/// or "already past" is not.
fn resets(at_unix_ms: u64) -> String {
    let at = i64::try_from(at_unix_ms).ok().and_then(DateTime::<Utc>::from_timestamp_millis).map_or_else(|| at_unix_ms.to_string(), |at| at.to_rfc3339_opts(SecondsFormat::Secs, true));
    format!("resets at {at}")
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
pub fn render_plan(windows: &[PlanQuota], _now_ms: u64) -> String {
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
                resets(plan.resets_at_unix_ms),
                read_at(plan.observed_at_unix_ms)
            ),
            None => format!("{} expired, {}", plan.window, resets(plan.resets_at_unix_ms)),
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
    fn a_window_reads_as_a_time_not_unix_milliseconds() {
        let now = 1_793_307_000_000 - (4 * 86_400 + 11 * 3600 + 30 * 60) * 1000;
        let line = render_plan(&[window(Some(423), 1_793_307_000_000, now - 385_639_000)], now);
        assert!(line.starts_with("Plan usage: five_hour 42.3% allowed, resets at 2026-10-29T20:50:00Z, read at 2026-10-20T22:12Z. "), "{line}");
        assert!(!line.contains("unix ms"), "{line}");
    }

    #[test]
fn the_reset_is_an_instant_whatever_the_moment_of_rendering() {        let reset = 1_793_307_000_000;        let at = |now| render_plan(&[window(Some(10), reset, reset - 5000)], now);        let before = at(reset - 5000);        assert!(before.contains("resets at 2026-10-29T20:50:00Z, read at 2026-10-29T20:49Z"), "{before}");        for now in [reset - 12_000_000, reset, reset + 1000] {            assert_eq!(at(now), before, "the line does not depend on when it is rendered");        }        assert!(!before.contains("(in ") && !before.contains("already past"), "{before}");        assert_eq!(render_plan(&[window(None, reset, reset)], reset - 12_000_000).split(". ").next().unwrap(), "Plan usage: five_hour expired, resets at 2026-10-29T20:50:00Z");    }
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
