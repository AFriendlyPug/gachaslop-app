//! Which toasts are due, catch-up after the app was closed, and which feeds to fetch.
use super::{rotation, utc};
use crate::banners::{FeedKind, FeedState, Feeds};
use crate::config::TICK;
use crate::games::{built_in, CustomGame, Game};
use crate::scheduler::*;
use crate::settings::{Settings, State};
use crate::store::Inner;
use crate::toast::Card;
use crate::time::{Ts, DAY, HOUR, MINUTE};

fn keys(events: &[Event]) -> Vec<String> {
    events.iter().map(|e| e.key.clone()).collect()
}

/// The events new at `now` after everything up to `before` was handled.
fn new_after(games: &[Game], feeds: &Feeds, before: Ts, now: Ts) -> Vec<Event> {
    let mut state = State::default();
    seed(&mut state, &due_events(games, feeds, 0, before));
    due_events(games, feeds, 0, now).into_iter().filter(|e| !state.notified.contains_key(&e.key)).collect()
}

fn inner(settings: Settings, feeds: Feeds) -> Inner {
    Inner::new(settings, State::default(), vec![], feeds)
}

#[test]
fn closed_for_three_days_gives_one_daily_toast_per_game() {
    let games = built_in();
    let new = new_after(&games, &Feeds::default(), utc(2026, 11, 18, 20, 0), utc(2026, 11, 21, 20, 0));
    let daily: Vec<_> = new.iter().filter(|e| e.kind == Kind::Reset).map(|e| e.game.as_str()).collect();
    assert_eq!(daily, ["ba", "zzz"]);
}

#[test]
fn a_weekly_reset_days_ago_is_still_reported() {
    let games = built_in();
    // closed Saturday, opened Thursday: BA's Sunday and ZZZ's Monday weeklies were both missed
    let new = new_after(&games, &Feeds::default(), utc(2026, 11, 14, 12, 0), utc(2026, 11, 19, 12, 0));
    let weekly: Vec<_> = new.iter().filter(|e| e.kind == Kind::Weekly).map(|e| (e.game.as_str(), e.at)).collect();
    assert_eq!(weekly, [("ba", utc(2026, 11, 15, 19, 0)), ("zzz", utc(2026, 11, 16, 3, 0))]);
    let plan = plan(&new, &Settings::default(), utc(2026, 11, 19, 12, 0));
    assert_eq!(plan.toasts.len(), 1);
    assert_eq!(plan.toasts[0].title, "While you were away");
    assert_eq!(plan.missed.len(), 4);
}

#[test]
fn toast_text_uses_relative_times() {
    let ba = vec![built_in().remove(0)];
    let events = due_events(&ba, &Feeds::default(), 0, utc(2026, 11, 18, 22, 0));
    let reset = events.iter().find(|e| e.kind == Kind::Reset).unwrap();
    assert_eq!(reset.title, "Blue Archive (Global) has reset");
    assert_eq!(reset.body, "Reset 3h 0m ago. Next reset in 21h 0m.");
    let just = due_events(&ba, &Feeds::default(), 0, utc(2026, 11, 18, 19, 0) + 20);
    assert!(just.iter().any(|e| e.body.starts_with("Reset just now.")));
}

#[test]
fn heads_up_only_for_a_reset_still_to_come() {
    let ba = vec![built_in().remove(0)];
    let feeds = Feeds::default();
    assert!(due_events(&ba, &feeds, 30, utc(2026, 11, 18, 18, 0)).iter().all(|e| e.kind != Kind::HeadsUp));
    let warn = due_events(&ba, &feeds, 30, utc(2026, 11, 18, 18, 40));
    let headsup: Vec<_> = warn.iter().filter(|e| e.kind == Kind::HeadsUp).collect();
    assert_eq!(headsup.len(), 1);
    assert_eq!(headsup[0].title, "Blue Archive (Global) reset in 20m");
    // after the reset, the heads-up for it is no longer due (the reset toast covers it)
    assert!(due_events(&ba, &feeds, 30, utc(2026, 11, 18, 19, 5)).iter().all(|e| e.kind != Kind::HeadsUp));
    // a heads-up before a weekly reset says so (22 Nov is a Sunday)
    let weekly = due_events(&ba, &feeds, 30, utc(2026, 11, 22, 18, 45));
    assert!(weekly.iter().any(|e| e.title == "Blue Archive (Global) daily and weekly reset in 15m"));
}

#[test]
fn banners_need_confirming_and_ones_that_ended_while_closed_are_skipped() {
    let now = utc(2026, 11, 18, 12, 0);
    let ba = vec![built_in().remove(0)];
    let mut feeds = Feeds {
        ba: FeedState {
            rotations: vec![rotation(FeedKind::Ba, now - 10 * DAY, now - 3 * DAY, "Gone"),
                            rotation(FeedKind::Ba, now - 2 * HOUR, now + 10 * HOUR, "Live")],
            fetched_at: Some(now - 3 * HOUR), // before "Live" started: still a projection
            ..FeedState::default()
        },
        ..Feeds::default()
    };
    let kinds = |f: &Feeds| due_events(&ba, f, 0, now).iter()
        .filter(|e| matches!(e.kind, Kind::BannerLive | Kind::BannerEnding)).map(|e| e.kind).collect::<Vec<_>>();
    assert_eq!(kinds(&feeds), [Kind::BannerEnding]);
    feeds.ba.fetched_at = Some(now - HOUR);
    assert_eq!(kinds(&feeds), [Kind::BannerLive, Kind::BannerEnding]);
}

#[test]
fn a_banner_found_by_the_startup_fetch_joins_the_catch_up_summary() {
    let games = built_in();
    let closed = utc(2026, 11, 18, 20, 0);
    let opened = utc(2026, 11, 24, 2, 5);
    let mut state = State::default();
    seed(&mut state, &due_events(&games, &Feeds::default(), 0, closed));
    // the startup fetch confirms BA's new banner (it started at 02:00 while the app was closed)
    let mut feeds = Feeds::default();
    feeds.ba.record(FeedKind::Ba, Ok(vec![rotation(FeedKind::Ba, utc(2026, 11, 24, 2, 0), utc(2026, 12, 8, 2, 0), "Nagusa")]), opened);
    let new: Vec<_> = due_events(&games, &feeds, 0, opened).into_iter()
        .filter(|e| !state.notified.contains_key(&e.key)).collect();
    let plan = plan(&new, &Settings::default(), opened);
    assert_eq!(plan.toasts.len(), 1);
    let Card { game, title, body } = &plan.toasts[0];
    assert_eq!(game, &None); // a summary shows the app's icon
    assert_eq!(title, "While you were away");
    assert!(body.contains("new BA banner (Nagusa)"), "{body}");
    // BA's weekly (Sunday) and daily (Monday) were at different times; ZZZ's Monday reset was both
    assert!(body.contains("BA daily reset") && body.contains("BA weekly reset"), "{body}");
    assert!(body.contains("ZZZ daily + weekly reset"), "{body}");
}

#[test]
fn first_run_and_a_new_game_produce_no_catch_up() {
    let mut games = built_in();
    let feeds = Feeds::default();
    let now = utc(2026, 11, 18, 12, 0);
    let mut state = State::default();
    seed(&mut state, &due_events(&games, &feeds, 0, now));
    let custom = CustomGame { id: "custom-1".into(), name: "Arknights".into(), icon: None, hour: 11, minute: 0,
                              weekly_day: Some(0), offset_minutes: 0 }.to_game();
    seed(&mut state, &due_events(std::slice::from_ref(&custom), &feeds, 0, now));
    games.push(custom);
    let new: Vec<_> = due_events(&games, &feeds, 0, now + MINUTE).into_iter()
        .filter(|e| !state.notified.contains_key(&e.key)).collect();
    assert_eq!(keys(&new), Vec::<String>::new());
}

#[test]
fn daily_and_weekly_at_the_same_moment_are_one_toast() {
    let zzz = vec![built_in().remove(1)];
    let monday = utc(2026, 11, 16, 3, 0);
    let new = new_after(&zzz, &Feeds::default(), monday - MINUTE, monday);
    assert_eq!(new.len(), 2);
    let plan = plan(&new, &Settings::default(), monday - HOUR); // running at the time: not missed
    assert_eq!(plan.toasts, [Card::new(Some("zzz"), "Zenless Zone Zero (EU) daily and weekly reset",
                                       "Daily and weekly reset just now. Next reset in 1d 0h.")]);
    assert!(plan.missed.is_empty());
}

#[test]
fn settings_decide_what_is_shown() {
    let games = built_in();
    let new = new_after(&games, &Feeds::default(), utc(2026, 11, 18, 20, 0), utc(2026, 11, 21, 20, 0));
    let running_since = utc(2026, 11, 21, 20, 0);
    let off = Settings { catchup: false, ..Settings::default() };
    assert_eq!(plan(&new, &off, running_since), Plan::default());
    let no_resets = Settings { reset_toasts: false, ..Settings::default() };
    assert_eq!(plan(&new, &no_resets, running_since), Plan::default());
    let shown = plan(&new, &Settings::default(), running_since);
    assert_eq!(shown.toasts.len(), 2); // two games' daily resets: under the summary threshold
    assert_eq!(shown.missed.len(), 2);
}

#[test]
fn notified_keys_expire() {
    let ba = vec![built_in().remove(0)];
    let now = utc(2026, 11, 18, 20, 0);
    let mut state = State::default();
    seed(&mut state, &due_events(&ba, &Feeds::default(), 0, now));
    let later = now + 9 * DAY;
    state.notified.retain(|_, forget_at| *forget_at > later);
    assert!(state.notified.is_empty(), "{:?}", state.notified);
}

#[test]
fn next_due_is_the_earliest_upcoming_moment() {
    let games = built_in();
    let now = utc(2026, 11, 18, 18, 59);
    assert_eq!(next_due(&games, &Feeds::default(), &Settings::default(), now), now + TICK);
    let near = now + 50;
    assert_eq!(next_due(&games, &Feeds::default(), &Settings::default(), near), utc(2026, 11, 18, 19, 0));
    let mut feeds = Feeds::default();
    feeds.ba.next_check = Some(now - HOUR); // overdue: straight away (the next second)
    assert_eq!(next_due(&games, &feeds, &Settings::default(), now), now + 1);
    assert_eq!(next_due(&games, &feeds, &Settings { offline: true, ..Settings::default() }, now), now + TICK);
}

#[test]
fn feeds_to_check_respects_offline_mode_switched_off_games_and_fresh_data() {
    let now = utc(2026, 11, 18, 12, 0);
    let mut feeds = Feeds::default();
    feeds.ba.next_check = Some(now - HOUR);           // overdue
    feeds.zzz.next_check = Some(now + DAY);           // not due
    feeds.zzz.fetched_at = Some(now - 2 * MINUTE);    // fetched moments ago
    let online = Settings::default();
    assert_eq!(feeds_to_check(&inner(online.clone(), feeds.clone()), now, false), [FeedKind::Ba]);
    // offline mode: nothing at all, even when asked
    let offline = Settings { offline: true, ..Settings::default() };
    assert_eq!(feeds_to_check(&inner(offline, feeds.clone()), now, true), []);
    // switching it off again: the overdue feed is checked once (record() then moves next_check on)
    let mut back = inner(online.clone(), feeds.clone());
    assert_eq!(feeds_to_check(&back, now, false), [FeedKind::Ba]);
    back.feeds.ba.record(FeedKind::Ba, Ok(vec![rotation(FeedKind::Ba, now - DAY, now + DAY, "Live")]), now);
    assert_eq!(feeds_to_check(&back, now + MINUTE, false), []);
    // a switched-off game's feed is never fetched
    let no_ba = Settings { disabled_games: vec!["ba".into()], ..Settings::default() };
    assert_eq!(feeds_to_check(&inner(no_ba, feeds.clone()), now, true), []);
    // manual refresh: ZZZ was fetched under 5 minutes ago, so only BA
    assert_eq!(feeds_to_check(&inner(online, feeds), now, true), [FeedKind::Ba]);
}

#[test]
fn a_long_gap_between_passes_means_the_pc_slept() {
    let now = utc(2026, 11, 18, 12, 0);
    assert!(!is_gap(now - TICK, now));
    assert!(!is_gap(now - 2 * MINUTE, now));
    assert!(is_gap(now - 3 * MINUTE, now));
    assert!(is_gap(now - 8 * HOUR, now));
}

// ---- Settings' "Testing" section ----
fn live_zzz(now: Ts) -> Feeds {
    let mut feeds = Feeds::default();
    feeds.zzz.record(FeedKind::Zzz, Ok(vec![rotation(FeedKind::Zzz, now - DAY, now + 10 * DAY, "Severian")]), now);
    feeds
}

#[test]
fn test_toasts_use_the_real_text_and_say_they_are_tests() {
    let now = utc(2026, 11, 18, 12, 0);
    let feeds = live_zzz(now);
    let toast = |s| sample_toast(s, &built_in(), &feeds, now).unwrap();
    assert_eq!(toast(Sample::Reset),
               Card::new(Some("ba"), "[Test] Blue Archive (Global) has reset", "Reset 17h 0m ago. Next reset in 7h 0m."));
    assert_eq!(toast(Sample::Weekly).title, "[Test] Blue Archive (Global) weekly reset");
    assert_eq!(toast(Sample::HeadsUp).body, "Time to finish your dailies.");
    assert_eq!(toast(Sample::BannerLive),
               Card::new(Some("zzz"), "[Test] New Zenless Zone Zero banner", "Severian\nEnds in 10d 0h."));
    // as it will read a day before the banner ends
    assert_eq!(toast(Sample::BannerEnding).body, "Severian\nEnds in 1d 0h.");
    assert_eq!(toast(Sample::Summary).title, "[Test] While you were away");
}

#[test]
fn test_banner_toasts_need_a_live_banner() {
    let now = utc(2026, 11, 18, 12, 0);
    assert_eq!(sample_toast(Sample::BannerLive, &built_in(), &Feeds::default(), now), None);
    assert_eq!(sample_toast(Sample::BannerEnding, &built_in(), &Feeds::default(), now), None);
    let ba_only: Vec<Game> = built_in().into_iter().filter(|g| g.key == "ba").collect();
    assert_eq!(sample_toast(Sample::BannerLive, &ba_only, &live_zzz(now), now), None); // ZZZ switched off
}

#[test]
fn replaying_catch_up_shows_what_is_due_again_as_missed() {
    let now = utc(2026, 11, 18, 12, 0);
    let (games, feeds) = (built_in(), live_zzz(now));
    let events = due_events(&games, &feeds, 0, now);
    let mut state = State::default();
    seed(&mut state, &events);
    forget(&mut state, &events);
    let new: Vec<Event> = events.into_iter().filter(|e| !state.notified.contains_key(&e.key)).collect();
    let plan = plan(&new, &Settings::default(), now); // the scheduler's pass after "Replay catch-up"
    assert_eq!(plan.toasts[0].title, "While you were away");
    assert_eq!(plan.missed.len(), new.len()); // two dailies, two weeklies and the banner, all missed
}

#[test]
fn banner_marks_are_written_out_in_notifications() {
    let now = utc(2026, 11, 18, 12, 0);
    let mut rot = rotation(FeedKind::Ba, now - DAY, now + 5 * DAY, "Rio");
    rot.picks[0].marks = "\u{1f31f} \u{1f381}".into();
    let mut feeds = Feeds::default();
    feeds.ba.record(FeedKind::Ba, Ok(vec![rot]), now);
    let live = due_events(&built_in(), &feeds, 0, now).into_iter().find(|e| e.kind == Kind::BannerLive).unwrap();
    assert_eq!(live.body, "Rio (Fest, Free pulls)\nEnds in 5d 0h.");

    let mut rot = rotation(FeedKind::Ba, now - DAY, now + 5 * DAY, "Ibuki (Swimsuit)");
    rot.picks[0].marks = "\u{1f31f}".into();
    feeds.ba.record(FeedKind::Ba, Ok(vec![rot]), now);
    let live = due_events(&built_in(), &feeds, 0, now).into_iter().find(|e| e.kind == Kind::BannerLive).unwrap();
    assert_eq!(live.body, "Ibuki (Swimsuit, Fest)\nEnds in 5d 0h."); // one set of brackets
}
