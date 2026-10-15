import assert from "node:assert/strict";
import { test } from "node:test";
import { parseEngineEvent } from "./engineEvents.ts";

const json = (value: unknown): string => JSON.stringify(value);

// Shapes as the engine emits them: token_move.rs `emit_token_move_along` and
// `refuse_at_wall`, door_icons.rs, interaction.rs `detect_entries`, sdk.rs.

test("a token move, as a step emits it", () => {
  const event = parseEngineEvent(
    json({
      type: "upsert_token",
      token: { id: "hero", x: -351, y: 58.5, z: 2, scale: 1, rotation: 0 },
      worldId: "landing",
    }),
  );
  assert.deepEqual(event, {
    type: "upsert_token",
    token: { id: "hero", x: -351, y: 58.5 },
  });
});

test("a token move along a route keeps the route", () => {
  const path = [
    { x: 0, y: 0 },
    { x: 117, y: 0 },
  ];
  const event = parseEngineEvent(
    json({
      type: "upsert_token",
      token: { id: "hero", x: 117, y: 0, z: 2 },
      path,
    }),
  );
  assert.deepEqual(event, {
    type: "upsert_token",
    token: { id: "hero", x: 117, y: 0 },
    path,
  });
});

test("a refused move", () => {
  assert.deepEqual(
    parseEngineEvent(
      json({
        type: "movement_blocked",
        wallId: "door-hall",
        at: { x: -351, y: -292.5 },
      }),
    ),
    {
      type: "movement_blocked",
      wallId: "door-hall",
      at: { x: -351, y: -292.5 },
    },
  );
});

test("a door's icon pressed", () => {
  assert.deepEqual(
    parseEngineEvent(
      json({ type: "door_icon_pressed", wallId: "door-hall", icon: "open" }),
    ),
    {
      type: "door_icon_pressed",
      wallId: "door-hall",
      icon: "open",
    },
  );
});

test("a region entered", () => {
  assert.deepEqual(
    parseEngineEvent(
      json({
        type: "interactionTriggered",
        interactiveId: "chest-region",
        trigger: "enter",
      }),
    ),
    {
      type: "interactionTriggered",
      interactiveId: "chest-region",
      trigger: "enter",
    },
  );
});

test("an sdk error, with and without the command", () => {
  assert.deepEqual(
    parseEngineEvent(
      json({
        type: "sdkError",
        code: "malformed",
        message: "bad",
        command: "upsert_wall",
      }),
    ),
    {
      type: "sdkError",
      code: "malformed",
      message: "bad",
      command: "upsert_wall",
    },
  );
  assert.deepEqual(
    parseEngineEvent(
      json({
        type: "sdkError",
        code: "malformed",
        message: "bad",
        command: null,
      }),
    ),
    {
      type: "sdkError",
      code: "malformed",
      message: "bad",
      command: null,
    },
  );
  assert.deepEqual(
    parseEngineEvent(
      json({ type: "sdkError", code: "versionMismatch", message: "old" }),
    ),
    {
      type: "sdkError",
      code: "versionMismatch",
      message: "old",
      command: null,
    },
  );
});

test("the scene's clock, running and stopped (spec 099)", () => {
  assert.deepEqual(
    parseEngineEvent(json({ type: "time_of_day", hour: 21.5, darkness: 0.72 })),
    { type: "time_of_day", hour: 21.5, darkness: 0.72 },
  );
  assert.deepEqual(parseEngineEvent(json({ type: "time_of_day", darkness: 0 })), {
    type: "time_of_day",
    hour: null,
    darkness: 0,
  });
  assert.equal(parseEngineEvent(json({ type: "time_of_day", hour: "noon", darkness: 0 })), null);
  assert.equal(parseEngineEvent(json({ type: "time_of_day", hour: 12 })), null);
});

test("any other event is null", () => {
  assert.equal(
    parseEngineEvent(json({ type: "grid_snap_changed", enabled: true })),
    null,
  );
  assert.equal(parseEngineEvent(json({ type: "openLore", loreId: "x" })), null);
});

test("anything malformed is null", () => {
  for (const bad of [
    "",
    "not json",
    "{",
    "null",
    "42",
    '"upsert_token"',
    "[]",
    json({}),
    json({ type: 7 }),
    json({ type: "upsert_token" }),
    json({ type: "upsert_token", token: { id: "hero", x: "1", y: 0 } }),
    json({ type: "upsert_token", token: { id: 3, x: 1, y: 0 } }),
    json({
      type: "upsert_token",
      token: { id: "hero", x: 1, y: 0 },
      path: [{ x: 1 }],
    }),
    json({
      type: "upsert_token",
      token: { id: "hero", x: 1, y: 0 },
      path: "a",
    }),
    json({ type: "movement_blocked", at: { x: 0, y: 0 } }),
    json({ type: "movement_blocked", wallId: "w", at: null }),
    json({ type: "door_icon_pressed", wallId: "door-hall", icon: "smash" }),
    json({ type: "door_icon_pressed", icon: "open" }),
    json({
      type: "interactionTriggered",
      interactiveId: "chest-region",
      trigger: "click",
    }),
    json({ type: "interactionTriggered", trigger: "enter" }),
    json({ type: "sdkError", code: "malformed" }),
    json({ type: "sdkError", code: "malformed", message: "m", command: 5 }),
  ]) {
    assert.equal(parseEngineEvent(bad), null, bad);
  }
});
