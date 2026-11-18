//! Banner feeds: parsing both APIs, when to fetch, and the icon cache.
use std::collections::HashSet;

use super::{fixture, rotation, utc, TempDir};
use crate::banners::*;
use crate::config::CHECK_DELAY;
use crate::time::{DAY, HOUR, MINUTE};

// ---- parsing (fixtures from the GachaSlop project's tests: ZZZ saved from the live API on 17 Nov 2026,
// BA a hand-made sample in the API's format) ----
#[test]
fn zzz_groups_a_phase_into_one_rotation() {
    let rots = parse_zzz(&fixture("zzz_calendar.json")).unwrap();
    assert_eq!(rots.len(), 1);
    let r = &rots[0];
    assert_eq!(r.label(), "Severian, Norma");
    assert_eq!(r.details, ["A-rank agents: Nicole, Pulchra"]);
    assert_eq!(r.extras, vec![]); // both W-Engines found their agent
    assert_eq!(r.icons().count(), 4);
    assert!(r.icons().all(|i| i.file.starts_with("zzz/")));
    assert_eq!(r.picks[0].icon.as_ref().unwrap().file, "zzz/agent-1631.png");
}

#[test]
fn zzz_agents_get_their_signature_w_engines() {
    let r = &parse_zzz(&fixture("zzz_calendar.json")).unwrap()[0];
    let pairs: Vec<_> = r.picks.iter().map(|p| (p.name.as_str(), p.engine.as_ref().map(|e| e.name.as_str()))).collect();
    assert_eq!(pairs, [("Severian", Some("Evil Caught Within")), ("Norma", Some("The Genius's Follower"))]);
    assert_eq!(r.picks[0].engine.as_ref().unwrap().icon.as_ref().unwrap().file, "zzz/wengine-14163.png");
}

fn named(name: &str, role: &str) -> (Pick, String) {
    (Pick { name: name.into(), marks: String::new(), icon: None, engine: None }, role.into())
}

#[test]
fn w_engines_pair_by_specialty_not_list_order() {
    let agents = vec![named("Stunner", "Stun"), named("Hitter", "Attack")];
    let engines = vec![named("Big Sword", "Attack"), named("Big Hammer", "Stun")];
    let (agents, left) = pair_engines(agents, engines);
    let engine = |i: usize| agents[i].engine.as_ref().map(|e| e.name.clone());
    assert_eq!((engine(0), engine(1)), (Some("Big Hammer".into()), Some("Big Sword".into())));
    assert_eq!(left, vec![]);
}

#[test]
fn w_engines_without_a_matching_agent_are_left_over() {
    let agents = vec![named("A", "Attack"), named("B", "Attack"), named("C", "")];
    let engines = vec![named("First", "Attack"), named("Odd", "Defense"), named("Second", "Attack")];
    let (agents, left) = pair_engines(agents, engines);
    let names: Vec<_> = agents.iter().map(|a| a.engine.as_ref().map(|e| e.name.as_str())).collect();
    assert_eq!(names, [Some("First"), Some("Second"), None]); // same specialty: list order; none: no guess
    assert_eq!(left.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["Odd"]);
}

#[test]
fn stored_banners_from_before_w_engine_pairing_still_load() {
    let old = r#"{"name":"Severian","marks":"","icon":null}"#;
    let pick: Pick = serde_json::from_str(old).unwrap();
    assert_eq!(pick.engine, None);
    assert!(!serde_json::to_string(&pick).unwrap().contains("engine")); // and nothing extra is saved
}

#[test]
fn zzz_times_are_moved_from_the_asia_server_to_eu_server_time() {
    let r = &parse_zzz(&fixture("zzz_calendar.json")).unwrap()[0];
    // the API gives 12:00 and 14:59:59 Asia server time (UTC+8): the same clock time on the EU server (UTC+1)
    assert_eq!(r.start, utc(2026, 11, 11, 11, 0));
    assert_eq!(r.end, utc(2026, 12, 1, 13, 59) + 59);
    assert!(r.key.starts_with("zzz|"));
}

#[test]
fn zzz_banner_without_dates_is_skipped() {
    let payload = serde_json::json!({"banners": [{"start_time": null, "end_time": 5, "agents": []}]});
    assert_eq!(parse_zzz(&payload).unwrap(), vec![]);
}

#[test]
fn ba_groups_pickups_by_start_and_end() {
    let rots = parse_ba(&fixture("ba_banners.json")).unwrap();
    assert_eq!(rots.len(), 2);
    let marked: Vec<_> = rots[0].picks.iter().map(|p| format!("{} {}", p.name, p.marks)).collect();
    assert_eq!(marked, ["Rio 🌟", "Iroha (Swimsuit) ⏳", "Neru (School) 🎁"]);
    let files: Vec<_> = rots[0].icons().map(|i| i.file.as_str()).collect();
    assert_eq!(files, ["ba/20041.webp", "ba/10148.webp", "ba/10111.webp"]);
    assert_eq!(rots[1].label(), "Nagusa, No Icon Student");
    assert_eq!(rots[1].icons().count(), 1);
    assert_eq!(rots[0].start, utc(2026, 11, 17, 2, 0));
    assert!(rots[0].start < rots[1].start);
    assert_eq!(rots[0].icons().next().unwrap().url, "https://schaledb.com/images/student/icon/20041.webp");
}

#[test]
fn ba_accepts_numeric_free_pulls_and_dates_without_offset() {
    let payload = serde_json::json!({"data": [{"startDate": "2026-11-17T02:00:00.000", "endDate": "2026-11-24T01:59:59.000Z",
        "freePulls": 0, "pickupStudents": [{"name": "Ibuki (Swimsuit)", "isFestGlobal": true, "schaleDbId": 20060}]}]});
    let rots = parse_ba(&payload).unwrap();
    assert_eq!(rots[0].picks[0].marks, "🌟");
    assert_eq!(rots[0].start, utc(2026, 11, 17, 2, 0));
}

#[test]
fn bad_payloads_are_errors_not_panics() {
    assert!(parse_ba(&serde_json::json!({"oops": 1})).is_err());
    assert!(parse_ba(&serde_json::json!({"data": [{"startDate": "soon", "endDate": "later"}]})).is_err());
    assert!(parse_zzz(&serde_json::json!([])).is_err());
}

// ---- when to fetch ----
#[test]
fn ba_is_checked_when_each_projected_banner_is_due_to_start() {
    let now = utc(2026, 11, 18, 12, 0);
    let rots = [rotation(FeedKind::Ba, utc(2026, 11, 17, 2, 0), utc(2026, 11, 24, 2, 0), "Live"),
                rotation(FeedKind::Ba, utc(2026, 11, 24, 2, 0), utc(2026, 12, 8, 2, 0), "Next"),
                rotation(FeedKind::Ba, utc(2026, 12, 8, 2, 0), utc(2026, 12, 22, 2, 0), "Later")];
    let mut feed = FeedState::default();
    feed.record(FeedKind::Ba, Ok(rots.to_vec()), now);
    assert_eq!(feed.next_check, Some(utc(2026, 11, 24, 2, 5)));
    assert!(!feed.is_due(now) && feed.is_due(utc(2026, 11, 24, 2, 5)));
    assert_eq!(feed.failures, 0);
}

#[test]
fn zzz_is_checked_when_the_current_phase_ends() {
    let now = utc(2026, 11, 18, 12, 0);
    let mut feed = FeedState::default();
    feed.record(FeedKind::Zzz, Ok(vec![rotation(FeedKind::Zzz, utc(2026, 11, 11, 11, 0), utc(2026, 12, 1, 14, 0), "S")]), now);
    assert_eq!(feed.next_check, Some(utc(2026, 12, 1, 14, 5)));
}

#[test]
fn nothing_new_backs_off_further_each_time_then_resets() {
    let ended = rotation(FeedKind::Zzz, utc(2026, 11, 11, 0, 0), utc(2026, 12, 1, 14, 0), "Old");
    let mut feed = FeedState::default();
    let mut now = utc(2026, 12, 1, 14, 5);
    let mut waits = vec![];
    for _ in 0..5 {
        feed.record(FeedKind::Zzz, Ok(vec![ended.clone()]), now); // the source hasn't caught up
        waits.push(feed.next_check.unwrap() - now);
        now = feed.next_check.unwrap();
    }
    assert_eq!(waits, [30 * MINUTE, 2 * HOUR, 6 * HOUR, 12 * HOUR, 12 * HOUR]);
    feed.record(FeedKind::Zzz, Ok(vec![rotation(FeedKind::Zzz, now, now + 20 * DAY, "New")]), now);
    assert_eq!((feed.failures, feed.next_check), (0, Some(now + 20 * DAY + CHECK_DELAY)));
}

#[test]
fn a_failed_check_keeps_the_old_banners() {
    let now = utc(2026, 11, 18, 12, 0);
    let live = rotation(FeedKind::Ba, now - DAY, now + DAY, "Live");
    let mut feed = FeedState::default();
    feed.record(FeedKind::Ba, Ok(vec![live.clone()]), now);
    feed.record(FeedKind::Ba, Err("down".into()), now + MINUTE);
    assert_eq!(feed.rotations, vec![live]);
    assert_eq!(feed.fetched_at, Some(now));
    assert_eq!(feed.next_check, Some(now + 31 * MINUTE));
    assert_eq!(feed.last_error.as_deref(), Some("down"));
}

#[test]
fn a_changed_projection_moves_the_check_and_is_not_confirmed_early() {
    let now = utc(2026, 11, 24, 2, 5);
    let moved = rotation(FeedKind::Ba, utc(2026, 11, 25, 2, 0), utc(2026, 12, 9, 2, 0), "Next");
    let mut feed = FeedState::default();
    feed.record(FeedKind::Ba, Ok(vec![moved.clone()]), now);
    assert_eq!(feed.next_check, Some(utc(2026, 11, 25, 2, 5)));
    assert!(!feed.confirmed(&moved));
    feed.record(FeedKind::Ba, Ok(vec![moved.clone()]), utc(2026, 11, 25, 2, 5));
    assert!(feed.confirmed(&moved));
}

#[test]
fn nothing_stored_is_due_at_once_and_fresh_blocks_manual_refresh() {
    let now = utc(2026, 11, 18, 12, 0);
    let mut feed = FeedState::default();
    assert!(feed.is_due(now) && !feed.fresh(now));
    feed.record(FeedKind::Ba, Ok(vec![]), now);
    assert!(feed.fresh(now + 4 * MINUTE) && !feed.fresh(now + 5 * MINUTE));
}

#[test]
fn ended_banners_are_dropped() {
    let now = utc(2026, 11, 18, 12, 0);
    let mut feed = FeedState { rotations: vec![rotation(FeedKind::Ba, now - 2 * DAY, now - DAY, "Old"),
                                                rotation(FeedKind::Ba, now - HOUR, now + DAY, "Live")], ..FeedState::default() };
    assert!(feed.drop_ended(now));
    assert_eq!(feed.rotations.iter().map(|r| r.label()).collect::<Vec<_>>(), ["Live"]);
    assert!(!feed.drop_ended(now));
}

#[test]
fn feeds_round_trip_through_json() {
    let mut feeds = Feeds::default();
    feeds.zzz.record(FeedKind::Zzz, parse_zzz(&fixture("zzz_calendar.json")), utc(2026, 11, 18, 12, 0));
    let back: Feeds = serde_json::from_str(&serde_json::to_string(&feeds).unwrap()).unwrap();
    assert_eq!(back, feeds);
    assert_eq!(serde_json::from_str::<Feeds>("{}").unwrap(), Feeds::default());
}

// ---- icons ----
#[test]
fn image_check_and_extension() {
    assert!(is_image(b"\x89PNG\r\n") && is_image(b"\xff\xd8\xff\xe0") && is_image(b"RIFF\0\0\0\0WEBPVP8 "));
    assert!(!is_image(b"<html>") && !is_image(b"RIFF") && !is_image(b""));
    assert_eq!([image_ext(b"\x89PNG"), image_ext(b"\xff\xd8\xff"), image_ext(b"RIFF\0\0\0\0WEBP")], ["png", "jpg", "webp"]);
}

#[test]
fn prune_keeps_wanted_icons_and_removes_the_rest() {
    let dir = TempDir::new("prune");
    for f in ["ba/1.png", "ba/2.png", "zzz/agent-1.png", "zzz/agent-1.png.tmp"] {
        write_atomic(&dir.0.join(f), b"x").unwrap();
    }
    let now = utc(2026, 11, 18, 12, 0);
    let mut feeds = Feeds::default();
    feeds.ba.rotations = vec![rotation(FeedKind::Ba, now, now + DAY, "1")];
    feeds.zzz.rotations = vec![rotation(FeedKind::Zzz, now + DAY, now + 2 * DAY, "agent-1")]; // upcoming
    let keep = feeds.wanted_icons();
    assert_eq!(keep, HashSet::from(["ba/1.png".to_string(), "zzz/agent-1.png".to_string()]));
    assert_eq!(prune_icons(&dir.0, &keep), 2);
    assert!(dir.0.join("ba/1.png").exists() && dir.0.join("zzz/agent-1.png").exists());
    assert!(!dir.0.join("ba/2.png").exists() && !dir.0.join("zzz/agent-1.png.tmp").exists());
}

#[test]
fn an_icon_shared_with_a_later_banner_is_kept() {
    let dir = TempDir::new("shared");
    write_atomic(&dir.0.join("ba/Hina.png"), b"x").unwrap();
    let now = utc(2026, 11, 18, 12, 0);
    let mut feed = FeedState { rotations: vec![rotation(FeedKind::Ba, now - 2 * DAY, now - DAY, "Hina"),
                                                rotation(FeedKind::Ba, now + DAY, now + 2 * DAY, "Hina")], ..FeedState::default() };
    feed.drop_ended(now);
    let feeds = Feeds { ba: feed, ..Feeds::default() };
    assert_eq!(prune_icons(&dir.0, &feeds.wanted_icons()), 0);
    assert!(dir.0.join("ba/Hina.png").exists());
}

#[test]
fn missing_icons_skips_files_already_present_and_duplicates() {
    let dir = TempDir::new("missing");
    write_atomic(&dir.0.join("ba/Have.png"), b"x").unwrap();
    let now = utc(2026, 11, 18, 12, 0);
    let mut feeds = Feeds::default();
    feeds.ba.rotations = vec![rotation(FeedKind::Ba, now, now + DAY, "Have"),
                              rotation(FeedKind::Ba, now, now + 2 * DAY, "Need"),
                              rotation(FeedKind::Ba, now + 2 * DAY, now + 3 * DAY, "Need")];
    let missing: Vec<_> = missing_icons(&feeds, &FeedKind::ALL, &dir.0).into_iter().map(|i| i.file).collect();
    assert_eq!(missing, ["ba/Need.png"]);
}

#[test]
fn icons_are_only_fetched_for_the_feeds_asked_for() {
    let dir = TempDir::new("icons-per-feed");
    let now = utc(2026, 11, 18, 12, 0);
    let mut feeds = Feeds::default();
    feeds.ba.rotations = vec![rotation(FeedKind::Ba, now, now + DAY, "ba-1")];
    feeds.zzz.rotations = vec![rotation(FeedKind::Zzz, now, now + DAY, "zzz-1")];
    let only_zzz: Vec<_> = missing_icons(&feeds, &[FeedKind::Zzz], &dir.0).into_iter().map(|i| i.file).collect();
    assert_eq!(only_zzz, ["zzz/zzz-1.png"]); // BA switched off: none of its icons
}
