//! Unix-time calendar maths, ISO parsing and duration text.
use super::utc;
use crate::time::*;

#[test]
fn civil_dates_round_trip_from_1970_to_2100() {
    for days in 0..days_from_civil(2100, 1, 1) {
        let (y, m, d) = civil_from_days(days);
        assert_eq!(days_from_civil(y, m, d), days);
    }
    assert_eq!(civil_from_days(0), (1970, 1, 1));
    assert_eq!(civil_from_days(days_from_civil(2028, 2, 29)), (2028, 2, 29));
}

#[test]
fn weekdays_are_monday_based_in_utc() {
    assert_eq!(weekday(0), 3); // 1970-01-01, a Thursday
    assert_eq!(weekday(utc(2026, 11, 16, 0, 0)), 0); // Monday
    assert_eq!(weekday(utc(2026, 11, 15, 23, 59)), 6); // Sunday
    assert_eq!(weekday(utc(1969, 12, 31, 12, 0)), 2); // before 1970 too
}

#[test]
fn iso_dates_parse_with_and_without_offsets() {
    let at = utc(2026, 11, 17, 2, 0);
    for text in ["2026-11-17T02:00:00Z", "2026-11-17T02:00:00.000Z", "2026-11-17T02:00:00", "2026-11-17 02:00:00",
                 "2026-11-17T11:00:00+09:00", "2026-11-17T11:00:00+0900", "2026-11-16T21:00:00-05:00",
                 " 2026-11-17T02:00:00.123456Z "] {
        assert_eq!(parse_iso(text), Some(at), "{text}");
    }
    assert_eq!(parse_iso("2026-11-24T01:59:59.000Z"), Some(utc(2026, 11, 24, 1, 59) + 59));
}

#[test]
fn bad_iso_dates_are_rejected() {
    for text in ["", "2026-11-17", "2026-13-01T00:00:00Z", "2026-02-31T00:00:00Z", "2026-11-17T24:00:00Z",
                 "2026-11-17T02:00:00.Z", "2026-11-17T02:00:00+9", "2026-11-17T02:00:00Q", "2026-1a-17T02:00:00Z",
                 "+026-11-17T02:00:00Z"] {
        assert_eq!(parse_iso(text), None, "{text}");
    }
}

#[test]
fn iso_formatting_and_months() {
    assert_eq!(fmt_iso(utc(2026, 11, 18, 19, 5) + 7), "2026-11-18T19:05:07Z");
    assert_eq!(parse_iso(&fmt_iso(1_794_369_600)), Some(1_794_369_600));
    assert_eq!(month(utc(2026, 11, 30, 23, 59)), "2026-11");
    assert_eq!(month(utc(2026, 12, 1, 0, 0)), "2026-12");
}

#[test]
fn fmt_duration_rounds_up_to_the_minute() {
    assert_eq!(fmt_duration(1), "1m");
    assert_eq!(fmt_duration(0), "1m");
    assert_eq!(fmt_duration(45 * MINUTE), "45m");
    assert_eq!(fmt_duration(2 * HOUR + 14 * MINUTE + 1), "2h 15m");
    assert_eq!(fmt_duration(3 * DAY + 4 * HOUR), "3d 4h");
}
