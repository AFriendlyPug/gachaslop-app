//! What's saved in the app data folder: settings.json (the user's choices), state.json (which toasts were
//! shown, the notifications list, request counts), games.json (custom games) and banners.json (stored banners).
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::banners::{write_atomic, Feeds};
use crate::config::{MAX_HEADSUP_MINUTES, MAX_NOTICES, NOTICE_KEEP};
use crate::games::CustomGame;
use crate::logs;
use crate::time::{month, Ts};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub disabled_games: Vec<String>, // games switched off (all on by default)
    pub reset_toasts: bool,
    pub headsup_minutes: u32,        // 0 = no heads-up
    pub banner_toasts: bool,
    pub catchup: bool,               // show what was missed while the app was closed
    pub offline: bool,               // make no network requests at all
    pub autostart: bool,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings { disabled_games: vec![], reset_toasts: true, headsup_minutes: 0, banner_toasts: true,
                   catchup: true, offline: false, autostart: false }
    }
}

impl Settings {
    pub fn enabled(&self, key: &str) -> bool {
        !self.disabled_games.iter().any(|k| k == key)
    }

    pub fn validated(mut self) -> Settings {
        self.headsup_minutes = self.headsup_minutes.min(MAX_HEADSUP_MINUTES);
        self.disabled_games.sort();
        self.disabled_games.dedup();
        self
    }
}

/// A notification that came in, for the popup's Notifications list.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Notice {
    pub at: Ts,        // when it happened
    pub added: Ts,     // when the app noticed it: later than `at` for something missed
    pub game: String,
    pub title: String,
    pub body: String,
    pub missed: bool,  // happened while the app was closed or the PC slept
}

/// The "While you were away" items saved by earlier versions: turned into notices on load.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Missed {
    pub at: Ts,
    pub game: String,
    pub text: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct State {
    pub initialised: bool,               // false until the first run has set its baseline
    pub notified: BTreeMap<String, Ts>,  // event key -> when it may be forgotten
    pub last_seen: Option<Ts>,           // last time the app was known to be running
    pub notices: Vec<Notice>,            // notifications that came in, oldest first (see keep_notices)
    pub read_at: Option<Ts>,             // notices added after this are new to the user
    #[serde(skip_serializing)]
    pub missed: Vec<Missed>,             // from earlier versions only, see State::upgrade
    pub unseen: bool,                    // missed items the popup hasn't shown yet (tray dot)
    pub request_month: String,           // "2026-11"
    pub requests: u32,                   // network requests made in request_month
}

impl State {
    pub fn count_request(&mut self, now: Ts) {
        let month = month(now);
        if self.request_month != month {
            (self.request_month, self.requests) = (month, 0);
        }
        self.requests += 1;
    }

    pub fn requests_this_month(&self, now: Ts) -> u32 {
        if self.request_month == month(now) { self.requests } else { 0 }
    }

    /// Notices added since the user last looked at the list.
    pub fn is_new(&self, notice: &Notice) -> bool {
        self.read_at.is_none_or(|read| notice.added > read)
    }

    /// Earlier versions kept missed items as one-line texts: keep them as missed notices.
    pub fn upgrade(mut self) -> State {
        for m in std::mem::take(&mut self.missed) {
            self.notices.push(Notice { at: m.at, added: m.at, game: m.game, title: m.text, body: String::new(),
                                       missed: true });
        }
        self.notices.sort_by_key(|n| n.added);
        self
    }
}

/// Drop notices older than NOTICE_KEEP, and the oldest beyond MAX_NOTICES.
pub fn keep_notices(notices: &mut Vec<Notice>, now: Ts) {
    notices.retain(|n| n.added > now - NOTICE_KEEP);
    let extra = notices.len().saturating_sub(MAX_NOTICES);
    notices.drain(..extra);
}

/// Where everything is kept.
#[derive(Clone, Debug)]
pub struct Paths {
    pub root: PathBuf,
}

impl Paths {
    pub fn settings(&self) -> PathBuf { self.root.join("settings.json") }
    pub fn state(&self) -> PathBuf { self.root.join("state.json") }
    pub fn games(&self) -> PathBuf { self.root.join("games.json") }
    pub fn banners(&self) -> PathBuf { self.root.join("banners.json") }
    pub fn icons(&self) -> PathBuf { self.root.join("icons") }
    pub fn game_icons(&self) -> PathBuf { self.root.join("game-icons") }
}

/// Read a JSON file, or the default if it's missing or unreadable (logged, and the bad file kept aside).
pub fn load<T: DeserializeOwned + Default>(path: &Path) -> T {
    let Ok(text) = std::fs::read_to_string(path) else { return T::default() };
    serde_json::from_str(&text).unwrap_or_else(|err| {
        logs::warn(format!("Could not read {} ({err}), starting it afresh", path.display()));
        let _ = std::fs::rename(path, path.with_extension("json.bad"));
        T::default()
    })
}

pub fn save<T: Serialize>(path: &Path, value: &T) {
    let result = serde_json::to_vec_pretty(value).map_err(std::io::Error::other)
        .and_then(|data| write_atomic(path, &data));
    if let Err(err) = result {
        logs::warn(format!("Could not save {} ({err})", path.display()));
    }
}

/// Everything loaded from disk at startup.
pub struct Saved {
    pub settings: Settings,
    pub state: State,
    pub custom: Vec<CustomGame>,
    pub feeds: Feeds,
}

pub fn load_all(paths: &Paths) -> Saved {
    Saved {
        settings: load::<Settings>(&paths.settings()).validated(),
        state: load::<State>(&paths.state()).upgrade(),
        custom: load::<Vec<CustomGame>>(&paths.games()).into_iter().map(CustomGame::in_utc).collect(),
        feeds: load(&paths.banners()),
    }
}
