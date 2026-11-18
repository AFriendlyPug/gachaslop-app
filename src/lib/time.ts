// Times arrive from Rust as Unix seconds (UTC). Everything shown is in the PC's own time zone; reset times
// of games are also given in UTC alongside.

const MINUTE = 60_000;

/** Unix seconds to a Date. */
const date = (ts: number) => new Date(ts * 1000);

/** "45m", "2h 15m" or "3d 4h", rounded up to the minute (same as time::fmt_duration in Rust). */
export function fmtDuration(ms: number): string {
  const minutes = Math.max(1, Math.ceil(ms / MINUTE));
  const days = Math.floor(minutes / 1440);
  const hours = Math.floor(minutes / 60) % 24;
  if (days) return `${days}d ${hours}h`;
  return hours ? `${hours}h ${minutes % 60}m` : `${minutes}m`;
}

const pad = (n: number) => String(n).padStart(2, "0");

/** A ticking countdown: "02:15:07", or "5d 01:02:03" from a day up (same as time::fmt_clock in Rust). */
export function countdown(ms: number): string {
  const s = Math.max(0, Math.floor(ms / 1000));
  const days = Math.floor(s / 86_400);
  const clock = `${pad(Math.floor(s / 3600) % 24)}:${pad(Math.floor(s / 60) % 60)}:${pad(s % 60)}`;
  return days ? `${days}d ${clock}` : clock;
}

/** Time left until `ts` as a countdown, or "now" once it's here. `now` is in milliseconds (Date.now()). */
export function left(ts: number, now: number): string {
  const ms = ts * 1000 - now;
  return ms > 0 ? countdown(ms) : "now";
}

/** "in 02:15:07" until `ts`, or "now" once it's here. */
export function until(ts: number, now: number): string {
  const ms = ts * 1000 - now;
  return ms > 0 ? `in ${countdown(ms)}` : "now";
}

/** "just now", "5m ago", "3d 2h ago"; "never" for no time. */
export function ago(ts: number | null, now: number): string {
  if (ts === null) return "never";
  const ms = now - ts * 1000;
  return ms < MINUTE ? "just now" : `${fmtDuration(ms)} ago`;
}

/** Local clock time, with the day if it isn't today: "19:00" or "Sun 19:00". */
export function localTime(ts: number, now: number): string {
  const d = date(ts);
  const time = d.toLocaleTimeString("en-GB", { hour: "2-digit", minute: "2-digit" });
  const sameDay = new Date(now).toDateString() === d.toDateString();
  return sameDay ? time : `${d.toLocaleDateString("en-GB", { weekday: "short" })} ${time}`;
}

/** Local date and time: "Tue 24 Nov, 02:00". */
export function localDate(ts: number): string {
  return date(ts).toLocaleString("en-GB", {
    weekday: "short", day: "numeric", month: "short", hour: "2-digit", minute: "2-digit",
  });
}

/** "19:00 UTC". */
export const utcTime = (hour: number, minute: number) => `${pad(hour)}:${pad(minute)} UTC`;

/** A UTC clock time in local time today: utcToLocal(19, 0) is "20:00" in UTC+1. */
export function utcToLocal(hour: number, minute: number, now: number): string {
  const d = new Date(now);
  d.setUTCHours(hour, minute, 0, 0);
  return d.toLocaleTimeString("en-GB", { hour: "2-digit", minute: "2-digit" });
}

/** "Daily 19:00 UTC (20:00 your time)", plus the weekly day when there is one. */
export function resetText(hour: number, minute: number, weeklyDay: number | null, now: number): string {
  const daily = `Daily ${utcTime(hour, minute)} (${utcToLocal(hour, minute, now)} your time)`;
  return weeklyDay === null ? daily : `${daily}, weekly on ${WEEKDAYS[weeklyDay]}s (UTC)`;
}

export const WEEKDAYS = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];
