//! Time zones for custom games: daylight saving dates, local to UTC, and resets that follow the clocks.
use super::utc;
use crate::games::{last_reset, next_reset, next_weekly, CustomGame};
use crate::time::{days_from_civil, HOUR};
use crate::zones::*;

/// The UK's rule (as Windows gives it for "GMT Standard Time"): summer time from 01:00 on the last Sunday
/// of March to 02:00 on the last Sunday of October.
fn uk() -> Rule {
    Rule { bias: 0, standard_bias: 0, daylight: Some(Daylight {
        starts: Transition { month: 3, week: 5, weekday: 0, hour: 1, minute: 0 },
        ends: Transition { month: 10, week: 5, weekday: 0, hour: 2, minute: 0 },
        bias: -60,
    }) }
}

/// Sydney's: UTC+10, summer time from the first Sunday of October to the first Sunday of April.
fn sydney() -> Rule {
    Rule { bias: -600, standard_bias: 0, daylight: Some(Daylight {
        starts: Transition { month: 10, week: 1, weekday: 0, hour: 2, minute: 0 },
        ends: Transition { month: 4, week: 1, weekday: 0, hour: 3, minute: 0 },
        bias: -60,
    }) }
}

#[test]
fn daylight_saving_dates_are_found() {
    let uk = uk().daylight.unwrap();
    assert_eq!(transition_day(2026, uk.starts), days_from_civil(2026, 3, 29)); // last Sunday of March
    assert_eq!(transition_day(2026, uk.ends), days_from_civil(2026, 10, 25));
    let sydney = sydney().daylight.unwrap();
    assert_eq!(transition_day(2026, sydney.starts), days_from_civil(2026, 10, 4)); // first Sunday of October
    assert_eq!(transition_day(2026, sydney.ends), days_from_civil(2026, 4, 5));
}

#[test]
fn local_times_become_utc_with_daylight_saving() {
    let local = |m, d, h| utc(2026, m, d, h, 0); // a wall-clock time, written as if it were UTC
    assert_eq!(to_utc(&uk(), 2026, local(1, 15, 4)), utc(2026, 1, 15, 4, 0)); // winter: UTC
    assert_eq!(to_utc(&uk(), 2026, local(7, 15, 4)), utc(2026, 7, 15, 3, 0)); // summer: UTC+1
    assert_eq!(to_utc(&sydney(), 2026, local(1, 15, 4)), utc(2026, 1, 14, 17, 0)); // southern summer: UTC+11
    assert_eq!(to_utc(&sydney(), 2026, local(7, 15, 4)), utc(2026, 7, 14, 18, 0)); // southern winter: UTC+10
    let none = Rule { bias: -540, standard_bias: 0, daylight: None }; // Tokyo: UTC+9 all year
    assert_eq!(to_utc(&none, 2026, local(7, 15, 4)), utc(2026, 7, 14, 19, 0));
}

#[test]
fn a_game_in_a_time_zone_follows_the_clock_change() {
    remember("Test UK", 2026, Some(uk()));
    let game = CustomGame { id: "custom-9".into(), name: "Test".into(), icon: None, hour: 4, minute: 0,
                            weekly_day: Some(0), zone: Some("Test UK".into()), offset_minutes: 0 }.to_game();
    // 04:00 UK time is 03:00 UTC in summer time; the clocks go back at 02:00 on 25 October, so that day's
    // reset is already 04:00 UTC
    assert_eq!(next_reset(&game, utc(2026, 10, 23, 12, 0)), utc(2026, 10, 24, 3, 0));
    assert_eq!(next_reset(&game, utc(2026, 10, 24, 12, 0)), utc(2026, 10, 25, 4, 0));
    assert_eq!(last_reset(&game, utc(2026, 10, 25, 3, 30)), utc(2026, 10, 24, 3, 0));
    // the weekly reset is on Monday in UK time
    assert_eq!(next_weekly(&game, utc(2026, 10, 21, 12, 0)), Some(utc(2026, 10, 26, 4, 0)));
    assert_eq!(next_reset(&game, utc(2026, 10, 26, 4, 0) - 1), utc(2026, 10, 26, 4, 0));
    assert_eq!(next_reset(&game, utc(2026, 10, 26, 4, 0)) - utc(2026, 10, 26, 4, 0), 24 * HOUR);
}

#[test]
fn an_unknown_zone_is_treated_as_utc() {
    remember("Test Nowhere", 2026, None);
    let game = CustomGame { id: "custom-8".into(), name: "Test".into(), icon: None, hour: 4, minute: 0,
                            weekly_day: None, zone: Some("Test Nowhere".into()), offset_minutes: 0 }.to_game();
    assert_eq!(next_reset(&game, utc(2026, 7, 1, 12, 0)), utc(2026, 7, 2, 4, 0));
}

#[cfg(windows)]
#[test]
fn windows_knows_the_uk_rule() {
    assert!(exists("GMT Standard Time"));
    assert!(!exists("Not A Zone"));
    let zones = list();
    let uk_zone = zones.iter().find(|z| z.id == "GMT Standard Time").expect("listed");
    assert!(uk_zone.name.contains("London"), "{}", uk_zone.name);
    let summer = local_to_utc("GMT Standard Time", days_from_civil(2026, 7, 15), 4, 0);
    assert_eq!(summer, utc(2026, 7, 15, 3, 0));
    let winter = local_to_utc("GMT Standard Time", days_from_civil(2026, 1, 15), 4, 0);
    assert_eq!(winter, utc(2026, 1, 15, 4, 0));
}
