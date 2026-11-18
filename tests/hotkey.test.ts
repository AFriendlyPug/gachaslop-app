// Tests for src/lib/hotkey.ts: turning a key press into the shortcut text Settings saves.
import { test } from "node:test";
import assert from "node:assert/strict";
import { hotkeyFrom } from "../src/lib/hotkey.ts";

const press = (code: string, mods: Partial<Record<"ctrlKey" | "altKey" | "shiftKey" | "metaKey", boolean>> = {}) =>
  ({ code, ctrlKey: false, altKey: false, shiftKey: false, metaKey: false, ...mods });

test("key presses become shortcut text", () => {
  assert.equal(hotkeyFrom(press("KeyG", { ctrlKey: true, altKey: true })), "Ctrl+Alt+G");
  assert.equal(hotkeyFrom(press("F9", { ctrlKey: true, shiftKey: true })), "Ctrl+Shift+F9");
  assert.equal(hotkeyFrom(press("Digit5", { metaKey: true })), "Win+5");
  assert.equal(hotkeyFrom(press("ArrowUp", { altKey: true })), "Alt+Up");
});

test("modifiers alone or odd keys aren't a shortcut yet", () => {
  assert.equal(hotkeyFrom(press("ControlLeft", { ctrlKey: true })), null);
  assert.equal(hotkeyFrom(press("Semicolon", { ctrlKey: true })), null);
});
