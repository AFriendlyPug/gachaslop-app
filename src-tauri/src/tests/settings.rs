//! Saved settings and state: defaults, old files, request counts.
use super::{utc, TempDir};
use crate::settings::*;
use crate::time::DAY;

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

#[test]
fn old_missed_items_become_missed_notices() {
    let old = r#"{"initialised": true, "missedSince": 5, "missed": [{"at": 100, "game": "ba", "text": "BA daily reset"}]}"#;
    let state = serde_json::from_str::<State>(old).unwrap().upgrade();
    assert_eq!(state.notices, [Notice { at: 100, added: 100, game: "ba".into(), title: "BA daily reset".into(),
                                        body: String::new(), missed: true }]);
    assert!(!serde_json::to_string(&state).unwrap().contains("\"missed\":[")); // not saved the old way again
}

#[test]
fn the_notifications_list_is_kept_for_a_month_and_capped() {
    let now = utc(2026, 11, 18, 12, 0);
    let notice = |added| Notice { at: added, added, game: "ba".into(), title: "Reset".into(), body: String::new(),
                                  missed: false };
    let mut notices = vec![notice(now - 31 * DAY), notice(now - 29 * DAY)];
    keep_notices(&mut notices, now);
    assert_eq!(notices, [notice(now - 29 * DAY)]);
    let mut many: Vec<Notice> = (0..250).map(|i| notice(now - 250 + i)).collect();
    keep_notices(&mut many, now);
    assert_eq!(many.len(), 200);
    assert_eq!(many[0].added, now - 200); // the oldest went first
}

#[test]
fn new_settings_have_sensible_defaults_and_old_files_get_them() {
    let s = Settings::default();
    assert!(!s.tray_icon && s.free_memory && s.update_checks);
    assert!(!s.hotkey_on); // the keyboard shortcut is off until turned on...
    assert_eq!(s.active_hotkey(), "");
    assert_eq!(s.hotkey, "Ctrl+Alt+G"); // ...and then this is the one used
    let on = Settings { hotkey_on: true, ..Settings::default() };
    assert_eq!(on.active_hotkey(), "Ctrl+Alt+G");
    assert_eq!(s.alerts("ba"), Alerts::default()); // a game not listed gets everything
    let old: Settings = serde_json::from_str(r#"{"offline": true}"#).unwrap();
    assert_eq!((old.hotkey_on, old.free_memory, old.tray_icon), (false, true, false));
}

#[test]
fn settings_are_tidied_before_saving() {
    let mut s = Settings { hotkey: "  Ctrl+Alt+K ".into(), ..Settings::default() };
    s.game_alerts.insert("ba".into(), Alerts::default()); // same as the default: not worth saving
    s.game_alerts.insert("zzz".into(), Alerts { banners: false, ..Alerts::default() });
    let s = s.validated();
    assert_eq!(s.hotkey, "Ctrl+Alt+K");
    assert_eq!(s.game_alerts.keys().collect::<Vec<_>>(), ["zzz"]);
}

#[test]
fn settings_from_the_short_lived_custom_height_still_load() {
    let old: Settings = serde_json::from_str(r#"{"popupHeight": 700, "offline": true}"#).unwrap();
    assert!(old.offline); // the old field is simply ignored
}
