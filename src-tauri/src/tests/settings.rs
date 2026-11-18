//! Saved settings and state: defaults, old files, request counts.
use super::{utc, TempDir};
use crate::settings::*;

#[test]
fn old_or_partial_files_still_load() {
    let s: Settings = serde_json::from_str(r#"{"offline": true, "someOldField": 1}"#).unwrap();
    assert!(s.offline && s.reset_toasts && s.catchup && s.banner_toasts);
    assert_eq!(serde_json::from_str::<State>("{}").unwrap(), State::default());
}

#[test]
fn settings_are_tidied_on_save() {
    let s = Settings { headsup_minutes: 9999, disabled_games: vec!["zzz".into(), "ba".into(), "zzz".into()],
                       ..Settings::default() }.validated();
    assert_eq!(s.headsup_minutes, 720);
    assert_eq!(s.disabled_games, ["ba", "zzz"]);
    assert!(!s.enabled("ba") && s.enabled("custom-1"));
}

#[test]
fn request_count_resets_each_month() {
    let mut st = State::default();
    let nov = utc(2026, 11, 30, 23, 0);
    st.count_request(nov);
    st.count_request(nov);
    assert_eq!(st.requests_this_month(nov), 2);
    let dec = utc(2026, 12, 1, 0, 0);
    assert_eq!(st.requests_this_month(dec), 0);
    st.count_request(dec);
    assert_eq!(st.requests_this_month(dec), 1);
}

#[test]
fn unreadable_file_falls_back_to_defaults_and_is_kept_aside() {
    let dir = TempDir::new("settings");
    let path = dir.0.join("settings.json");
    std::fs::write(&path, "not json").unwrap();
    assert_eq!(load::<Settings>(&path), Settings::default());
    assert!(dir.0.join("settings.json.bad").exists());
    save(&path, &Settings { offline: true, ..Settings::default() });
    assert!(load::<Settings>(&path).offline);
}

#[test]
fn state_from_before_times_were_numbers_is_set_aside() {
    let dir = TempDir::new("old-state");
    let path = dir.0.join("state.json");
    std::fs::write(&path, r#"{"initialised": true, "lastSeen": "2026-11-18T01:50:10Z"}"#).unwrap();
    assert_eq!(load::<State>(&path), State::default());
    assert!(dir.0.join("state.json.bad").exists());
}

#[test]
fn load_all_moves_old_custom_games_to_utc() {
    let dir = TempDir::new("load-all");
    let paths = Paths { root: dir.0.clone() };
    std::fs::write(paths.games(),
        r#"[{"id":"custom-1","name":"HSR (EU)","icon":null,"hour":4,"minute":0,"offsetMinutes":60,"weeklyDay":0}]"#).unwrap();
    let saved = load_all(&paths);
    assert_eq!((saved.custom[0].hour, saved.custom[0].weekly_day, saved.custom[0].offset_minutes), (3, Some(0), 0));
    assert_eq!(saved.settings, Settings::default());
}
