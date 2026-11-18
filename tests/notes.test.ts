// Tests for src/lib/notes.ts: release notes as plain lines for the update banner.
import { test } from "node:test";
import assert from "node:assert/strict";
import { noteLines } from "../src/lib/notes.ts";

test("changelog bullets become plain lines", () => {
  const md = "## What's new\n\n- **Faster** popup, see [the notes](https://example.com).\r\n* Fixed `widget` spacing\nThanks for using it.";
  assert.deepEqual(noteLines(md), [
    { kind: "heading", text: "What's new" },
    { kind: "item", text: "Faster popup, see the notes." },
    { kind: "item", text: "Fixed widget spacing" },
    { kind: "text", text: "Thanks for using it." },
  ]);
  assert.deepEqual(noteLines(""), []);
});
