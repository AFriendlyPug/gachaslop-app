// Typed wrappers for the Rust commands, and the shapes they return (see src-tauri/src/commands.rs).
// All times are Unix seconds (UTC); src/lib/time.ts shows them in the PC's own time zone.
import { convertFileSrc, invoke } from "@tauri-apps/api/core";

/** Which notifications one game sends, on top of the switches for all games. */
export interface Alerts {
  resets: boolean;
  headsup: boolean;
  banners: boolean;
}

export interface Settings {
  disabledGames: string[];
  resetToasts: boolean;
  headsupMinutes: number;
  bannerToasts: boolean;
  catchup: boolean;
  offline: boolean;
  autostart: boolean;
  updateChecks: boolean; // look for a new version once a day (never installs by itself)
  trayIcon: boolean; // a tray icon instead of the taskbar widget
  gameAlerts: Record<string, Alerts>; // per game; a game not listed gets everything
  hotkeyOn: boolean; // the keyboard shortcut is off unless turned on
  hotkey: string; // the shortcut that opens and closes the popup, e.g. "Ctrl+Alt+G"
  freeMemory: boolean; // destroy the popup's web view a while after it's closed
}

export interface GameView {
  key: string;
  name: string;
  short: string;
  color: string;
  icon: string | null;
  hour: number; // daily reset, UTC
  minute: number;
  weeklyDay: number | null; // 0 = Monday .. 6 = Sunday, UTC (or in `zone`)
  zone: string | null; // a Windows time zone id the reset times are in; null for UTC
  zoneName: string | null; // its name as Windows shows it
  feed: string | null;
  builtin: boolean;
  iconPath: string | null;
  enabled: boolean;
  nextReset: number;
  nextWeekly: number | null;
}

export interface PickView {
  name: string;
  tags: string[]; // "Fest", "Limited", "Free pulls"
  iconPath: string | null;
  engine: PickView | null; // a ZZZ agent's signature W-Engine
}

export interface RotationView {
  key: string;
  start: number;
  end: number;
  confirmed: boolean;
  picks: PickView[];
  extras: PickView[];
  details: string[];
}

export interface FeedView {
  key: string;
  name: string;
  credit: string;
  projected: boolean;
  enabled: boolean;
  fetchedAt: number | null;
  checkedAt: number | null;
  nextCheck: number | null;
  lastError: string | null;
  rotations: RotationView[];
  nextUnknownFrom: number | null;
}

/** A notification that came in (newest first in the snapshot). */
export interface Notice {
  at: number; // when it happened
  added: number; // when the app noticed it: later than `at` if it was missed
  game: string;
  title: string;
  body: string;
  missed: boolean; // happened while the app was closed or the PC slept
  new: boolean; // came in since the Notifications list was last opened
}

/** A newer version on GitHub (see src-tauri/src/update.rs). */
export interface UpdateView {
  version: string;
  page: string;
  installer: string;
  size: number;
  sha256: string | null;
  published: number | null;
  notes: string; // what's new, Markdown from CHANGELOG.md
  dismissed: boolean; // "Not now" on its banner
}

export interface Snapshot {
  version: string; // this build's
  hotkeyProblem: string | null; // why the saved shortcut couldn't be set at startup
  update: UpdateView | null;
  updateChecked: number | null;
  now: number;
  games: GameView[];
  feeds: FeedView[];
  settings: Settings;
  notices: Notice[];
  newNotices: number;
  requestsThisMonth: number;
}

/** Kinds of test notification (Settings, Testing). */
export type Sample = "reset" | "weekly" | "headsUp" | "bannerLive" | "bannerEnding" | "summary";

export interface Diagnostics {
  widget: { strips: number; trayShown: boolean; trayOnly: boolean };
  remembered: number;
  icons: number;
  dataDir: string;
}

export interface GameInput {
  name: string;
  hour: number; // UTC, or in `zone`
  minute: number;
  weeklyDay: number | null;
  zone: string | null;
}

/** A time zone a custom game can follow (Windows' id and display name). */
export interface Zone {
  id: string;
  name: string;
}

export interface ResetPreview {
  nextReset: number;
  nextWeekly: number | null;
}

/** A stored file the popup may show (only the app's own icon folders are allowed). */
export const fileUrl = (path: string | null) => (path ? convertFileSrc(path) : null);

/** A built-in game's icon, shipped with the app. */
export const builtinIcon = (key: string) => `/games/${key}.png`;

export const api = {
  snapshot: () => invoke<Snapshot>("get_snapshot"),
  saveSettings: (settings: Settings) => invoke<void>("save_settings", { settings }),
  refreshBanners: () => invoke<string>("refresh_banners"),
  markNoticesRead: () => invoke<void>("mark_notices_read"),
  clearNotices: () => invoke<void>("clear_notices"),
  setDialogOpen: (open: boolean) => invoke<void>("set_dialog_open", { open }),
  /** Let go of the current shortcut while a new one is being recorded. */
  suspendHotkey: (suspended: boolean) => invoke<void>("suspend_hotkey", { suspended }),
  /** Send a picked image's bytes; returns the staged file's path. */
  stageGameIcon: (bytes: Uint8Array) => invoke<string>("stage_game_icon", bytes),
  discardStaged: () => invoke<void>("discard_staged"),
  /** Download an image from a link (HTTPS); its bytes, for the popup to turn into an icon. */
  fetchIcon: (url: string) => invoke<ArrayBuffer>("fetch_icon", { url }),
  addGame: (game: GameInput, icon: string | null) => invoke<void>("add_game", { game, icon }),
  updateGame: (id: string, game: GameInput, icon: string | null, clearIcon: boolean) =>
    invoke<void>("update_game", { id, game, icon, clearIcon }),
  deleteGame: (id: string) => invoke<void>("delete_game", { id }),
  previewReset: (game: GameInput) => invoke<ResetPreview>("preview_reset", { game }),
  timeZones: () => invoke<Zone[]>("list_time_zones"),
  testNotification: (sample: Sample) => invoke<string>("test_notification", { sample }),
  replayCatchup: () => invoke<void>("replay_catchup"),
  diagnostics: () => invoke<Diagnostics>("diagnostics"),
  openPath: (what: "data" | "log" | "release") => invoke<void>("open_path", { what }),
  checkForUpdate: () => invoke<string>("check_for_update"),
  /** Downloads and runs the installer; the app closes on success, so this only returns on failure. */
  installUpdate: () => invoke<void>("install_update"),
  dismissUpdate: () => invoke<void>("dismiss_update"),
  hide: () => invoke<void>("hide_popup"),
  /** The height (CSS pixels) the popup's sections need, with dropdowns closed. */
  fitPopup: (height: number) => invoke<void>("fit_popup", { height }),
  /** The view the popup was asked to open on before it existed ("settings"), if any. */
  takeView: () => invoke<"settings" | null>("take_view"),
  quit: () => invoke<void>("quit"),
};
