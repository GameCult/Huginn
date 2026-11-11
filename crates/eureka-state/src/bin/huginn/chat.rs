//! The `huginn chat` verbs' arguments: one instant parser for `--after` and
//! `--before`. Display and the request are in `main` and `render`.

use chrono::{DateTime, SecondsFormat, Utc};

use crate::trouble::Trouble;

/// An instant as Unix milliseconds: an RFC 3339 time (`2026-11-11T10:00:00Z`)
/// or an age before `now` (`90s`, `15m`, `6h`, `2d`, `1w`). The one parse of
/// both flags.
pub fn instant_ms(flag: &str, text: &str, now: DateTime<Utc>) -> Result<u64, Trouble> {
    let wrong = || Trouble::invalid(format!("{flag} is an RFC 3339 time or an age such as 6h or 2d, not `{text}`"));
    let moment = if let Ok(time) = DateTime::parse_from_rfc3339(text) {
        time.with_timezone(&Utc)
    } else {
        let (digits, unit) = text.split_at(text.len().saturating_sub(1));
        let count: i64 = digits.parse().map_err(|_| wrong())?;
        let seconds = match unit {
            "s" => 1,
            "m" => 60,
            "h" => 3600,
            "d" => 86_400,
            "w" => 604_800,
            _ => return Err(wrong()),
        };
        now - chrono::Duration::try_seconds(count.checked_mul(seconds).ok_or_else(wrong)?).ok_or_else(wrong)?
    };
    u64::try_from(moment.timestamp_millis()).map_err(|_| wrong())
}

/// An instant as the CLI prints it.
pub fn shown(ms: u64) -> String {
    DateTime::from_timestamp_millis(i64::try_from(ms).unwrap_or(i64::MAX))
        .map_or_else(|| ms.to_string(), |time| time.to_rfc3339_opts(SecondsFormat::Millis, true))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_instant_is_rfc3339_or_an_age_before_now() {
        let now = DateTime::parse_from_rfc3339("2026-11-11T12:00:00Z").unwrap().with_timezone(&Utc);
        let at = |text: &str| instant_ms("--after", text, now).map(shown);
        assert_eq!(at("2026-11-11T10:00:00Z").unwrap(), "2026-11-11T10:00:00.000Z");
        assert_eq!(at("2026-11-11T12:00:00+02:00").unwrap(), "2026-11-11T10:00:00.000Z");
        assert_eq!(at("6h").unwrap(), "2026-11-11T06:00:00.000Z");
        assert_eq!(at("2d").unwrap(), "2026-11-09T12:00:00.000Z");
        assert_eq!(at("90s").unwrap(), "2026-11-11T11:58:30.000Z");
        for bad in ["", "h", "6", "6x", "-2d", "yesterday", "1e3h"] {
            assert!(matches!(at(bad), Err(Trouble::InvalidInput(ref detail)) if detail.contains("--after")), "{bad}");
        }
    }
}
