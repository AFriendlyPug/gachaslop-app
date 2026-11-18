//! Reset times (all UTC) and the date maths behind them.
use super::utc;
use crate::games::*;
use crate::time::{weekday, Ts, DAY, MINUTE};

fn game(key: &str) -> Game {
    built_in().into_iter().find(|g| g.key == key).unwrap()
}

fn custom(hour: u32, minute: u32, weekly_day: Option<u32>) -> Game {
    CustomGame { id: "custom-1".into(), name: "Test".into(), icon: None, hour, minute, weekly_day, zone: None, offset_minutes: 0 }
        .to_game()
}

/// The straightforward weekly algorithm: next_weekly must always agree with it.
fn simple_next_weekly(game: &Game, after: Ts) -> Ts {
    let day = after.div_euclid(DAY);
    let ahead = (game.weekly_day.unwrap() + 7 - weekday(after)) % 7;
    let mut candidate = (day + ahead as i64) * DAY + (game.hour * 60 + game.minute) as i64 * MINUTE;
    if candidate <= after {
        candidate += 7 * DAY;
    }
    candidate
}

#[test]
fn built_in_resets_are_in_utc() {
    let ba = game("ba");
    let zzz = game("zzz");
    assert_eq!(next_reset(&ba, utc(2026, 11, 18, 12, 0)), utc(2026, 11, 18, 19, 0));
    // ZZZ EU resets at 04:00 server time (UTC+1, no daylight saving): 03:00 UTC
    assert_eq!(next_reset(&zzz, utc(2026, 11, 18, 12, 0)), utc(2026, 11, 19, 3, 0));
    assert_eq!(next_weekly(&ba, utc(2026, 11, 18, 12, 0)), Some(utc(2026, 11, 22, 19, 0))); // Sunday
    assert_eq!(next_weekly(&zzz, utc(2026, 11, 18, 12, 0)), Some(utc(2026, 11, 23, 3, 0))); // Monday
}

#[test]
fn next_reset_is_strictly_after() {
    let at_reset = utc(2026, 11, 17, 3, 0);
    let zzz = game("zzz");
    assert_eq!(next_reset(&zzz, at_reset), at_reset + DAY);
    assert_eq!(next_reset(&zzz, at_reset - 1), at_reset);
    assert_eq!(last_reset(&zzz, at_reset), at_reset);
    assert_eq!(last_reset(&zzz, at_reset - 1), at_reset - DAY);
}

#[test]
fn next_weekly_matches_the_simple_algorithm_everywhere() {
    let start = utc(2026, 1, 1, 0, 0);
    for g in [game("ba"), game("zzz"), custom(23, 30, Some(3)), custom(0, 0, Some(0))] {
        let mut samples: Vec<Ts> = (0..4000).map(|i| start + 17 * MINUTE * i).collect();
        for d in 0..15 {
            let r = next_reset(&g, start + d * DAY);
            samples.extend([r - 1, r, r + 1]);
        }
        for t in samples {
            assert_eq!(next_weekly(&g, t), Some(simple_next_weekly(&g, t)), "{} at {t}", g.key);
        }
    }
}

#[test]
fn weekly_is_none_without_a_weekly_day() {
    let g = custom(4, 0, None);
    assert_eq!(next_weekly(&g, utc(2026, 11, 18, 0, 0)), None);
    assert_eq!(last_weekly(&g, utc(2026, 11, 18, 0, 0)), None);
}

#[test]
fn is_weekly_moment_uses_the_utc_day() {
    let monday_reset = utc(2026, 11, 16, 3, 0);
    assert!(is_weekly_moment(&game("zzz"), monday_reset));
    assert!(!is_weekly_moment(&game("zzz"), monday_reset + DAY));
    assert!(is_weekly_moment(&game("ba"), utc(2026, 11, 15, 19, 0))); // Sunday
}

#[test]
fn ordered_soonest_first() {
    let games = built_in();
    let keys = |at| ordered(&games, at).iter().map(|(g, _)| g.key.clone()).collect::<Vec<_>>();
    assert_eq!(keys(utc(2026, 11, 17, 8, 0)), ["ba", "zzz"]); // BA at 19:00, ZZZ at 03:00 tomorrow
    assert_eq!(keys(utc(2026, 11, 17, 20, 0)), ["zzz", "ba"]);
}

#[test]
fn old_games_saved_with_a_server_offset_move_to_utc() {
    let old = |hour, minute, offset_minutes, weekly_day| CustomGame {
        id: "custom-1".into(), name: "Old".into(), icon: None, hour, minute, weekly_day, zone: None, offset_minutes,
    }.in_utc();
    // 04:00 UTC-7 is 11:00 UTC, same day
    let g = old(4, 0, -420, Some(0));
    assert_eq!((g.hour, g.minute, g.weekly_day, g.offset_minutes), (11, 0, Some(0), 0));
    // 04:00 UTC+9 is 19:00 UTC the day before: Monday becomes Sunday
    let g = old(4, 0, 540, Some(0));
    assert_eq!((g.hour, g.minute, g.weekly_day), (19, 0, Some(6)));
    // 22:30 UTC-5:30 is 04:00 UTC the next day: Sunday becomes Monday
    let g = old(22, 30, -330, Some(6));
    assert_eq!((g.hour, g.minute, g.weekly_day), (4, 0, Some(0)));
    // already UTC: unchanged
    assert_eq!(old(4, 0, 0, None), CustomGame { id: "custom-1".into(), name: "Old".into(), icon: None, hour: 4,
                                                minute: 0, weekly_day: None, zone: None, offset_minutes: 0 });
}

#[test]
fn games_json_round_trip_and_old_format() {
    let games = vec![CustomGame { id: "custom-1".into(), name: "Arknights".into(), icon: Some("custom-1-5.png".into()),
                                  hour: 20, minute: 0, weekly_day: Some(0), zone: None, offset_minutes: 0 }];
    let json = serde_json::to_string(&games).unwrap();
    assert!(!json.contains("offsetMinutes"));
    assert_eq!(serde_json::from_str::<Vec<CustomGame>>(&json).unwrap(), games);
    let old: Vec<CustomGame> = serde_json::from_str(
        r#"[{"id":"custom-2","name":"HSR","icon":null,"hour":4,"minute":0,"offsetMinutes":60,"weeklyDay":0}]"#).unwrap();
    assert_eq!(old[0].clone().in_utc().hour, 3);
}

#[test]
fn input_validation() {
    let ok = GameInput { name: "  Arknights ".into(), hour: 20, minute: 0, weekly_day: Some(0), zone: None };
    assert_eq!(ok.validated().unwrap().name, "Arknights");
    assert!(GameInput { name: " ".into(), ..ok.clone() }.validated().is_err());
    assert!(GameInput { name: "x".repeat(41), ..ok.clone() }.validated().is_err());
    assert!(GameInput { hour: 24, ..ok.clone() }.validated().is_err());
    assert!(GameInput { minute: 60, ..ok.clone() }.validated().is_err());
    assert!(GameInput { weekly_day: Some(7), ..ok }.validated().is_err());
}
