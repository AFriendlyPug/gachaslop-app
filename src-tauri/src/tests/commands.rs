//! Custom games: deleting and their icons (picked, staged, kept, removed).
use super::{utc, TempDir};
use crate::banners::Feeds;
use crate::commands::*;
use crate::games::CustomGame;
use crate::scheduler::due_events;
use crate::settings::{Notice, Paths, Settings, State};
use crate::store::Inner;

const PNG: &[u8] = b"\x89PNG\r\n\x1a\nrest-of-file";

fn game(id: &str, icon: Option<&str>) -> CustomGame {
    CustomGame { id: id.into(), name: id.into(), icon: icon.map(String::from), hour: 11, minute: 0,
                 weekly_day: Some(0), zone: None, offset_minutes: 0 }
}

fn files(dir: &std::path::Path) -> Vec<String> {
    let mut names: Vec<_> = std::fs::read_dir(dir).map(|e| e.flatten().map(|f| f.file_name().to_string_lossy().into_owned())
        .collect()).unwrap_or_default();
    names.sort();
    names
}

#[test]
fn deleting_a_game_removes_everything_recorded_about_it() {
    let now = utc(2026, 11, 18, 12, 0);
    let mut inner = Inner::new(Settings { disabled_games: vec!["custom-1".into(), "zzz".into()], ..Settings::default() },
                               State::default(), vec![game("custom-1", Some("custom-1-5.png")), game("custom-12", None)],
                               Feeds::default());
    seed_game(&mut inner, "custom-1", now);
    seed_game(&mut inner, "custom-12", now);
    let notice = |game: &str| Notice { at: now, added: now, game: game.into(), title: "Reset".into(), body: String::new(),
                                       missed: false };
    inner.state.notices = vec![notice("custom-1"), notice("custom-12")];
    let removed = remove_custom_game(&mut inner, "custom-1").unwrap();
    assert_eq!(removed.icon.as_deref(), Some("custom-1-5.png"));
    assert_eq!(inner.custom.iter().map(|g| g.id.as_str()).collect::<Vec<_>>(), ["custom-12"]);
    assert!(inner.state.notified.keys().all(|k| !k.contains(":custom-1:")), "{:?}", inner.state.notified);
    assert!(inner.state.notified.keys().any(|k| k.contains(":custom-12:"))); // a similar id is untouched
    assert_eq!(inner.state.notices.len(), 1); // its notifications go too
    assert_eq!(inner.settings.disabled_games, ["zzz"]);
    assert!(remove_custom_game(&mut inner, "custom-1").is_none());
}

#[test]
fn seeding_a_game_covers_all_its_current_events() {
    let now = utc(2026, 11, 18, 12, 0);
    let mut inner = Inner::new(Settings::default(), State::default(), vec![game("custom-1", None)], Feeds::default());
    seed_game(&mut inner, "custom-1", now);
    let games = inner.games();
    let custom: Vec<_> = games.iter().filter(|g| g.key == "custom-1").cloned().collect();
    for e in due_events(&custom, &inner.feeds, 0, now) {
        assert!(inner.state.notified.contains_key(&e.key), "{}", e.key);
    }
}

#[test]
fn picked_icons_are_checked_staged_then_kept_or_cleared() {
    let dir = TempDir::new("icons");
    let paths = Paths { root: dir.0.clone() };
    assert!(stage_icon(&paths, b"<html>not an image</html>").is_err());
    let mut big = PNG.to_vec();
    big.resize(1_000_001, 0);
    assert!(stage_icon(&paths, &big).is_err());

    let staged = stage_icon(&paths, PNG).unwrap();
    assert!(staged.file_name().unwrap().to_string_lossy().starts_with("staged-"));
    assert_eq!(std::fs::read(&staged).unwrap(), PNG);
    let kept = adopt_staged(&paths, &staged.to_string_lossy(), "custom-7").unwrap();
    assert!(kept.starts_with("custom-7-") && kept.ends_with(".png"), "{kept}");
    assert_eq!(files(&paths.game_icons()), [kept.as_str()]);
    // a staged name can't point outside the folder, and must still exist
    assert!(adopt_staged(&paths, "..\\..\\settings.json", "custom-7").is_err());
    assert!(adopt_staged(&paths, &staged.to_string_lossy(), "custom-7").is_err());

    // picked but never saved: cleared, while saved icons stay
    stage_icon(&paths, PNG).unwrap();
    stage_icon(&paths, b"\xff\xd8\xff\xe0jpeg").unwrap();
    clear_staged(&paths);
    assert_eq!(files(&paths.game_icons()), [kept.as_str()]);
    remove_icon(&paths, &Some(kept));
    assert!(files(&paths.game_icons()).is_empty());
}
