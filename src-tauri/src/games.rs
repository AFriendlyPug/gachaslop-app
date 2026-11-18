//! The games (two built in, plus any the user adds) and the date maths for their resets. Every reset
//! time is in UTC; the popup shows it in the PC's own time.
use serde::{Deserialize, Serialize};

use crate::config::MAX_GAME_NAME;
use crate::time::{start_of_day, weekday, Ts, DAY, HOUR, MINUTE};

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    pub key: String,
    pub name: String,
    pub short: String,               // tray tooltip and summaries, e.g. "BA"
    pub color: String,               // badge colour when there is no icon
    pub icon: Option<String>,        // file name in the game-icons folder (custom games only)
    pub hour: u32,                   // daily reset time, UTC
    pub minute: u32,
    pub weekly_day: Option<u32>,     // UTC day of the weekly reset (0 = Monday .. 6 = Sunday), at the daily time
    pub feed: Option<String>,        // banner feed key, built-ins only
    pub builtin: bool,
}

/// The built-in games. Custom games are added from Settings and saved in games.json.
pub fn built_in() -> Vec<Game> {
    vec![
        Game {
            key: "ba".into(), name: "Blue Archive (Global)".into(), short: "BA".into(), color: "#3498db".into(),
            icon: None, hour: 19, minute: 0, weekly_day: Some(6), feed: Some("ba".into()), builtin: true,
        },
        Game {
            // 04:00 on the EU server (UTC+1, no daylight saving)
            key: "zzz".into(), name: "Zenless Zone Zero (EU)".into(), short: "ZZZ".into(), color: "#f1c40f".into(),
            icon: None, hour: 3, minute: 0, weekly_day: Some(0), feed: Some("zzz".into()), builtin: true,
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
            feed: None, builtin: false,
        }
    }
}

/// What the add/edit form sends (times in UTC). The icon is handled separately.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameInput {
    pub name: String,
    pub hour: u32,
    pub minute: u32,
    pub weekly_day: Option<u32>,
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
        Ok(GameInput { name, ..self.clone() })
    }
}

/// Next reset strictly after `after`.
pub fn next_reset(game: &Game, after: Ts) -> Ts {
    let candidate = start_of_day(after) + game.hour as i64 * HOUR + game.minute as i64 * MINUTE;
    if candidate <= after { candidate + DAY } else { candidate }
}

/// The most recent reset at or before `at`.
pub fn last_reset(game: &Game, at: Ts) -> Ts {
    next_reset(game, at) - DAY
}

/// Next weekly reset strictly after `after`, or None if the game has none.
pub fn next_weekly(game: &Game, after: Ts) -> Option<Ts> {
    let day = game.weekly_day?;
    let next = next_reset(game, after); // the weekly reset is the first daily reset on weekly_day
    Some(next + ((day + 7 - weekday(next)) % 7) as i64 * DAY)
}

/// The most recent weekly reset at or before `at`, or None if the game has none.
pub fn last_weekly(game: &Game, at: Ts) -> Option<Ts> {
    next_weekly(game, at).map(|w| w - 7 * DAY)
}

/// True if this daily reset moment is also the game's weekly reset.
pub fn is_weekly_moment(game: &Game, at: Ts) -> bool {
    game.weekly_day == Some(weekday(at))
}

/// Games sorted by next reset, soonest first (ties keep list order).
pub fn ordered(games: &[Game], after: Ts) -> Vec<(&Game, Ts)> {
    let mut list: Vec<_> = games.iter().map(|g| (g, next_reset(g, after))).collect();
    list.sort_by_key(|(_, at)| *at);
    list
}
