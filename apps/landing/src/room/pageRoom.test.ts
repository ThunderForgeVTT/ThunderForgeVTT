import assert from "node:assert/strict";
import { test } from "node:test";
import { HERO_ID } from "@thunderforge/encounter";
import { fromPage, toPage } from "./pageRoom.ts";

// Run with `node --test src/room/pageRoom.test.ts` (Node strips the types).

/** A 128-unit square on a 48-pixel cell, the room's origin at (100, -2000). */
const AT = { x: 100, y: -2000, k: 48 / 128, walker: "walker" };

const one = (c: Record<string, unknown>) => toPage(c, AT)[0] as Record<string, unknown>;

test("walls, lights and regions land on the page, scaled to its cell", () => {
  const wall = { id: "w", x1: 0, y1: 0, x2: 256, y2: -128, doorState: "closed" };
  assert.deepEqual(one({ type: "upsert_wall", wall }), {
    type: "upsert_wall",
    wall: { id: "w", x1: 100, y1: -2000, x2: 196, y2: -2048, doorState: "closed" },
  });
  const light = { id: "l", x: 128, y: 128, radius: 512, brightRadius: 256 };
  assert.deepEqual(one({ type: "upsert_light", light }).light, {
    id: "l",
    x: 148,
    y: -1952,
    radius: 192,
    brightRadius: 96,
  });
  const geometry = { shape: "rect", x: 128, y: 0, width: 384, height: 256 };
  const region = one({ type: "upsert_interactive", interactive: { id: "r", geometry } });
  assert.deepEqual((region.interactive as { geometry: unknown }).geometry, {
    shape: "rect",
    x: 148,
    y: -2000,
    width: 144,
    height: 96,
  });
});

test("the room's hero is the page's walker, and the walker stays when the room goes", () => {
  const token = one({ type: "upsert_token", token: { id: HERO_ID, x: 256, y: 448, label: "Ash" } });
  assert.deepEqual(token.token, { id: "walker", x: 196, y: -1832, label: "Ash" });
  assert.deepEqual(one({ type: "set_carried_light", tokenId: HERO_ID, bright: 512, dim: 1024 }), {
    type: "set_carried_light",
    tokenId: "walker",
    bright: 192,
    dim: 384,
  });
  assert.deepEqual(toPage({ type: "remove_token", tokenId: HERO_ID }, AT), []);
  const chest = { type: "remove_token", tokenId: "chest" };
  assert.deepEqual(toPage(chest, AT), [chest]);
  assert.deepEqual(one({ type: "set_controlled_token", tokenId: null }), {
    type: "set_controlled_token",
    tokenId: "walker",
  });
});

test("the page keeps its grid, background, camera and play", () => {
  for (const type of ["set_scene_grid", "set_scene_background", "fit_camera_to", "set_scene_playing"]) {
    assert.deepEqual(toPage({ type }, AT), []);
  }
  const dark = { type: "set_ambient_light", level: "dark" };
  assert.deepEqual(toPage(dark, AT), [dark]);
});

test("the engine's answers come back in the room's units", () => {
  const moved = fromPage(
    { type: "upsert_token", token: { id: "walker", x: 196, y: -1832 }, path: [{ x: 100, y: -2000 }] },
    AT,
  );
  assert.deepEqual(moved, {
    type: "upsert_token",
    token: { id: HERO_ID, x: 256, y: 448 },
    path: [{ x: 0, y: 0 }],
  });
  const bump = fromPage(
    { type: "movement_blocked", tokenId: "walker", wallId: "w", at: { x: 148, y: -2000 } },
    AT,
  );
  assert.deepEqual(bump, {
    type: "movement_blocked",
    tokenId: HERO_ID,
    wallId: "w",
    at: { x: 128, y: 0 },
  });
  assert.equal(fromPage({ type: "cameraFitted", centerX: 0, centerY: 0 }, AT), null);
});
