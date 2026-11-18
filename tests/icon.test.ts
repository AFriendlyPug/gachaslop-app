// Tests for src/lib/icon.ts: which part of an image becomes the square icon.
import { test } from "node:test";
import assert from "node:assert/strict";
import { squareCrop } from "../src/lib/icon.ts";

test("icons use the centred square of the image", () => {
  assert.deepEqual(squareCrop(400, 200), { x: 100, y: 0, size: 200 }); // wide: the middle
  assert.deepEqual(squareCrop(120, 301), { x: 0, y: 90, size: 120 }); // tall
  assert.deepEqual(squareCrop(64, 64), { x: 0, y: 0, size: 64 }); // already square
});
