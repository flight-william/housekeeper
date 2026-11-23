//! A calendar day for the excuse list's deadlines.

use std::fmt;

/// A calendar day. `today` is always passed in; nothing here reads the clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Date {
    year: u16,
    month: u8,
    day: u8,
}

impl Date {
    /// A real `YYYY-MM-DD` date, or `None`.
    pub fn parse(text: &str) -> Option<Date> {
        let b = text.as_bytes();
        let digits = |r: std::ops::Range<usize>| b[r].iter().all(u8::is_ascii_digit);
        if b.len() != 10 || b[4] != b'-' || b[7] != b'-' || !digits(0..4) {
            return None;
        }
        if !digits(5..7) || !digits(8..10) {
            return None;
        }
        let year: u16 = text[0..4].parse().ok()?;
        let month: u8 = text[5..7].parse().ok()?;
        let day: u8 = text[8..10].parse().ok()?;
        let leap =
            year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
        let last = match month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 if leap => 29,
            2 => 28,
            _ => return None,
        };
        (1..=last)
            .contains(&day)
            .then_some(Date { year, month, day })
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

impl Date {
    /// The day `n` days after this one (before it when `n` is negative).
    pub fn add_days(self, n: i64) -> Option<Date> {
        let (y, m, d) = crate::record::civil(self.days() + n);
        Date::parse(&format!("{y:04}-{m:02}-{d:02}"))
    }

    /// Days since 1970-01-01 (civil calendar), so dates subtract.
    pub(crate) fn days(self) -> i64 {
        let (y, m, d) = (
            i64::from(self.year),
            i64::from(self.month),
            i64::from(self.day),
        );
        let y = if m <= 2 { y - 1 } else { y };
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    }
}
