// Typed wrappers for the Rust commands, and the shapes they return (see src-tauri/src/commands.rs).
// All times are Unix seconds (UTC); src/lib/time.ts shows them in the PC's own time zone.
import { convertFileSrc, invoke } from "@tauri-apps/api/core";

export interface Settings {
  disabledGames: string[];
  resetToasts: boolean;
  headsupMinutes: number;
  bannerToasts: boolean;
  catchup: boolean;
  offline: boolean;
  autostart: boolean;
}

export interface GameView {
  key: string;
  name: string;
  short: string;
  color: string;
  icon: string | null;
  hour: number; // daily reset, UTC
  minute: number;
  weeklyDay: number | null; // 0 = Monday .. 6 = Sunday, UTC
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

export interface Snapshot {
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
  hour: number; // UTC
  minute: number;
  weeklyDay: number | null;
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
  /** Send a picked image's bytes; returns the staged file's path. */
  stageGameIcon: (bytes: Uint8Array) => invoke<string>("stage_game_icon", bytes),
  discardStaged: () => invoke<void>("discard_staged"),
  addGame: (game: GameInput, icon: string | null) => invoke<void>("add_game", { game, icon }),
  updateGame: (id: string, game: GameInput, icon: string | null, clearIcon: boolean) =>
    invoke<void>("update_game", { id, game, icon, clearIcon }),
  deleteGame: (id: string) => invoke<void>("delete_game", { id }),
  previewReset: (game: GameInput) => invoke<ResetPreview>("preview_reset", { game }),
  testNotification: (sample: Sample) => invoke<string>("test_notification", { sample }),
  replayCatchup: () => invoke<void>("replay_catchup"),
  setTrayOnly: (on: boolean) => invoke<void>("set_tray_only", { on }),
  diagnostics: () => invoke<Diagnostics>("diagnostics"),
  openPath: (what: "data" | "log") => invoke<void>("open_path", { what }),
  hide: () => invoke<void>("hide_popup"),
  quit: () => invoke<void>("quit"),
};
