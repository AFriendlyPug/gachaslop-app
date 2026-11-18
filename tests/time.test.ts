// Tests for src/lib/time.ts, run with Node's built-in test runner: `npm test` (no test packages needed).
// Each test sets the time zone it needs, so results don't depend on the PC running them.
import { test } from "node:test";
import assert from "node:assert/strict";
import { ago, countdown, fmtDuration, lasts, left, localDate, roughlyUntil, localTime, resetText, until, utcTime, utcToLocal }
  from "../src/lib/time.ts";

const MIN = 60_000;
/** Unix seconds for a UTC date and time. */
const utc = (y: number, mo: number, d: number, h = 0, mi = 0) => Date.UTC(y, mo - 1, d, h, mi) / 1000;
/** Run with the PC's time zone set to `tz`. */
function inZone(tz: string, check: () => void) {
  const before = process.env.TZ;
  process.env.TZ = tz;
  try {
    check();
  } finally {
    if (before === undefined) delete process.env.TZ;
    else process.env.TZ = before;
  }
}

test("durations round up to the minute", () => {
  assert.equal(fmtDuration(1), "1m");
  assert.equal(fmtDuration(45 * MIN), "45m");
  assert.equal(fmtDuration((2 * 60 + 14) * MIN + 1000), "2h 15m");
  assert.equal(fmtDuration((3 * 24 + 4) * 60 * MIN), "3d 4h");
});

test("countdowns tick as HH:MM:SS, with days from a day up", () => {
  assert.equal(countdown(0), "00:00:00");
  assert.equal(countdown(-5000), "00:00:00");
  assert.equal(countdown(999), "00:00:00");
  assert.equal(countdown(7_000), "00:00:07");
  assert.equal(countdown(((2 * 60 + 15) * 60 + 7) * 1000), "02:15:07");
  assert.equal(countdown((24 * 3600 - 1) * 1000), "23:59:59");
  assert.equal(countdown(24 * 3600 * 1000), "1d 00:00:00");
  assert.equal(countdown(((5 * 24 + 1) * 3600 + 2 * 60 + 3) * 1000), "5d 01:02:03");
});

test("countdowns and time since", () => {
  const now = utc(2026, 11, 18, 17, 0) * 1000;
  assert.equal(until(utc(2026, 11, 18, 19, 0), now), "in 02:00:00");
  assert.equal(until(utc(2026, 11, 18, 16, 0), now), "now");
  assert.equal(left(utc(2026, 11, 18, 19, 0) + 7, now), "02:00:07");
  assert.equal(left(utc(2026, 11, 18, 17, 0), now), "now");
  assert.equal(ago(utc(2026, 11, 18, 16, 59) + 30, now), "just now");
  assert.equal(ago(utc(2026, 11, 15, 17, 0), now), "3d 0h ago");
  assert.equal(ago(null, now), "never");
});

test("UTC times are shown in the PC's own time zone", () => {
  const reset = utc(2026, 11, 18, 19, 0); // BA's daily reset
  inZone("Europe/London", () => {
    const now = utc(2026, 11, 18, 12, 0) * 1000;
    assert.equal(localTime(reset, now), "19:00");
    assert.equal(utcToLocal(19, 0, now), "19:00");
  });
  inZone("Asia/Tokyo", () => {
    const now = utc(2026, 11, 18, 12, 0) * 1000; // 21:00 on the 18th in Tokyo
    assert.equal(localTime(reset, now), "Thu 04:00"); // 04:00 the next morning
    assert.equal(utcToLocal(19, 0, now), "04:00");
    assert.match(localDate(reset), /Thu 19 Nov.*04:00/);
  });
  inZone("America/Los_Angeles", () => {
    assert.equal(utcToLocal(3, 0, utc(2026, 11, 18, 12, 0) * 1000), "19:00"); // ZZZ: 03:00 UTC, UTC-8 in November
    assert.equal(utcToLocal(3, 0, utc(2026, 7, 1, 12, 0) * 1000), "20:00");   // and UTC-7 in summer
  });
});

test("reset descriptions give UTC first, then local time", () => {
  inZone("Europe/Paris", () => {
    const now = utc(2026, 11, 18, 12, 0) * 1000;
    assert.equal(utcTime(3, 0), "03:00 UTC");
    assert.equal(resetText(3, 0, 0, now), "Daily 03:00 UTC (04:00 your time), weekly on Mondays (UTC)");
    assert.equal(resetText(19, 30, null, now), "Daily 19:30 UTC (20:30 your time)");
  });
});

test("an end at 13:59:59 is shown as 14:00", () => {
  inZone("UTC", () => {
    const end = utc(2026, 12, 1, 13, 59) + 59;
    assert.equal(localDate(end), "Tue 1 Dec, 14:00");
  });
});

test("how long a banner runs", () => {
  const start = utc(2026, 11, 24, 3);
  assert.equal(lasts(start, start + 14 * 86_400 - 1), "14 days"); // ends at 02:59:59
  assert.equal(lasts(start, start + 86_400), "1 day");
  assert.equal(lasts(start, start + 12 * 3600), "12 hours");
  assert.equal(lasts(start, start + 3600), "1 hour");
});

test("rough countdown for banners days away", () => {
  const now = Date.UTC(2026, 10, 18, 4, 31, 49);
  assert.equal(roughlyUntil(now / 1000 + 5 * 86_400 + 21 * 3600 + 28 * 60, now), "in 5d 21h");
  assert.equal(roughlyUntil(now / 1000 + 3 * 3600 + 5 * 60, now), "in 3h 5m");
  assert.equal(roughlyUntil(now / 1000 - 10, now), "now");
});
