import assert from "node:assert/strict";
import { test } from "node:test";
import { describePlace } from "./describe.ts";
import { provingGround } from "./fixtures.ts";
import type { RoomState } from "./referee.ts";

const script = provingGround();

const at = (
  x: number,
  y: number,
  over: Partial<RoomState> = {},
): RoomState => ({
  hero: { x, y },
  doors: { "door-hall": "closed", "door-west": "closed" },
  goals: [],
  rolls: [],
  ...over,
});

test("the start square", () => {
  assert.equal(
    describePlace(script, at(-3, 1.5)),
    "You stand in a dark room. The door to the hall is four squares north, shut.",
  );
});

test("next to the shut door, with the chest beyond it", () => {
  assert.equal(
    describePlace(script, at(-3, -1.5)),
    "You stand in a dark room. The door to the hall is within reach to the north, shut. " +
      "A chest is three squares north-east.",
  );
});

test("through the open door, in the hall", () => {
  const open = {
    doors: { "door-hall": "open" as const, "door-west": "closed" as const },
  };
  assert.equal(
    describePlace(script, at(-3, -2.5, open)),
    "You stand in the hall. The door to the hall is within reach to the south, open. " +
      "A chest is two squares north-east.",
  );
});

test("down the hall, the west door is the nearest", () => {
  assert.equal(
    describePlace(script, at(-12, -3.5)),
    "You stand in the hall. The door to the west room is three squares south-west, shut.",
  );
});

test("standing at the chest, before and after it opens", () => {
  const near = at(-1, -3.5, {
    doors: { "door-hall": "open", "door-west": "closed" },
  });
  assert.match(describePlace(script, near), / A chest is here\.$/);
  const opened = {
    ...near,
    goals: [
      "door-opened" as const,
      "chest-found" as const,
      "chest-opened" as const,
    ],
  };
  assert.match(describePlace(script, opened), / An open chest is here\.$/);
});

test("a chest more than five squares off is not mentioned", () => {
  assert.doesNotMatch(describePlace(script, at(-3, 1.5)), /chest/);
});
