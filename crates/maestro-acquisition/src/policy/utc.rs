//! One strict Gregorian UTC implementation for grants and transport times.
use crate::Refusal;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Parse the documented fixed-width UTC spelling without a time dependency.
pub(crate) fn parse(text: &str) -> Result<SystemTime, Refusal> {
    // The shared strict date decoder checks leap years, bounds and exact fields.
    super::shape::valid_time(text)
        .then_some(())
        .ok_or(Refusal::Invalid)?;
    let parts: Vec<u64> = text
        .split(['-', 'T', ':', 'Z'])
        .filter(|part| !part.is_empty())
        .map(str::parse)
        .collect::<Result<_, _>>()
        .map_err(|_| Refusal::Invalid)?;
    let [year, month, day, hour, minute, second] = parts.as_slice() else {
        return Err(Refusal::Invalid);
    };
    let mut days = 0;
    for previous in 0..*year {
        days += year_days(previous);
    }
    for previous in 1..*month {
        days += month_days(*year, previous);
    }
    days += day - 1;
    // Gregorian days from year zero to 1970-01-01, including year zero's leap day.
    let epoch = 719_528 * 86_400;
    let seconds = days * 86_400 + hour * 3600 + minute * 60 + second;
    if seconds < epoch {
        UNIX_EPOCH
            .checked_sub(Duration::from_secs(epoch - seconds))
            .ok_or(Refusal::Invalid)
    } else {
        UNIX_EPOCH
            .checked_add(Duration::from_secs(seconds - epoch))
            .ok_or(Refusal::Invalid)
    }
}
/// Number of days in a Gregorian year.
fn year_days(year: u64) -> u64 {
    if year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400)) {
        366
    } else {
        365
    }
}
/// Shared leap-year arithmetic, after the strict decoder validated the month.
fn month_days(year: u64, month: u64) -> u64 {
    match month {
        2 => year_days(year) - 337,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Advance a checked UTC policy timestamp by trusted monotonic elapsed time.
pub(crate) fn advance(text: &str, elapsed: Duration) -> Result<String, Refusal> {
    format(parse(text)?.checked_add(elapsed).ok_or(Refusal::Invalid)?)
}
/// Fixed-width Gregorian UTC spelling, with years 1601–9999 on every platform.
/// # Errors
/// Unrepresentable host time refuses instead of constructing a non-UTC request.
pub fn format(time: SystemTime) -> Result<String, Refusal> {
    let seconds = match time.duration_since(UNIX_EPOCH) {
        Ok(duration) => i128::from(duration.as_secs()),
        Err(error) => {
            -i128::from(error.duration().as_secs())
                - i128::from(error.duration().subsec_nanos() != 0)
        }
    } + 719_528 * 86_400;
    let seconds = u64::try_from(seconds).map_err(|_| Refusal::Invalid)?;
    let mut days = seconds / 86_400;
    let mut year = 0;
    while days >= year_days(year) {
        days -= year_days(year);
        year += 1;
        if year > 9999 {
            return Err(Refusal::Invalid);
        }
    }
    if year < u64::from(super::shape::MIN_YEAR) {
        return Err(Refusal::Invalid);
    }
    let mut month = 1;
    while days >= month_days(year, month) {
        days -= month_days(year, month);
        month += 1;
    }
    let within = seconds % 86_400;
    Ok(format!(
        "{year:04}-{month:02}-{:02}T{:02}:{:02}:{:02}Z",
        days + 1,
        within / 3600,
        within / 60 % 60,
        within % 60
    ))
}
/// Only delta-seconds and IMF-fixdate are accepted; unknown delay never shortens.
pub(crate) fn retry_after(text: &str, now: SystemTime) -> Result<u64, Refusal> {
    let delay = if !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit()) {
        Duration::from_secs(text.parse().map_err(|_| Refusal::Invalid)?)
    } else {
        let parts: Vec<_> = text.split(' ').collect();
        let [weekday, day, month, year, clock, zone] = parts.as_slice() else {
            return Err(Refusal::Invalid);
        };
        if !["Mon,", "Tue,", "Wed,", "Thu,", "Fri,", "Sat,", "Sun,"].contains(weekday)
            || *zone != "GMT"
            || day.len() != 2
            || year.len() != 4
        {
            return Err(Refusal::Invalid);
        }
        let month = [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ]
        .iter()
        .position(|name| name == month)
        .ok_or(Refusal::Invalid)?
            + 1;
        let date = parse(&format!("{year}-{month:02}-{day}T{clock}Z"))?;
        date.duration_since(now).unwrap_or(Duration::ZERO)
    };
    // Millisecond rounding goes upward: a fractional floor is never shortened.
    u64::try_from(delay.as_nanos().div_ceil(1_000_000)).map_err(|_| Refusal::Invalid)
}

#[cfg(test)]
mod tests {
    use super::{advance, format, parse, retry_after};
    use crate::Refusal;
    use std::time::{Duration, UNIX_EPOCH};

    #[test]
    fn n09_utc_round_trips_year_and_leap_boundaries() {
        for text in [
            "1601-01-01T00:00:00Z",
            "1969-12-31T23:59:59Z",
            "1970-01-01T00:00:00Z",
            "2000-02-29T23:59:59Z",
            "9999-12-31T23:59:59Z",
        ] {
            assert_eq!(format(parse(text).unwrap()).unwrap(), text);
        }
        assert_eq!(
            advance("2000-02-28T23:59:59Z", Duration::from_secs(1)).unwrap(),
            "2000-02-29T00:00:00Z"
        );
        assert_eq!(
            advance("2100-02-28T23:59:59Z", Duration::from_secs(1)).unwrap(),
            "2100-03-01T00:00:00Z"
        );
        assert_eq!(
            advance("9999-12-31T23:59:59Z", Duration::from_secs(1)),
            Err(Refusal::Invalid)
        );
        assert_eq!(
            // Windows SystemTime has 100 ns ticks, so use a portable fraction.
            format(UNIX_EPOCH - Duration::from_nanos(100)).unwrap(),
            "1969-12-31T23:59:59Z"
        );
    }
    #[test]
    fn n09_utc_refuses_dates_before_portable_range() {
        for text in [
            "0000-01-01T00:00:00Z",
            "1600-02-29T00:00:00Z",
            "1600-12-31T23:59:59Z",
            "1600-12-31T23:59:59.999Z",
        ] {
            assert!(!super::super::shape::valid_time(text));
            assert_eq!(parse(text), Err(Refusal::Invalid));
        }
        // Hosts able to construct earlier times must refuse formatting them too.
        let earliest = parse("1601-01-01T00:00:00Z").unwrap();
        if let Some(previous) = earliest.checked_sub(Duration::from_nanos(100)) {
            assert_eq!(format(previous), Err(Refusal::Invalid));
        }
        assert_eq!(
            retry_after("Sun, 31 Dec 1600 23:59:59 GMT", UNIX_EPOCH),
            Err(Refusal::Invalid)
        );
    }
    #[test]
    fn n09_retry_after_date_and_fractional_floor_never_shorten() {
        let now = parse("2026-09-30T20:00:00Z").unwrap() + Duration::from_millis(1);
        assert_eq!(retry_after("Wed, 30 Sep 2026 20:00:01 GMT", now), Ok(999));
        assert_eq!(retry_after("Wed, 30 Sep 2026 19:59:59 GMT", now), Ok(0));
        assert_eq!(retry_after("3", now), Ok(3000));
        for text in [
            "",
            "-1",
            "18446744073709551615",
            "invalid",
            "Wed Sep 30 20:00:01 2026",
            "Wednesday, 30-Sep-26 20:00:01 GMT",
            "Wed, 31 Feb 2026 20:00:01 GMT",
        ] {
            assert_eq!(retry_after(text, now), Err(Refusal::Invalid));
        }
    }
}
