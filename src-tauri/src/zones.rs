//! Time zones for custom games whose reset follows a server's local time, daylight saving included.
//! Windows has the rules (the same ones its clock uses), so the app needs no time zone database: for each
//! zone and year it gives the standard offset and, if the zone has daylight saving, when that starts and
//! ends. The maths here is plain Rust; only looking the rules up and listing the zones use Windows.
use std::collections::HashMap;
use std::sync::Mutex;

use serde::Serialize;

use crate::time::{civil_from_days, days_from_civil, Ts, DAY, HOUR, MINUTE};

/// When daylight saving starts or ends: the `week`th `weekday` of `month` (week 5 is the last one), at a
/// local time. Weekdays count from Sunday = 0, as Windows does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Transition {
    pub month: u32,
    pub week: u32,
    pub weekday: u32,
    pub hour: u32,
    pub minute: u32,
}

/// A zone's rule for one year. Offsets are Windows' "bias": minutes to add to local time to get UTC.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rule {
    pub bias: i32,
    pub standard_bias: i32,
    pub daylight: Option<Daylight>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Daylight {
    pub starts: Transition, // in local standard time
    pub ends: Transition,   // in local daylight time
    pub bias: i32,          // usually -60
}

/// The day (days since 1970-01-01) a transition falls on in `year`.
pub fn transition_day(year: i64, t: Transition) -> i64 {
    let first = days_from_civil(year, t.month, 1);
    let first_weekday = (first + 4).rem_euclid(7) as u32; // 1970-01-01 was a Thursday (4, counting from Sunday)
    let mut day = first + ((t.weekday + 7 - first_weekday) % 7) as i64 + (t.week.clamp(1, 5) as i64 - 1) * 7;
    let next_month = if t.month == 12 { days_from_civil(year + 1, 1, 1) } else { days_from_civil(year, t.month + 1, 1) };
    while day >= next_month {
        day -= 7; // "week 5" means the last one
    }
    day
}

/// The UTC moment of a local wall-clock time (given as seconds as if it were UTC) under this year's rule.
pub fn to_utc(rule: &Rule, year: i64, local: Ts) -> Ts {
    let at = |t: Transition| transition_day(year, t) * DAY + t.hour as i64 * HOUR + t.minute as i64 * MINUTE;
    let daylight = rule.daylight.filter(|d| {
        let (starts, ends) = (at(d.starts), at(d.ends));
        if starts < ends { (starts..ends).contains(&local) } else { local >= starts || local < ends } // south: over new year
    });
    let bias = rule.bias + daylight.map_or(rule.standard_bias, |d| d.bias);
    local + bias as i64 * MINUTE
}

/// Rules looked up, by zone and year (tests add their own with `remember`).
type Rules = HashMap<(String, i64), Option<Rule>>; // by zone id and year; None if Windows has no rule
static RULES: Mutex<Option<Rules>> = Mutex::new(None);

/// Keep a rule for a zone and year, as if Windows had given it.
pub fn remember(zone: &str, year: i64, rule: Option<Rule>) {
    RULES.lock().unwrap().get_or_insert_with(HashMap::new).insert((zone.to_string(), year), rule);
}

fn rule(zone: &str, year: i64) -> Option<Rule> {
    if let Some(found) = RULES.lock().unwrap().as_ref().and_then(|m| m.get(&(zone.to_string(), year))) {
        return *found;
    }
    let found = win::rule(zone, year);
    remember(zone, year, found);
    found
}

/// A local wall-clock time on `day` (days since 1970) in `zone`, as a UTC moment. An unknown zone is
/// treated as UTC.
pub fn local_to_utc(zone: &str, day: i64, hour: u32, minute: u32) -> Ts {
    let local = day * DAY + hour as i64 * HOUR + minute as i64 * MINUTE;
    match rule(zone, civil_from_days(day).0) {
        Some(rule) => to_utc(&rule, civil_from_days(day).0, local),
        None => local,
    }
}

/// A zone the user can pick: Windows' id (saved in games.json) and its display name.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Zone {
    pub id: String,   // "GMT Standard Time"
    pub name: String, // "(UTC+00:00) Dublin, Edinburgh, Lisbon, London"
}

pub use win::{exists, list};

#[cfg(windows)]
mod win {
    use std::ffi::c_void;
    use std::ptr::null_mut;

    use windows_sys::Win32::Foundation::{ERROR_SUCCESS, SYSTEMTIME};
    use windows_sys::Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ};
    use windows_sys::Win32::System::Time::{
        EnumDynamicTimeZoneInformation, GetTimeZoneInformationForYear, DYNAMIC_TIME_ZONE_INFORMATION,
        TIME_ZONE_INFORMATION,
    };

    use super::{Daylight, Rule, Transition, Zone};

    fn text(chars: &[u16]) -> String {
        String::from_utf16_lossy(&chars[..chars.iter().position(|c| *c == 0).unwrap_or(chars.len())])
    }

    fn zones() -> Vec<DYNAMIC_TIME_ZONE_INFORMATION> {
        let mut out = Vec::new();
        for index in 0.. {
            let mut zone: DYNAMIC_TIME_ZONE_INFORMATION = unsafe { std::mem::zeroed() };
            if unsafe { EnumDynamicTimeZoneInformation(index, &mut zone) } != ERROR_SUCCESS {
                break;
            }
            out.push(zone);
        }
        out
    }

    /// The name Windows shows for a zone, e.g. "(UTC+00:00) Dublin, Edinburgh, Lisbon, London".
    fn display(id: &str) -> String {
        let key: Vec<u16> = format!(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Time Zones\{id}")
            .encode_utf16().chain(Some(0)).collect();
        let name: Vec<u16> = "Display".encode_utf16().chain(Some(0)).collect();
        let mut buffer = [0u16; 256];
        let mut size = (buffer.len() * 2) as u32;
        let found = unsafe {
            RegGetValueW(HKEY_LOCAL_MACHINE, key.as_ptr(), name.as_ptr(), RRF_RT_REG_SZ, null_mut(),
                         buffer.as_mut_ptr() as *mut c_void, &mut size)
        } == ERROR_SUCCESS;
        if found { text(&buffer) } else { id.to_string() }
    }

    /// Every zone Windows knows, sorted as Windows lists them (by offset).
    pub fn list() -> Vec<Zone> {
        let mut out: Vec<(i32, Zone)> = zones().iter().map(|z| {
            let id = text(&z.TimeZoneKeyName);
            (z.Bias, Zone { name: display(&id), id })
        }).collect();
        out.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.name.cmp(&b.1.name))); // west to east, like Windows
        out.into_iter().map(|(_, z)| z).collect()
    }

    pub fn exists(id: &str) -> bool {
        zones().iter().any(|z| text(&z.TimeZoneKeyName) == id)
    }

    fn transition(t: &SYSTEMTIME) -> Transition {
        Transition { month: t.wMonth as u32, week: t.wDay as u32, weekday: t.wDayOfWeek as u32,
                     hour: t.wHour as u32, minute: t.wMinute as u32 }
    }

    pub fn rule(id: &str, year: i64) -> Option<Rule> {
        let zone = zones().into_iter().find(|z| text(&z.TimeZoneKeyName) == id)?;
        let mut info: TIME_ZONE_INFORMATION = unsafe { std::mem::zeroed() };
        if unsafe { GetTimeZoneInformationForYear(year as u16, &zone, &mut info) } == 0 {
            return None;
        }
        // a month of 0 means no daylight saving; a year-specific date (wYear set) isn't used by any zone today
        let daylight = (info.DaylightDate.wMonth != 0 && info.StandardDate.wMonth != 0).then(|| Daylight {
            starts: transition(&info.DaylightDate), ends: transition(&info.StandardDate), bias: info.DaylightBias,
        });
        Some(Rule { bias: info.Bias, standard_bias: info.StandardBias, daylight })
    }
}

#[cfg(not(windows))]
mod win {
    use super::{Rule, Zone};

    pub fn list() -> Vec<Zone> {
        vec![]
    }

    pub fn exists(_id: &str) -> bool {
        false
    }

    pub fn rule(_id: &str, _year: i64) -> Option<Rule> {
        None
    }
}
