// Keyboard shortcuts as Settings shows and saves them ("Ctrl+Alt+G"); src-tauri/src/widget.rs reads them.

type KeyPress = Pick<KeyboardEvent, "code" | "ctrlKey" | "altKey" | "shiftKey" | "metaKey">;

const NAMED: Record<string, string> = {
  Space: "Space", PageUp: "PageUp", PageDown: "PageDown", End: "End", Home: "Home", Insert: "Insert",
  ArrowLeft: "Left", ArrowUp: "Up", ArrowRight: "Right", ArrowDown: "Down",
};

/** The key of a press, by position on the keyboard ("KeyG" is G whatever the layout), or null if it can't be used. */
function keyName(code: string): string | null {
  if (/^Key[A-Z]$/.test(code)) return code.slice(3);
  if (/^Digit[0-9]$/.test(code)) return code.slice(5);
  if (/^F([1-9]|1[0-9]|2[0-4])$/.test(code)) return code;
  return NAMED[code] ?? null;
}

/** The shortcut a key press makes, e.g. "Ctrl+Alt+G"; null while only modifiers are held or for unusable keys. */
export function hotkeyFrom(e: KeyPress): string | null {
  const key = keyName(e.code);
  if (!key) return null;
  const mods = [e.ctrlKey && "Ctrl", e.altKey && "Alt", e.shiftKey && "Shift", e.metaKey && "Win"].filter(Boolean);
  return [...mods, key].join("+");
}
