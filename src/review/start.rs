//! A record's `start`, read as its day in UTC. house writes every start in UTC
//! (`2026-11-02T09:00:00Z`), and a period's days are UTC days; a start written with
//! another offset counts under the UTC day it falls on.

use crate::excuses::Date;

/// The UTC day of `start`: `YYYY-MM-DDTHH:MM:SS`, an optional fraction of a second,
/// then `Z` or `+HH:MM` / `-HH:MM`; a bare date is that day. `None` for anything
/// else, so the line is not a house record.
pub fn utc_day(start: &str) -> Option<Date> {
    let day = Date::parse(start.get(..10)?)?;
    let Some(rest) = start.get(10..).filter(|r| !r.is_empty()) else {
        return Some(day);
    };
    let time = rest.strip_prefix('T')?;
    let (hour, minute, second) = (two(time, 0)?, two(time, 3)?, two(time, 6)?);
    let colons = time.get(2..3) == Some(":") && time.get(5..6) == Some(":");
    if !colons || hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    let mut zone = time.get(8..)?;
    if let Some(fraction) = zone.strip_prefix('.') {
        let digits = fraction.bytes().take_while(u8::is_ascii_digit).count();
        if digits == 0 {
            return None;
        }
        zone = &fraction[digits..];
    }
    let minutes = i64::from(hour) * 60 + i64::from(minute) - offset_minutes(zone)?;
    day.add_days(minutes.div_euclid(24 * 60))
}

/// `Z` is 0; `+05:30` is 330 and `-10:00` is -600.
fn offset_minutes(zone: &str) -> Option<i64> {
    if zone == "Z" {
        return Some(0);
    }
    let sign = match zone.get(..1)? {
        "+" => 1,
        "-" => -1,
        _ => return None,
    };
    let (hours, minutes) = (two(zone, 1)?, two(zone, 4)?);
    if zone.len() != 6 || zone.get(3..4) != Some(":") || hours > 23 || minutes > 59 {
        return None;
    }
    Some(sign * (i64::from(hours) * 60 + i64::from(minutes)))
}

/// The two ASCII digits at `at` in `text`.
fn two(text: &str, at: usize) -> Option<u8> {
    let digits = text.get(at..at + 2)?;
    if digits.bytes().all(|b| b.is_ascii_digit()) {
        digits.parse().ok()
    } else {
        None
    }
}
