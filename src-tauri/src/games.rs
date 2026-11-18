//! The games (two built in, plus any the user adds) and the date maths for their resets. A reset time is
//! in UTC, or for a custom game optionally in a server's time zone (following its daylight saving, see
//! zones.rs); the popup shows it in the PC's own time.
use serde::{Deserialize, Serialize};

use crate::config::MAX_GAME_NAME;
use crate::time::{weekday, Ts, DAY, HOUR, MINUTE};
use crate::zones;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    pub key: String,
    pub name: String,
    pub short: String,               // tray tooltip and summaries, e.g. "BA"
    pub color: String,               // badge colour when there is no icon
    pub icon: Option<String>,        // file name in the game-icons folder (custom games only)
    pub hour: u32,                   // daily reset time, UTC (or in `zone`)
    pub minute: u32,
    pub weekly_day: Option<u32>,     // day of the weekly reset (0 = Monday .. 6 = Sunday), at the daily time
    pub zone: Option<String>,        // a Windows time zone id the times are in; None for UTC
    pub feed: Option<String>,        // banner feed key, built-ins only
    pub builtin: bool,
}

/// The built-in games. Custom games are added from Settings and saved in games.json.
pub fn built_in() -> Vec<Game> {
    vec![
        Game {
            key: "ba".into(), name: "Blue Archive (Global)".into(), short: "BA".into(), color: "#3498db".into(),
            icon: None, hour: 19, minute: 0, weekly_day: Some(6), zone: None, feed: Some("ba".into()), builtin: true,
        },
        Game {
            // 04:00 on the EU server (UTC+1, no daylight saving)
            key: "zzz".into(), name: "Zenless Zone Zero (EU)".into(), short: "ZZZ".into(), color: "#f1c40f".into(),
            icon: None, hour: 3, minute: 0, weekly_day: Some(0), zone: None, feed: Some("zzz".into()), builtin: true,
        },
    ]
}

/// A game the user added, as saved in games.json.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomGame {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub hour: u32,
    pub minute: u32,
    pub weekly_day: Option<u32>,
    /// The server's time zone (a Windows zone id) the times are in, following its daylight saving; None for UTC.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zone: Option<String>,
    /// Only in files from before times were UTC: the server's offset the time was entered in.
    #[serde(default, skip_serializing)]
    pub offset_minutes: i32,
}

impl CustomGame {
    /// The same game with its reset moved to UTC, if it was saved in a server's local time.
    pub fn in_utc(mut self) -> CustomGame {
        if self.offset_minutes != 0 {
            let local = (self.hour * 60 + self.minute) as i64 - self.offset_minutes as i64;
            let days = local.div_euclid(1440); // the UTC reset can fall on the day before or after
            (self.hour, self.minute) = ((local.rem_euclid(1440) / 60) as u32, (local.rem_euclid(60)) as u32);
            self.weekly_day = self.weekly_day.map(|d| (d as i64 + days).rem_euclid(7) as u32);
            self.offset_minutes = 0;
        }
        self
    }

    pub fn to_game(&self) -> Game {
        Game {
            key: self.id.clone(), name: self.name.clone(), short: self.name.clone(), color: "#8e8e93".into(),
            icon: self.icon.clone(), hour: self.hour, minute: self.minute, weekly_day: self.weekly_day,
            zone: self.zone.clone(), feed: None, builtin: false,
        }
    }
}

/// What the add/edit form sends (times in UTC, or in `zone`). The icon is handled separately.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameInput {
    pub name: String,
    pub hour: u32,
    pub minute: u32,
    pub weekly_day: Option<u32>,
    #[serde(default)]
    pub zone: Option<String>,
}

impl GameInput {
    /// The input with its name trimmed, or why it can't be saved.
    pub fn validated(&self) -> Result<GameInput, String> {
        let name = self.name.trim().to_string();
        if name.is_empty() || name.chars().count() > MAX_GAME_NAME {
            return Err(format!("The name must be 1 to {MAX_GAME_NAME} characters."));
        }
        if self.hour > 23 || self.minute > 59 {
            return Err("The reset time is not a valid time.".into());
        }
        if self.weekly_day.is_some_and(|d| d > 6) {
            return Err("The weekly reset day is not valid.".into());
        }
        let zone = self.zone.as_deref().map(str::trim).filter(|z| !z.is_empty()).map(String::from);
        if zone.as_deref().is_some_and(|z| !zones::exists(z)) {
            return Err("That time zone isn't one Windows knows.".into());
        }
        Ok(GameInput { name, zone, ..self.clone() })
    }
}

/// The game's reset on a given day of its own calendar (days since 1970, in UTC or its zone).
pub fn reset_on(game: &Game, day: i64) -> Ts {
    match &game.zone {
        Some(zone) => zones::local_to_utc(zone, day, game.hour, game.minute),
        None => day * DAY + game.hour as i64 * HOUR + game.minute as i64 * MINUTE,
    }
}

/// The days to try around `t`: a zone's offset moves a day's reset by up to about a day either way.
fn days_around(t: Ts, before: i64, after: i64) -> std::ops::RangeInclusive<i64> {
    t.div_euclid(DAY) - before..=t.div_euclid(DAY) + after
}

fn is_weekly_day(game: &Game, day: i64) -> bool {
    game.weekly_day == Some(weekday(day * DAY))
}

/// Next reset strictly after `after`.
pub fn next_reset(game: &Game, after: Ts) -> Ts {
    days_around(after, 2, 2).map(|d| reset_on(game, d)).find(|t| *t > after).unwrap_or(after + DAY)
}

/// The most recent reset at or before `at`.
pub fn last_reset(game: &Game, at: Ts) -> Ts {
    days_around(at, 2, 2).rev().map(|d| reset_on(game, d)).find(|t| *t <= at).unwrap_or(at - DAY)
}

/// Next weekly reset strictly after `after`, or None if the game has none.
pub fn next_weekly(game: &Game, after: Ts) -> Option<Ts> {
    game.weekly_day?;
    days_around(after, 2, 9).filter(|d| is_weekly_day(game, *d)).map(|d| reset_on(game, d)).find(|t| *t > after)
}

/// The most recent weekly reset at or before `at`, or None if the game has none.
pub fn last_weekly(game: &Game, at: Ts) -> Option<Ts> {
    game.weekly_day?;
    days_around(at, 9, 2).rev().filter(|d| is_weekly_day(game, *d)).map(|d| reset_on(game, d)).find(|t| *t <= at)
}

/// True if this daily reset moment is also the game's weekly reset.
pub fn is_weekly_moment(game: &Game, at: Ts) -> bool {
    days_around(at, 2, 2).any(|d| is_weekly_day(game, d) && reset_on(game, d) == at)
}

/// Games sorted by next reset, soonest first (ties keep list order).
pub fn ordered(games: &[Game], after: Ts) -> Vec<(&Game, Ts)> {
    let mut list: Vec<_> = games.iter().map(|g| (g, next_reset(g, after))).collect();
    list.sort_by_key(|(_, at)| *at);
    list
}
