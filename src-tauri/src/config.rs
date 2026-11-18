//! Constants the rest of the app shares: data source URLs, timings (in seconds) and limits. No side effects.
use crate::time::{DAY, HOUR, MINUTE};

/// Unofficial Blue Archive Global banner list: current banners, plus future ones projected from JP.
pub const BA_BANNERS_URL: &str = "https://ba.joexyz.online/api/v1/banners";
/// SchaleDB student icons, by `schaleDbId`.
pub const BA_ICON_URL: &str = "https://schaledb.com/images/student/icon/{id}.webp";
/// Unofficial pass-through of HoYoLAB's ZZZ gacha calendar (current phase only).
pub const ZZZ_CALENDAR_URL: &str = "https://api.ennead.cc/mihoyo/zenless/calendar";
/// The calendar's times are Asia server (UTC+8). Banners change at the same local server time on every
/// server, so they are moved to the EU server's (UTC+1, no daylight saving).
pub const ZZZ_CALENDAR_UTC_OFFSET: i64 = 8 * HOUR;
pub const ZZZ_EU_UTC_OFFSET: i64 = HOUR;

pub const USER_AGENT: &str = concat!("gachaslop-app/", env!("CARGO_PKG_VERSION"));
pub const HTTP_TIMEOUT_SECS: u32 = 15;

/// Check a feed this long after the banner change that prompted it, so the source has caught up.
pub const CHECK_DELAY: i64 = 5 * MINUTE;
/// After a check that failed or found nothing new, wait this long before the next (the last repeats).
pub const BACKOFF: [i64; 4] = [30 * MINUTE, 2 * HOUR, 6 * HOUR, 12 * HOUR];
/// A manual refresh inside this gap of the last fetch makes no request.
pub const MANUAL_REFRESH_GAP: i64 = 5 * MINUTE;

/// "Banner ending soon" toast this long before a banner ends.
pub const ENDING_WARNING: i64 = DAY;
/// An event this much older than when the app started (or woke) counts as missed (catch-up).
pub const CATCHUP_AFTER: i64 = 2 * MINUTE;
/// Three or more toasts at once become one summary.
pub const SUMMARY_FROM: usize = 3;
/// How long reset keys stay in the notified log (banner keys go when their banner ends).
pub const RESET_KEY_KEEP: i64 = 8 * DAY;
pub const MAX_MISSED: usize = 30;

/// The scheduler never sleeps longer than this, so PC sleep and clock changes are noticed quickly.
pub const TICK: i64 = 30;
/// A pass this much later than TICK means the PC slept (or the clock jumped).
pub const WAKE_GAP: i64 = TICK + 90;
/// How often `last_seen` is saved while running.
pub const HEARTBEAT: i64 = 5 * MINUTE;

/// Custom game icons: largest file accepted.
pub const MAX_ICON_BYTES: usize = 1_000_000;
pub const MAX_GAME_NAME: usize = 40;
pub const MAX_HEADSUP_MINUTES: u32 = 720;

/// Local port the running copy listens on, so a second launch can ask it to show its popup instead.
pub const INSTANCE_PORT: u16 = 47_613;
/// The log is started afresh when it grows past this.
pub const MAX_LOG_BYTES: u64 = 1_000_000;
/// Name of the "Start with Windows" entry under HKCU\...\Run.
pub const AUTOSTART_NAME: &str = "GachaSlop";
