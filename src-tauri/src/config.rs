//! Constants the rest of the app shares: data source URLs, timings (in seconds unless a Duration) and limits.
//! No side effects.
use std::time::Duration;

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
/// The Notifications list keeps this long, and at most this many.
pub const NOTICE_KEEP: i64 = 30 * DAY;
pub const MAX_NOTICES: usize = 200;

/// The scheduler never sleeps longer than this, so PC sleep and clock changes are noticed quickly.
pub const TICK: i64 = 30;
/// A pass this much later than TICK means the PC slept (or the clock jumped).
pub const WAKE_GAP: i64 = TICK + 90;
/// How often `last_seen` is saved while running.
pub const HEARTBEAT: i64 = 5 * MINUTE;

/// Notification cards (toast.rs): on screen for TOAST_BASE plus TOAST_PER_LINE for each line of text, up
/// to TOAST_MAX; after the mouse leaves one, it stays TOAST_AFTER_HOVER longer.
pub const TOAST_BASE: Duration = Duration::from_secs(6);
pub const TOAST_PER_LINE: Duration = Duration::from_millis(1500);
pub const TOAST_MAX: Duration = Duration::from_secs(12);
pub const TOAST_AFTER_HOVER: Duration = Duration::from_secs(3);
pub const TOAST_FADE_IN: Duration = Duration::from_millis(180);
pub const TOAST_FADE_OUT: Duration = Duration::from_millis(260);
/// A card listing several things (a summary, or cards that waited behind a full-screen game) shows this
/// many lines at most.
pub const TOAST_MERGED_LINES: usize = 4;

/// Updates (update.rs): GitHub's latest release of this app, where its installers may come from, how long
/// a download may take, and how often to check when the user allows it (a failed check tries again sooner).
pub const RELEASES_LATEST_URL: &str = "https://api.github.com/repos/AFriendlyPug/gachaslop-app/releases/latest";
pub const RELEASE_DOWNLOAD_PREFIX: &str = "https://github.com/AFriendlyPug/gachaslop-app/releases/download/";
pub const UPDATE_DOWNLOAD_TIMEOUT_SECS: u32 = 300;
pub const UPDATE_CHECK_EVERY: i64 = DAY;
pub const UPDATE_RETRY: i64 = 2 * HOUR;
/// The update check never runs sooner than this after the app starts: startup makes no requests.
pub const UPDATE_START_DELAY: i64 = 10 * MINUTE;
pub const RELEASES_PAGE: &str = "https://github.com/AFriendlyPug/gachaslop-app/releases";

/// With "Free memory when closed", the popup's web view is destroyed this long after it was closed (it
/// takes most of the app's memory) and built again when next opened.
pub const FREE_POPUP_AFTER: u64 = 30;
/// The popup's height (logical pixels): it fits the home view's sections (dropdowns counted closed; other
/// views keep that height and scroll), never shorter
/// than this, nor taller than the screen less a margin.
pub const MIN_POPUP_HEIGHT: u32 = 220;

/// Custom game icons: largest file accepted (the popup turns every picked image into a 128px PNG first), and
/// the largest image downloaded from a link.
pub const MAX_ICON_BYTES: usize = 1_000_000;
pub const MAX_ICON_DOWNLOAD_BYTES: u32 = 10_000_000;
pub const MAX_GAME_NAME: usize = 40;
pub const MAX_HEADSUP_MINUTES: u32 = 720;

/// Local port the running copy listens on, so a second launch can ask it to show its popup instead.
pub const INSTANCE_PORT: u16 = 47_613;
/// The log is started afresh when it grows past this.
pub const MAX_LOG_BYTES: u64 = 1_000_000;
/// Name of the "Start with Windows" entry under HKCU\...\Run.
pub const AUTOSTART_NAME: &str = "GachaSlop";
