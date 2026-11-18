//! Scheduler: one thread that checks the banner feeds when due, shows toasts (including anything missed
//! while the app was closed or the PC slept), and keeps the tray up to date.
use std::sync::Arc;

use tauri::{AppHandle, Emitter};
use tauri_plugin_notification::NotificationExt;

use crate::banners::{self, missing_icons, prune_icons, FeedKind, Feeds};
use crate::config::{CATCHUP_AFTER, ENDING_WARNING, HEARTBEAT, MAX_MISSED, RESET_KEY_KEEP, SUMMARY_FROM, TICK, WAKE_GAP};
use crate::games::{is_weekly_moment, last_reset, last_weekly, next_reset, Game};
use crate::logs;
use crate::settings::{Missed, Settings, State};
use crate::store::{Inner, Store};
use crate::time::{fmt_duration, now, Ts, MINUTE};
use crate::tray;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Reset,
    Weekly,
    HeadsUp,
    BannerLive,
    BannerEnding,
}

/// Something worth a toast. Its key goes in the notified log once handled, so it's never shown twice.
#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    pub key: String,
    pub kind: Kind,
    pub game: String,
    pub name: String,
    pub short: String,
    pub at: Ts,        // when it happened
    pub forget_at: Ts, // when its key can leave the notified log
    pub title: String,
    pub body: String,
    pub summary: String, // short form for summary toasts and the "While you were away" card
}

/// "just now" or "3h 5m ago". Toasts use relative times: the popup shows clock times in the PC's zone.
fn ago(at: Ts, now: Ts) -> String {
    if now - at < MINUTE { "just now".into() } else { format!("{} ago", fmt_duration(now - at)) }
}

/// Every event currently due for these games, whether or not it has been shown: the latest daily and
/// weekly reset, a heads-up if one is due, and each live banner's "new" and "ending soon".
pub fn due_events(games: &[Game], feeds: &Feeds, headsup_minutes: u32, now: Ts) -> Vec<Event> {
    let mut events = Vec::new();
    for g in games {
        let event = |kind, key: String, at, forget_at, title: String, body: String, summary: String| Event {
            key, kind, game: g.key.clone(), name: g.name.clone(), short: g.short.clone(), at, forget_at, title, body, summary,
        };
        let next = next_reset(g, now);
        let reset = last_reset(g, now);
        events.push(event(Kind::Reset, format!("reset:{}:{reset}", g.key), reset, reset + RESET_KEY_KEEP,
                          format!("{} has reset", g.name),
                          format!("Reset {}. Next reset in {}.", ago(reset, now), fmt_duration(next - now)),
                          format!("{} daily reset", g.short)));
        if let Some(weekly) = last_weekly(g, now) {
            events.push(event(Kind::Weekly, format!("weekly:{}:{weekly}", g.key), weekly, weekly + RESET_KEY_KEEP,
                              format!("{} weekly reset", g.name), format!("Weekly reset {}.", ago(weekly, now)),
                              format!("{} weekly reset", g.short)));
        }
        if headsup_minutes > 0 {
            let warn = next - headsup_minutes as i64 * MINUTE;
            if now >= warn {
                let what = if is_weekly_moment(g, next) { "daily and weekly reset" } else { "reset" };
                let left = fmt_duration(next - now);
                events.push(event(Kind::HeadsUp, format!("headsup:{}:{next}", g.key), warn, next + RESET_KEY_KEEP,
                                  format!("{} {what} in {left}", g.name), "Time to finish your dailies.".into(),
                                  format!("{} {what} in {left}", g.short)));
            }
        }
        let Some(kind) = g.feed.as_deref().and_then(FeedKind::from_key) else { continue };
        let feed = feeds.get(kind);
        for rot in feed.rotations.iter().filter(|r| r.is_live(now)) {
            let names = rot.picks.iter().map(|p| format!("{} {}", p.name, p.marks).trim().to_string())
                .collect::<Vec<_>>().join(", ");
            let left = fmt_duration(rot.end - now);
            if feed.confirmed(rot) {
                events.push(event(Kind::BannerLive, format!("banner-live:{}", rot.key), rot.start, rot.end,
                                  format!("New {} banner", kind.name()), format!("{names}\nEnds in {left}."),
                                  format!("new {} banner ({})", g.short, rot.label())));
            }
            let warn = rot.end - ENDING_WARNING;
            if now >= warn {
                events.push(event(Kind::BannerEnding, format!("banner-ending:{}", rot.key), warn, rot.end,
                                  format!("{} banner ending soon", kind.name()), format!("{names}\nEnds in {left}."),
                                  format!("{} banner ending ({})", g.short, rot.label())));
            }
        }
    }
    events
}

/// Mark these events as handled without showing them (first run, or a game just added or switched on).
pub fn seed(state: &mut State, events: &[Event]) {
    state.notified.extend(events.iter().map(|e| (e.key.clone(), e.forget_at)));
}

/// What to do with events that haven't been handled yet.
#[derive(Debug, Default, PartialEq)]
pub struct Plan {
    pub toasts: Vec<(String, String)>,
    pub missed: Vec<Missed>,
}

/// Turn new events into toasts. An event from before `running_since` (when the app last started or woke)
/// happened while it wasn't running: it is "missed", and also goes on the "While you were away" card.
/// A daily and weekly reset at the same moment become one toast; three or more toasts become one summary.
pub fn plan(new: &[Event], settings: &Settings, running_since: Ts) -> Plan {
    let missed = |e: &Event| e.kind != Kind::HeadsUp && e.at < running_since - CATCHUP_AFTER;
    let wanted = |e: &Event| match e.kind {
        Kind::Reset | Kind::Weekly => settings.reset_toasts,
        Kind::HeadsUp => true,
        Kind::BannerLive | Kind::BannerEnding => settings.banner_toasts,
    } && (settings.catchup || !missed(e));
    let shown: Vec<&Event> = new.iter().filter(|e| wanted(e)).collect();
    let pair = |e: &Event, kind| shown.iter().any(|o| o.kind == kind && o.game == e.game && o.at == e.at);

    let mut items: Vec<(String, String, String, &Event)> = Vec::new(); // title, body, summary, event
    for e in &shown {
        match e.kind {
            Kind::Weekly if pair(e, Kind::Reset) => continue, // shown with its daily reset
            Kind::Reset if pair(e, Kind::Weekly) => items.push((
                format!("{} daily and weekly reset", e.name), e.body.replacen("Reset", "Daily and weekly reset", 1),
                format!("{} daily + weekly reset", e.short), e)),
            _ => items.push((e.title.clone(), e.body.clone(), e.summary.clone(), e)),
        }
    }
    let toasts = if items.len() >= SUMMARY_FROM {
        let title = if items.iter().any(|(.., e)| missed(e)) { "While you were away" } else { "GachaSlop" };
        vec![(title.to_string(), items.iter().map(|(_, _, s, _)| s.as_str()).collect::<Vec<_>>().join(", "))]
    } else {
        items.iter().map(|(t, b, ..)| (t.clone(), b.clone())).collect()
    };
    let missed = items.iter().filter(|(.., e)| missed(e))
        .map(|(_, _, summary, e)| Missed { at: e.at, game: e.game.clone(), text: summary.clone() }).collect();
    Plan { toasts, missed }
}

/// The next moment something could become due: a reset, heads-up, banner change or feed check.
pub fn next_due(games: &[Game], feeds: &Feeds, settings: &Settings, now: Ts) -> Ts {
    let mut times = vec![now + TICK];
    for g in games {
        let next = next_reset(g, now);
        times.push(next);
        if settings.headsup_minutes > 0 {
            times.push(next - settings.headsup_minutes as i64 * MINUTE);
        }
        let Some(kind) = g.feed.as_deref().and_then(FeedKind::from_key) else { continue };
        let feed = feeds.get(kind);
        for r in &feed.rotations {
            times.extend([r.start, r.end, r.end - ENDING_WARNING]);
        }
        if !settings.offline {
            times.extend(feed.next_check.map(|t| t.max(now + 1)));
        }
    }
    times.into_iter().filter(|t| *t > now).min().unwrap_or(now + TICK) // this second's pass has just run
}

/// Feeds to fetch now: due ones (or all not fetched in the last few minutes, after "Refresh banners"),
/// only for games that are switched on, and none at all in offline mode.
pub fn feeds_to_check(inner: &Inner, now: Ts, manual: bool) -> Vec<FeedKind> {
    if inner.settings.offline {
        return vec![];
    }
    inner.enabled_feeds().into_iter()
        .filter(|k| inner.feeds.get(*k).is_due(now) || (manual && !inner.feeds.get(*k).fresh(now))).collect()
}

/// True if passes this far apart mean the PC slept (or the clock jumped) in between.
pub fn is_gap(previous: Ts, now: Ts) -> bool {
    now - previous > WAKE_GAP
}

/// Fetch any feed that is due (or was asked for by "Refresh banners"), then its missing icons.
fn check_feeds(app: &AppHandle, store: &Store) {
    let now = now();
    let (due, dropped) = {
        let mut inner = store.lock();
        let manual = std::mem::take(&mut inner.refresh_requested);
        let dropped = FeedKind::ALL.iter().fold(false, |d, k| inner.feeds.get_mut(*k).drop_ended(now) | d);
        (feeds_to_check(&inner, now, manual), dropped)
    };
    if due.is_empty() && !dropped {
        return;
    }
    for kind in &due {
        let result = banners::fetch(*kind);
        match &result {
            Ok(rots) => logs::info(format!("Fetched {} banners: {} stored", kind.key(), rots.len())),
            Err(err) => logs::warn(format!("Could not fetch {} banners ({err})", kind.key())),
        }
        let mut inner = store.lock();
        inner.state.count_request(crate::time::now());
        inner.feeds.get_mut(*kind).record(*kind, result, crate::time::now());
    }
    let dir = store.paths.icons();
    if !due.is_empty() {
        let missing = missing_icons(&store.lock().feeds, &dir); // banners just fetched, and any that failed before
        for icon in missing {
            let result = banners::download_icon(&dir, &icon);
            store.lock().state.count_request(crate::time::now());
            if let Err(err) = result {
                logs::warn(format!("Could not fetch icon {} ({err})", icon.file));
            }
        }
    }
    let inner = store.lock();
    store.save_feeds(&inner);
    store.save_state(&inner);
    let removed = prune_icons(&dir, &inner.feeds.wanted_icons());
    drop(inner);
    if removed > 0 {
        logs::info(format!("Deleted {removed} icons no stored banner uses"));
    }
    let _ = app.emit("state-changed", ());
}

/// Show toasts for anything new since the last pass, and record it all.
fn notify(app: &AppHandle, store: &Store, running_since: Ts, away_since: Option<Ts>) {
    let now = now();
    let mut inner = store.lock();
    let events = due_events(&inner.enabled_games(), &inner.feeds, inner.settings.headsup_minutes, now);
    let before = inner.state.notified.len();
    inner.state.notified.retain(|_, forget_at| *forget_at > now);
    let pruned = inner.state.notified.len() != before;
    if !inner.state.initialised {
        // first run: what's already happened is the baseline, not news
        seed(&mut inner.state, &events);
        inner.state.initialised = true;
        store.save_state(&inner);
        return;
    }
    let new: Vec<Event> = events.into_iter().filter(|e| !inner.state.notified.contains_key(&e.key)).collect();
    if new.is_empty() {
        if pruned {
            store.save_state(&inner);
        }
        return;
    }
    let plan = plan(&new, &inner.settings, running_since);
    seed(&mut inner.state, &new);
    if !plan.missed.is_empty() {
        if inner.state.missed.is_empty() {
            inner.state.missed_since = away_since;
        }
        inner.state.missed.extend(plan.missed);
        let extra = inner.state.missed.len().saturating_sub(MAX_MISSED);
        inner.state.missed.drain(..extra);
        inner.state.unseen = true;
    }
    store.save_state(&inner);
    drop(inner);
    for (title, body) in plan.toasts {
        if let Err(err) = app.notification().builder().title(&title).body(&body).show() {
            logs::warn(format!("Could not show a notification ({err})"));
        }
    }
    let _ = app.emit("state-changed", ());
}

/// The scheduler's loop, on its own thread.
pub fn run(app: AppHandle, store: Arc<Store>) {
    let mut running_since = now();
    let mut away_since = store.lock().state.last_seen; // when the app was last known to be running
    let mut last_pass = running_since;
    let mut last_heartbeat: Option<Ts> = None;
    loop {
        let start = now();
        if is_gap(last_pass, start) {
            // the PC slept (or the clock jumped): anything since the last pass was missed
            (running_since, away_since) = (start, Some(last_pass));
        }
        last_pass = start;
        check_feeds(&app, &store); // first, so a banner it finds joins the same catch-up
        notify(&app, &store, running_since, away_since);
        tray::sync(&app, &store);
        let now = now();
        if last_heartbeat.is_none_or(|t| now - t >= HEARTBEAT) {
            let mut inner = store.lock();
            inner.state.last_seen = Some(now);
            store.save_state(&inner);
            last_heartbeat = Some(now);
        }
        let wake_at = {
            let inner = store.lock();
            next_due(&inner.enabled_games(), &inner.feeds, &inner.settings, now)
        };
        store.sleep(wake_at - crate::time::now());
    }
}
