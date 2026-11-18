# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

A personal Windows app (Tauri 2: Rust core + a small WebView2 popup in Svelte 5) that counts down to game resets and shows current and upcoming banners for Blue Archive (Global) and Zenless Zone Zero (EU). It draws a small strip on every screen's taskbar, just left of the clock (`BA 02:15:07` / `ZZZ 10:15:07`); clicking it opens the popup, right-clicking shows the menu. It sends Windows notifications for resets, heads-ups, new banners and banners ending, and catches up on anything missed while it was closed. Users can add their own games (icon, name, UTC reset time), which get resets but never banners. See [README.md](README.md) for features, data sources and where files are kept.

It has **nothing to do with Discord**. The GachaSlop Discord bot (`E:\Github\GachaSlop`) was only the reference for the reset maths and the banner parsers; nothing is imported from or connected to it.

## Commands

In Windows PowerShell, use `npm.cmd` if script running is disabled (`npm` fails with "running scripts is disabled").

```
npm install
npm.cmd run tauri dev      # run with live reload (rebuilds and restarts the app when src-tauri changes)
npm.cmd run tauri build    # NSIS installer in src-tauri\target\release\bundle\nsis (about 1.3 MB)
npm.cmd test               # JS tests: Node's built-in runner on tests/ (no test package)
npm.cmd run check          # svelte-check type check
cd src-tauri; cargo test   # Rust tests
cd src-tauri; cargo clippy --all-targets   # keep this warning-free
```

GitHub builds only on version tags (a Release with the installer) or the "Run workflow" button; see `.github/workflows/build.yml` and the README's "Building on GitHub".

`tauri dev` keeps Vite on port 1420; a second `tauri dev` fails with "Port 1420 is already in use" while the first is running. Only one copy of the app runs at a time (see `instance.rs`): launching it again shows the running copy's popup.

## Architecture

### Rust (`src-tauri/src`)

`lib.rs` wires everything up. Each module only uses those above it in this list:
- `time.rs`: `Ts` (Unix seconds, UTC), ISO parsing, calendar maths, `fmt_clock` ("02:15:07") and `fmt_duration` ("2h 15m"). Replaces chrono.
- `config.rs`: URLs, timings (in seconds) and limits. No side effects.
- `logs.rs`: `info()`/`warn()` to `%LOCALAPPDATA%\com.gachaslop.app\logs\GachaSlop.log`.
- `net.rs`: HTTP GET through Windows' own `curl.exe`, and `hidden()` for running System32 tools without a console window.
- `games.rs`: built-in games (BA 19:00 UTC weekly Sunday; ZZZ EU 03:00 UTC weekly Monday), `CustomGame`, reset maths (`next_reset`, `last_reset`, `next_weekly`, `ordered`).
- `banners.rs`: `FeedKind`, `Rotation`, `FeedState`, both parsers, when to fetch (`data_runs_out`, `record`), icon download and `prune_icons`.
- `settings.rs`: `Settings`, `State`, `Paths`, JSON load/save (atomic; an unreadable file is renamed `.bad` and defaults are used).
- `store.rs`: `Store`, the shared in-memory copy of everything, with a `Condvar` to wake the scheduler.
- `autostart.rs`: "Start with Windows" via `reg.exe` on `HKCU\...\Run`.
- `instance.rs`: one copy at a time via a local port (47613).
- `tray.rs`: popup show/hide/placement (`place()`), shared menu actions (`menu_action`), and the tray icon, which is only shown when the taskbar widget can't be.
- `widget.rs`: the taskbar strips (one per taskbar) drawn with Win32 (`windows-sys`).
- `scheduler.rs`: one thread: fetch due feeds, work out due events, show toasts and catch-up, keep the tray in sync.
- `commands.rs`: the commands the popup calls, and the `Snapshot` it renders.

Tests live in `src-tauri/src/tests/` (inside the crate so they can reach private modules; a top-level `src-tauri/tests/` would be compiled as integration tests and couldn't). Sample API responses are in `src-tauri/fixtures/`.

### Popup (`src/`)

`App.svelte` (home, settings and editor views; ticks `now` every second while visible), `components/GameCard.svelte` (one card per enabled game: countdowns plus that game's banners, upcoming in a `<details>` dropdown), `GameBadge.svelte`, `GameEditor.svelte` (add/edit custom games; picks images with a plain `<input type="file">` and sends the bytes as a raw invoke body), `Settings.svelte`. `lib/api.ts` has typed `invoke()` wrappers; `lib/time.ts` turns UTC seconds into local time and countdowns. Built-in game icons are `public/games/{ba,zzz}.png`.

### Times

Everything is UTC (`Ts`, Unix seconds) in Rust, in saved files and over IPC. The popup converts to the PC's time zone with `Date`. Rust has no time zone database, so notification text uses relative times only ("Reset 3h ago", "Ends in 6d 0h"). Custom games are entered in UTC; old `games.json` entries with `offsetMinutes` are converted on load (`CustomGame::in_utc`).

### Network: as few requests as possible

Banners are stored in `banners.json` and fetched only when the stored data runs out, never on a timer, at startup, on wake or on opening the popup:
- BA (`ba.joexyz.online`): future banners are projections from the JP schedule, so the feed is checked as each stored upcoming banner is due to start, and a "banner live" toast only fires once a fetch made after its start still lists it (`FeedState::confirmed`).
- ZZZ (`api.ennead.cc`): lists only the current phase, so it's checked when that phase ends.
- Nothing new or a failure: back off 30 min, 2 h, 6 h, then every 12 h. Manual refresh is skipped if fetched under 5 minutes ago. Offline mode makes no requests at all.
- Icons are downloaded once for current and upcoming banners and deleted when no stored banner uses them.

### Notifications and catch-up

`due_events` lists every currently due event (latest daily and weekly reset, heads-up, banner live, banner ending); the notified-keys log in `state.json` decides what's new. There is no catch-up time window: anything missed while the app was closed or the PC slept is shown on load (several days of missed dailies collapse into one), three or more become one summary, and missed items go on the "While you were away" card plus a red dot on the widget. First run, and adding or switching on a game, seed the log so no stale toasts fire.

### Taskbar widget (`widget.rs`)

Modelled on [Wireless Battery](https://github.com/krisgfx9/Wireless-Device-Battery-Monitor): a layered `WS_CHILD` window parented to each taskbar (`Shell_TrayWnd`, and on Windows 10 every `Shell_SecondaryTrayWnd`), placed left of the notification area or the secondary taskbar's `ClockButton`, repainted and re-placed once a second by a hidden owner window's timer, recreated on `TaskbarCreated`. On Windows 10 the task buttons (`ReBarWindow32` / `WorkerW`) are narrowed so they never slide under it, and given back on quit. Falls back to the tray icon when the main taskbar is vertical, auto-hides or can't be found. Things that matter here:
- The background must never be alpha 0: Windows passes clicks on fully transparent pixels to the taskbar behind (`background()` uses alpha 1).
- A layered child window needs the Windows 8+ `supportedOS` entries in `src-tauri/windows-app-manifest.xml` (applied by `build.rs`); Tauri's default manifest doesn't have them.
- Clicks are posted to the owner window rather than handled inside the taskbar's child window.
- Check that a Win32 constant exists in the imported `windows-sys` modules before matching on it: an unknown name in a `match` arm becomes a catch-all binding (this happened with `WM_MOUSELEAVE`, now defined locally). Clippy flags it.

### Threads and locks

Tauri runs commands, tray events and the widget's window procedure on the main thread; the scheduler has its own thread. Never call tray or window APIs while holding `Store::lock()`: those calls wait for the main thread, which may be waiting for the same lock (this deadlocked once; see the comment on `tray::sync`). Take what you need, drop the guard, then touch the UI. The widget's thread-local state uses `try_borrow` in its window procedure, since `TrackPopupMenu` runs a nested message loop.

## Conventions

- **Keep packages to a minimum.** Rust uses only `tauri`, `tauri-plugin-notification`, `serde`, `serde_json` and `windows-sys` (already built by Tauri). Before adding a crate or npm package, check whether std, an existing dependency, a Windows tool, or a little own code can do it, and say why if not.
- **Offline first:** anything fetched is stored and used from disk; give every cached file a clean-up rule.
- **Built-in games are only BA and ZZZ;** custom games never get banners or cause network requests.
- British English in user-facing text and docs; no em dashes in user-facing strings (use `·`, commas or full stops).
- Comments explain why, briefly, in the style already in the files. Pure logic stays in plain functions so it can be tested; new behaviour comes with tests in `src-tauri/src/tests/` or `tests/`.
- When checking the app live, don't take screenshots of the user's screen beyond the app's own windows: they may be running a game full screen.
