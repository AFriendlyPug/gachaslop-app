//! The commands the popup calls, and the snapshot of everything it shows. Times are Unix seconds (UTC).
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::Arc;

use serde::Serialize;
use tauri::ipc::{InvokeBody, Request};
use tauri::{AppHandle, Emitter, State};

use crate::autostart;
use crate::banners::{data_runs_out, image_ext, is_image, write_atomic, FeedKind, Pick};
use crate::config::MAX_ICON_BYTES;
use crate::games::{next_reset, next_weekly, CustomGame, Game, GameInput};
use crate::scheduler::{due_events, seed};
use crate::settings::{Missed, Paths, Settings};
use crate::store::{Inner, Store};
use crate::time::{now, now_millis, Ts};
use crate::tray::{self, DIALOG_OPEN};

type Store_<'a> = State<'a, Arc<Store>>;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameView {
    #[serde(flatten)]
    game: Game,
    icon_path: Option<String>,
    enabled: bool,
    next_reset: Ts,
    next_weekly: Option<Ts>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PickView {
    name: String,
    marks: String,
    icon_path: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RotationView {
    key: String,
    start: Ts,
    end: Ts,
    confirmed: bool,
    picks: Vec<PickView>,
    extras: Vec<PickView>,
    details: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedView {
    key: &'static str,
    name: &'static str,
    legend: &'static str,
    credit: &'static str,
    projected: bool,
    enabled: bool,
    fetched_at: Option<Ts>,
    checked_at: Option<Ts>,
    next_check: Option<Ts>,
    last_error: Option<String>,
    rotations: Vec<RotationView>,
    /// For a feed that never lists upcoming banners: when the next ones should start.
    next_unknown_from: Option<Ts>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    now: Ts,
    games: Vec<GameView>,
    feeds: Vec<FeedView>,
    settings: Settings,
    missed_since: Option<Ts>,
    missed: Vec<Missed>,
    requests_this_month: u32,
}

fn existing(path: &Path) -> Option<String> {
    path.exists().then(|| path.to_string_lossy().into_owned())
}

fn snapshot(paths: &Paths, inner: &Inner) -> Snapshot {
    let now = now();
    let icons = paths.icons();
    let pick = |p: &Pick| PickView {
        name: p.name.clone(), marks: p.marks.clone(),
        icon_path: p.icon.as_ref().and_then(|i| existing(&icons.join(&i.file))),
    };
    let games = inner.games().into_iter().map(|g| GameView {
        icon_path: g.icon.as_ref().and_then(|f| existing(&paths.game_icons().join(f))),
        enabled: inner.settings.enabled(&g.key),
        next_reset: next_reset(&g, now),
        next_weekly: next_weekly(&g, now),
        game: g,
    }).collect();
    let enabled = inner.enabled_feeds();
    let feeds = FeedKind::ALL.iter().map(|&kind| {
        let feed = inner.feeds.get(kind);
        let rotations: Vec<_> = feed.rotations.iter().filter(|r| r.end > now).collect();
        let next_unknown_from = (!kind.upcoming_projected() && !rotations.iter().any(|r| r.start > now))
            .then(|| data_runs_out(kind, &feed.rotations, now)).flatten();
        FeedView {
            key: kind.key(), name: kind.name(), legend: kind.legend(), credit: kind.credit(),
            projected: kind.upcoming_projected(), enabled: enabled.contains(&kind),
            fetched_at: feed.fetched_at, checked_at: feed.checked_at, next_check: feed.next_check,
            last_error: feed.last_error.clone(), next_unknown_from,
            rotations: rotations.into_iter().map(|r| RotationView {
                key: r.key.clone(), start: r.start, end: r.end, confirmed: feed.confirmed(r),
                picks: r.picks.iter().map(pick).collect(), extras: r.extras.iter().map(pick).collect(),
                details: r.details.clone(),
            }).collect(),
        }
    }).collect();
    Snapshot {
        now, games, feeds, settings: inner.settings.clone(), missed_since: inner.state.missed_since,
        missed: inner.state.missed.clone(), requests_this_month: inner.state.requests_this_month(now),
    }
}

/// Record a game's current resets as already shown, so adding or switching it on fires no stale toasts.
pub(crate) fn seed_game(inner: &mut Inner, key: &str, now: Ts) {
    let games: Vec<Game> = inner.games().into_iter().filter(|g| g.key == key).collect();
    let events = due_events(&games, &inner.feeds, inner.settings.headsup_minutes, now);
    seed(&mut inner.state, &events);
}

/// Take a custom game out, with everything recorded about it. Returns it so its icon can be deleted.
pub(crate) fn remove_custom_game(inner: &mut Inner, id: &str) -> Option<CustomGame> {
    let index = inner.custom.iter().position(|g| g.id == id)?;
    let game = inner.custom.remove(index);
    let tag = format!(":{id}:");
    inner.state.notified.retain(|key, _| !key.contains(&tag));
    inner.state.missed.retain(|m| m.game != id);
    inner.settings.disabled_games.retain(|k| k != id);
    Some(game)
}

// ---- custom game icons: picked in the popup, staged here, kept once the game is saved ----------------
const STAGED: &str = "staged-";

/// Check a picked image and keep it as a staged file until the game is saved. Returns its path.
pub(crate) fn stage_icon(paths: &Paths, data: &[u8]) -> Result<PathBuf, String> {
    if data.len() > MAX_ICON_BYTES {
        return Err("That image is over 1 MB. Please pick a smaller one.".into());
    }
    if !is_image(data) {
        return Err("That file isn't a PNG, JPG or WebP image.".into());
    }
    let path = paths.game_icons().join(format!("{STAGED}{}.{}", now_millis(), image_ext(data)));
    write_atomic(&path, data).map_err(|e| e.to_string())?;
    Ok(path)
}

/// Turn a staged icon into game `id`'s icon. Returns the new file name.
pub(crate) fn adopt_staged(paths: &Paths, staged: &str, id: &str) -> Result<String, String> {
    let name = Path::new(staged).file_name().and_then(|n| n.to_str()).unwrap_or_default();
    let ext = Path::new(name).extension().and_then(|e| e.to_str()).unwrap_or_default();
    let from = paths.game_icons().join(name);
    if !name.starts_with(STAGED) || !from.is_file() {
        return Err("The picked image has gone. Please choose it again.".into());
    }
    let file = format!("{id}-{}.{ext}", now_millis());
    std::fs::rename(&from, paths.game_icons().join(&file)).map_err(|e| e.to_string())?;
    Ok(file)
}

/// Delete any staged icons (picked but never saved).
pub(crate) fn clear_staged(paths: &Paths) {
    let Ok(entries) = std::fs::read_dir(paths.game_icons()) else { return };
    for entry in entries.flatten() {
        if entry.file_name().to_string_lossy().starts_with(STAGED) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

pub(crate) fn remove_icon(paths: &Paths, file: &Option<String>) {
    if let Some(file) = file {
        let _ = std::fs::remove_file(paths.game_icons().join(file));
    }
}

fn changed(app: &AppHandle, store: &Store) {
    store.wake();
    tray::sync(app, store);
    let _ = app.emit("state-changed", ());
}

// ---- commands ---------------------------------------------------------------------------------------
#[tauri::command]
pub fn get_snapshot(store: Store_<'_>) -> Snapshot {
    snapshot(&store.paths, &store.lock())
}

#[tauri::command]
pub fn save_settings(app: AppHandle, store: Store_<'_>, settings: Settings) -> Result<(), String> {
    let settings = settings.validated();
    let current = store.lock().settings.clone();
    if settings.autostart != current.autostart {
        autostart::set(settings.autostart)?;
    }
    let mut inner = store.lock();
    let switched_on: Vec<String> = inner.settings.disabled_games.iter().filter(|k| settings.enabled(k)).cloned().collect();
    inner.settings = settings;
    for key in switched_on {
        seed_game(&mut inner, &key, now());
    }
    store.save_settings(&inner);
    store.save_state(&inner);
    drop(inner);
    changed(&app, &store);
    Ok(())
}

#[tauri::command]
pub fn refresh_banners(store: Store_<'_>) -> String {
    let inner = store.lock();
    if inner.settings.offline {
        return "Offline mode is on.".into();
    }
    let feeds = inner.enabled_feeds();
    if feeds.is_empty() {
        return "No games with banners are switched on.".into();
    }
    if feeds.iter().all(|k| inner.feeds.get(*k).fresh(now())) {
        return "Already up to date.".into();
    }
    drop(inner);
    store.request_refresh();
    "Checking for new banners…".into()
}

#[tauri::command]
pub fn dismiss_missed(app: AppHandle, store: Store_<'_>) {
    let mut inner = store.lock();
    (inner.state.missed, inner.state.missed_since, inner.state.unseen) = (vec![], None, false);
    store.save_state(&inner);
    drop(inner);
    changed(&app, &store);
}

/// The file picker is opening (or has closed): don't hide the popup when it takes focus.
#[tauri::command]
pub fn set_dialog_open(open: bool) {
    DIALOG_OPEN.store(open, Ordering::SeqCst);
}

/// The picked image's bytes arrive as the raw request body.
#[tauri::command]
pub fn stage_game_icon(store: Store_<'_>, request: Request<'_>) -> Result<String, String> {
    let InvokeBody::Raw(data) = request.body() else { return Err("No image was sent.".into()) };
    stage_icon(&store.paths, data).map(|p| p.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn discard_staged(store: Store_<'_>) {
    clear_staged(&store.paths);
}

#[tauri::command]
pub fn add_game(app: AppHandle, store: Store_<'_>, game: GameInput, icon: Option<String>) -> Result<(), String> {
    let game = game.validated()?;
    let id = format!("custom-{}", now_millis());
    let icon = icon.map(|staged| adopt_staged(&store.paths, &staged, &id)).transpose()?;
    clear_staged(&store.paths);
    let mut inner = store.lock();
    inner.custom.push(CustomGame { id: id.clone(), name: game.name, icon, hour: game.hour, minute: game.minute,
                                   weekly_day: game.weekly_day, offset_minutes: 0 });
    seed_game(&mut inner, &id, now());
    store.save_games(&inner);
    store.save_state(&inner);
    drop(inner);
    changed(&app, &store);
    Ok(())
}

/// `icon`: a newly staged image to use; `clear_icon`: go back to the plain badge.
#[tauri::command]
pub fn update_game(app: AppHandle, store: Store_<'_>, id: String, game: GameInput, icon: Option<String>,
                   clear_icon: bool) -> Result<(), String> {
    let game = game.validated()?;
    let new_icon = icon.map(|staged| adopt_staged(&store.paths, &staged, &id)).transpose()?;
    clear_staged(&store.paths);
    let mut inner = store.lock();
    let Some(index) = inner.custom.iter().position(|g| g.id == id) else {
        remove_icon(&store.paths, &new_icon);
        return Err("That game no longer exists.".into());
    };
    let old = inner.custom[index].clone();
    let icon = if new_icon.is_some() || clear_icon { new_icon } else { old.icon.clone() };
    if icon != old.icon {
        remove_icon(&store.paths, &old.icon); // the replaced file is no longer needed
    }
    inner.custom[index] = CustomGame { id: id.clone(), name: game.name, icon, hour: game.hour, minute: game.minute,
                                       weekly_day: game.weekly_day, offset_minutes: 0 };
    if (old.hour, old.minute, old.weekly_day) != (game.hour, game.minute, game.weekly_day) {
        seed_game(&mut inner, &id, now()); // a new reset time: don't toast for a reset that only just "happened"
    }
    store.save_games(&inner);
    store.save_state(&inner);
    drop(inner);
    changed(&app, &store);
    Ok(())
}

#[tauri::command]
pub fn delete_game(app: AppHandle, store: Store_<'_>, id: String) -> Result<(), String> {
    let mut inner = store.lock();
    let Some(game) = remove_custom_game(&mut inner, &id) else { return Ok(()) };
    remove_icon(&store.paths, &game.icon);
    store.save_games(&inner);
    store.save_state(&inner);
    store.save_settings(&inner);
    drop(inner);
    changed(&app, &store);
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetPreview {
    next_reset: Ts,
    next_weekly: Option<Ts>,
}

/// When a game with these settings would next reset, for the form's preview.
#[tauri::command]
pub fn preview_reset(game: GameInput) -> Result<ResetPreview, String> {
    let g = GameInput { name: "Preview".into(), ..game }.validated()?; // the name may not be typed yet
    let game = CustomGame { id: "preview".into(), name: g.name, icon: None, hour: g.hour, minute: g.minute,
                            weekly_day: g.weekly_day, offset_minutes: 0 }.to_game();
    let now = now();
    Ok(ResetPreview { next_reset: next_reset(&game, now), next_weekly: next_weekly(&game, now) })
}

#[tauri::command]
pub fn hide_popup(app: AppHandle) {
    tray::hide_popup(&app);
}

#[tauri::command]
pub fn quit(app: AppHandle, store: Store_<'_>) {
    tray::quit(&app, &store);
}
