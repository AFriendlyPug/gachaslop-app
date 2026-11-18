//! Store: the in-memory copy of everything saved on disk, shared by the scheduler, tray and commands.
use std::sync::{Condvar, Mutex, MutexGuard};
use std::time::Duration;

use crate::banners::{FeedKind, Feeds};
use crate::games::{built_in, CustomGame, Game};
use crate::settings::{load_all, save, Paths, Settings, State};

pub struct Inner {
    pub settings: Settings,
    pub state: State,
    pub custom: Vec<CustomGame>,
    pub feeds: Feeds,
    pub refresh_requested: bool, // manual "Refresh banners", picked up by the scheduler
    pub refresh_feeds: Vec<FeedKind>, // feeds of games just switched back on: checked as if refreshed by hand
    pub replay_catchup: bool,    // Settings' "Replay catch-up": the next pass treats what's due as missed
}

impl Inner {
    pub fn new(settings: Settings, state: State, custom: Vec<CustomGame>, feeds: Feeds) -> Inner {
        Inner { settings, state, custom, feeds, refresh_requested: false, refresh_feeds: vec![], replay_catchup: false }
    }

    /// Every game, built-in first, then the user's in the order they were added.
    pub fn games(&self) -> Vec<Game> {
        built_in().into_iter().chain(self.custom.iter().map(CustomGame::to_game)).collect()
    }

    pub fn enabled_games(&self) -> Vec<Game> {
        self.games().into_iter().filter(|g| self.settings.enabled(&g.key)).collect()
    }

    /// Feeds whose game is switched on (a switched-off game's banners are never fetched).
    pub fn enabled_feeds(&self) -> Vec<FeedKind> {
        self.enabled_games().iter().filter_map(|g| g.feed.as_deref().and_then(FeedKind::from_key)).collect()
    }
}

fn unpoison<T>(result: std::sync::LockResult<T>) -> T {
    result.unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub struct Store {
    pub paths: Paths,
    inner: Mutex<Inner>,
    woken: Mutex<bool>, // set by wake(), so the scheduler runs a pass early
    signal: Condvar,
}

impl Store {
    pub fn open(paths: Paths) -> Store {
        let saved = load_all(&paths);
        let inner = Inner::new(saved.settings, saved.state, saved.custom, saved.feeds);
        Store { paths, inner: Mutex::new(inner), woken: Mutex::new(false), signal: Condvar::new() }
    }

    pub fn lock(&self) -> MutexGuard<'_, Inner> {
        unpoison(self.inner.lock())
    }

    /// Wake the scheduler now (settings changed, manual refresh).
    pub fn wake(&self) {
        *unpoison(self.woken.lock()) = true;
        self.signal.notify_one();
    }

    /// Sleep until `seconds` have passed or wake() is called.
    pub fn sleep(&self, seconds: i64) {
        let guard = unpoison(self.woken.lock());
        let wait = Duration::from_secs(seconds.max(0) as u64);
        let (mut woken, _) = unpoison(self.signal.wait_timeout_while(guard, wait, |woken| !*woken));
        *woken = false;
    }

    /// Ask the scheduler to check the banner feeds now (it skips any fetched in the last few minutes).
    pub fn request_refresh(&self) {
        self.lock().refresh_requested = true;
        self.wake();
    }

    /// Ask the scheduler to check these feeds now, as "Refresh banners" would (a game was switched back on).
    pub fn request_feeds(&self, kinds: Vec<FeedKind>) {
        if kinds.is_empty() {
            return;
        }
        self.lock().refresh_feeds.extend(kinds);
        self.wake();
    }

    pub fn save_settings(&self, inner: &Inner) {
        save(&self.paths.settings(), &inner.settings);
    }

    pub fn save_state(&self, inner: &Inner) {
        save(&self.paths.state(), &inner.state);
    }

    pub fn save_games(&self, inner: &Inner) {
        save(&self.paths.games(), &inner.custom);
    }

    pub fn save_feeds(&self, inner: &Inner) {
        save(&self.paths.banners(), &inner.feeds);
    }
}
