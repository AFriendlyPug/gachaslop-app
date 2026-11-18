//! Time as Unix seconds (UTC), and the little calendar maths the app needs. Everything is UTC here; the
//! popup shows it in the PC's own time zone.
use std::time::{SystemTime, UNIX_EPOCH};

/// A moment, in seconds since 1970-01-01 00:00 UTC.
pub type Ts = i64;

pub const MINUTE: i64 = 60;
pub const HOUR: i64 = 60 * MINUTE;
pub const DAY: i64 = 24 * HOUR;

pub fn now() -> Ts {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

pub fn now_millis() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

/// Days since 1970-01-01 for a (proleptic Gregorian) date. Howard Hinnant's days_from_civil.
pub fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let (m, d) = (month as i64, day as i64);
    let y = if m <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The date `days` after 1970-01-01, as (year, month, day).
pub fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + if month <= 2 { 1 } else { 0 };
    (year, month, day)
}

/// Day of the week in UTC: 0 = Monday .. 6 = Sunday (1970-01-01 was a Thursday).
pub fn weekday(t: Ts) -> u32 {
    (t.div_euclid(DAY) + 3).rem_euclid(7) as u32
}

/// "2026-11" for counting requests per month.
pub fn month(t: Ts) -> String {
    let (y, m, _) = civil_from_days(t.div_euclid(DAY));
    format!("{y:04}-{m:02}")
}

/// Parse "YYYY-MM-DDTHH:MM:SS", with optional fractional seconds and an optional "Z" or "+HH:MM" offset
/// (none means UTC).
pub fn parse_iso(text: &str) -> Option<Ts> {
    let b = text.trim().as_bytes();
    let num = |from: usize, to: usize| -> Option<i64> {
        let digits = b.get(from..to)?;
        digits.iter().all(u8::is_ascii_digit).then(|| std::str::from_utf8(digits).ok()?.parse().ok())?
    };
    if b.len() < 19 || b[4] != b'-' || b[7] != b'-' || !matches!(b[10], b'T' | b't' | b' ') || b[13] != b':' || b[16] != b':' {
        return None;
    }
    let (year, month, day) = (num(0, 4)?, num(5, 7)? as u32, num(8, 10)? as u32);
    let (hour, minute, second) = (num(11, 13)?, num(14, 16)?, num(17, 19)?);
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    let mut rest = &b[19..];
    if rest.first() == Some(&b'.') {
        let digits = rest[1..].iter().take_while(|c| c.is_ascii_digit()).count();
        if digits == 0 {
            return None;
        }
        rest = &rest[1 + digits..];
    }
    let offset = match rest {
        [] | [b'Z' | b'z'] => 0,
        [sign @ (b'+' | b'-'), h1, h2, rest @ ..] => {
            let mins = match rest {
                [b':', m1, m2] | [m1, m2] => [*m1, *m2],
                _ => return None,
            };
            let part = |pair: [u8; 2]| -> Option<i64> {
                pair.iter().all(u8::is_ascii_digit).then(|| ((pair[0] - b'0') * 10 + pair[1] - b'0') as i64)
            };
            let total = part([*h1, *h2])? * HOUR + part(mins)? * MINUTE;
            if *sign == b'-' { -total } else { total }
        }
        _ => return None,
    };
    let days = days_from_civil(year, month, day);
    if civil_from_days(days) != (year, month, day) {
        return None; // e.g. 31 February
    }
    Some(days * DAY + hour * HOUR + minute * MINUTE + second - offset)
}

/// "2026-11-18T19:00:00Z", for logs.
pub fn fmt_iso(t: Ts) -> String {
    let (y, m, d) = civil_from_days(t.div_euclid(DAY));
    let s = t.rem_euclid(DAY);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", s / HOUR, s / MINUTE % 60, s % MINUTE)
}

/// A countdown clock: "02:15:07", or "1d 02:15:07" from a day up (never negative).
pub fn fmt_clock(seconds: i64) -> String {
    let s = seconds.max(0);
    let clock = format!("{:02}:{:02}:{:02}", s / HOUR % 24, s / MINUTE % 60, s % MINUTE);
    if s >= DAY { format!("{}d {clock}", s / DAY) } else { clock }
}

/// "45m", "2h 15m" or "3d 4h", rounded up to the minute.
pub fn fmt_duration(seconds: i64) -> String {
    let minutes = (seconds + 59).div_euclid(MINUTE).max(1);
    let (days, hours, minutes) = (minutes / 1440, minutes / 60 % 24, minutes % 60);
    match (days, hours) {
        (0, 0) => format!("{minutes}m"),
        (0, _) => format!("{hours}h {minutes}m"),
        _ => format!("{days}d {hours}h"),
    }
}
