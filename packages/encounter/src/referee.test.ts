import assert from "node:assert/strict";
import { test } from "node:test";
import { provingGround, smallRoom, walkThrough } from "./fixtures.ts";
import {
  ANNOUNCEMENTS,
  createReferee,
  type Direction,
  type Judged,
  type Referee,
  type RoomState,
} from "./referee.ts";
import { goalsOf } from "./script.ts";

const total = (value: number) => ({
  formula: "1d20+2",
  kind: { Total: value },
});

/** Take steps one after another, keeping every outcome. */
function walk(referee: Referee, state: RoomState, dirs: Direction[]): Judged[] {
  const out: Judged[] = [];
  for (const dir of dirs) {
    const judged = referee.step(state, dir);
    out.push(judged);
    state = judged.state;
  }
  return out;
}

const last = (judged: Judged[]): Judged => judged[judged.length - 1]!;

const small = createReferee(smallRoom());
const real = createReferee(provingGround());

test("the starting state: the hero on the start square, doors shut, nothing met", () => {
  assert.deepEqual(small.start(), {
    hero: { x: 0, y: 1.5 },
    doors: { "door-hall": "closed", "door-west": "closed" },
    goals: [],
    rolls: [],
  });
});

test("a step into a wall is refused and the hero stays", () => {
  const { state, outcome } = small.step(small.start(), "south");
  assert.equal(outcome.accepted, false);
  assert.equal(outcome.announcement, "blocked-by-wall");
  assert.equal(outcome.wallId, "wall-03");
  assert.equal(outcome.goal, null);
  assert.deepEqual(state.hero, { x: 0, y: 1.5 });
});

test("a step that is clear moves the hero one square", () => {
  const { state, outcome } = small.step(small.start(), "north");
  assert.equal(outcome.accepted, true);
  assert.equal(outcome.announcement, "moved");
  assert.deepEqual(state.hero, { x: 0, y: 0.5 });
  assert.deepEqual(small.step(state, "east").state.hero, { x: 1, y: 0.5 });
  assert.deepEqual(small.step(state, "west").state.hero, { x: -1, y: 0.5 });
});

test("on the real map, walking west stops at the room's west wall", () => {
  const steps = walk(real, real.start(), Array<Direction>(7).fill("west"));
  assert.deepEqual(
    steps.slice(0, 6).map((s) => s.outcome.accepted),
    [true, true, true, true, true, true],
  );
  assert.equal(last(steps).outcome.wallId, "wall-24");
  assert.deepEqual(last(steps).state.hero, { x: -9, y: 1.5 });
});

test("coming within reach of door-hall while it is shut marks door-reached, once", () => {
  // The small room's door is north of its start; the real hall is south.
  // Reaching the door is being near enough to open it, not bumping it: a
  // drag that stops beside the door never bumps it (spec 091).
  for (const [referee, toward] of [
    [small, "north"],
    [real, "south"],
  ] as const) {
    const steps = walk(referee, referee.start(), Array(5).fill(toward));
    const reached = steps.findIndex((s) => s.outcome.goal === "door-reached");
    assert.ok(reached >= 0 && reached < 3, `reached on step ${reached + 1}`);
    assert.equal(steps[reached]!.outcome.accepted, true);
    assert.equal(steps[reached]!.outcome.announcement, "moved");
    const [fourth, fifth] = [steps[3]!, steps[4]!];
    assert.equal(fourth.outcome.accepted, false);
    assert.equal(fourth.outcome.wallId, "door-hall");
    assert.equal(fourth.outcome.announcement, "blocked-by-door");
    assert.equal(fourth.outcome.goal, null, "the bump marks nothing more");
    assert.equal(fifth.outcome.goal, null);
    assert.deepEqual(fifth.state.goals, ["door-reached"]);
  }
});

test("a refusal at door-west or at a plain wall marks nothing", () => {
  const steps = walk(small, small.start(), ["north", "west", "west", "west"]);
  assert.equal(last(steps).outcome.wallId, "door-west");
  assert.equal(last(steps).outcome.announcement, "blocked-by-door");
  assert.equal(last(steps).outcome.goal, null);
  const wall = small.step(small.start(), "south");
  assert.equal(wall.outcome.goal, null);
  assert.deepEqual(last(steps).state.goals, []);
});

test('"open the door" out of reach is refused, with the reason', () => {
  const { state, outcome } = small.toggleDoor(small.start());
  assert.equal(outcome.accepted, false);
  assert.equal(outcome.announcement, "no-door-in-reach");
  assert.ok(outcome.reason && outcome.reason.length > 0);
  assert.deepEqual(state.doors, small.start().doors);
});

test('"open the door" in reach opens it and marks door-opened', () => {
  const near = last(
    walk(small, small.start(), ["north", "north", "north"]),
  ).state;
  const { state, outcome } = small.toggleDoor(near);
  assert.equal(outcome.accepted, true);
  assert.equal(outcome.announcement, "door-opened");
  assert.deepEqual(outcome.door, { id: "door-hall", state: "open" });
  assert.equal(outcome.goal, "door-opened");
  assert.equal(state.doors["door-hall"], "open");
});

test("a door one square off is in reach, and one two squares off is not", () => {
  const oneOff = last(walk(small, small.start(), ["north", "north"])).state;
  assert.deepEqual(oneOff.hero, { x: 0, y: -0.5 });
  assert.equal(small.toggleDoor(oneOff).outcome.accepted, true);
  const twoOff = small.step(small.start(), "north").state;
  assert.equal(small.toggleDoor(twoOff).outcome.accepted, false);
});

test("door-west opens too, but it is no goal", () => {
  const near = last(
    walk(small, small.start(), ["north", "west", "west"]),
  ).state;
  const { state, outcome } = small.toggleDoor(near);
  assert.deepEqual(outcome.door, { id: "door-west", state: "open" });
  assert.equal(outcome.goal, null);
  assert.deepEqual(state.goals, []);
  // And through it the hero goes.
  assert.equal(small.step(state, "west").outcome.accepted, true);
});

test("opening an open door closes it, and no goal is unmarked", () => {
  const near = last(
    walk(small, small.start(), ["north", "north", "north", "north"]),
  ).state;
  const opened = small.toggleDoor(near).state;
  const { state, outcome } = small.toggleDoor(opened);
  assert.equal(outcome.announcement, "door-closed");
  assert.deepEqual(outcome.door, { id: "door-hall", state: "closed" });
  assert.equal(outcome.goal, null);
  assert.deepEqual(state.goals, ["door-reached", "door-opened"]);
  const again = small.toggleDoor(state);
  assert.equal(
    again.outcome.goal,
    null,
    "opening it a second time marks nothing",
  );
  assert.deepEqual(again.state.goals, ["door-reached", "door-opened"]);
});

test("a door pressed on the engine is judged like the button", () => {
  const near = last(
    walk(small, small.start(), ["north", "north", "north"]),
  ).state;
  const pressed = small.doorPressed(near, "door-hall");
  assert.deepEqual(pressed.outcome.door, { id: "door-hall", state: "open" });
  assert.equal(pressed.outcome.goal, "door-opened");
  assert.equal(
    small.doorPressed(small.start(), "door-hall").outcome.accepted,
    false,
  );
  assert.equal(
    small.doorPressed(near, "wall-00").outcome.announcement,
    "ignored",
  );
});

test("the chest region entered marks chest-found once; away and back marks nothing more", () => {
  const near = last(
    walk(small, small.start(), ["north", "north", "north"]),
  ).state;
  const open = small.toggleDoor(near).state;
  const steps = walk(small, open, ["north", "north", "south", "north"]);
  assert.deepEqual(
    steps.map((s) => s.outcome.goal),
    [null, "chest-found", null, null],
  );
  assert.equal(steps[1]!.outcome.announcement, "chest-found");
  // The engine's report of the same entry marks nothing more.
  const reported = small.entered(last(steps).state, "chest-region");
  assert.equal(reported.outcome.goal, null);
  assert.deepEqual(reported.state.goals, [
    "door-reached",
    "door-opened",
    "chest-found",
  ]);
});

test("the engine's region report marks chest-found, and any other region is ignored", () => {
  const { state, outcome } = small.entered(small.start(), "chest-region");
  assert.equal(outcome.goal, "chest-found");
  assert.deepEqual(state.goals, ["chest-found"]);
  assert.equal(
    small.entered(small.start(), "elsewhere").outcome.announcement,
    "ignored",
  );
});

test("on the real map: through the door and east into the chest's region", () => {
  const near = last(
    walk(real, real.start(), ["south", "south", "south", "south"]),
  ).state;
  const opened = real.toggleDoor(near);
  assert.equal(opened.outcome.goal, "door-opened");
  const steps = walk(real, opened.state, ["south", "south", "east"]);
  assert.deepEqual(
    steps.map((s) => s.outcome.goal),
    [null, null, "chest-found"],
  );
  assert.deepEqual(last(steps).state.hero, { x: -2, y: -3.5 });
});

const found = (): RoomState =>
  small.entered(small.start(), "chest-region").state;

test("a roll before chest-found is refused", () => {
  const { state, outcome } = small.rolled(small.start(), total(20));
  assert.equal(outcome.accepted, false);
  assert.equal(outcome.announcement, "roll-too-soon");
  assert.deepEqual(state.rolls, []);
});

test("11 fails, 12 opens the chest; a failed roll may be rolled again; each try is kept", () => {
  const failed = small.rolled(found(), total(11));
  assert.equal(failed.outcome.announcement, "roll-failed");
  assert.equal(failed.outcome.goal, null);
  const opened = small.rolled(failed.state, total(12));
  assert.equal(opened.outcome.announcement, "roll-succeeded");
  assert.equal(opened.outcome.goal, "chest-opened");
  assert.deepEqual(opened.state.rolls, [
    { total: 11, dc: 12, success: false },
    { total: 12, dc: 12, success: true },
  ]);
  const again = small.rolled(opened.state, total(20));
  assert.equal(again.outcome.announcement, "chest-already-open");
  assert.equal(again.outcome.goal, null);
  assert.equal(again.state.rolls.length, 2);
});

test("a roll that is not a total is refused", () => {
  const { outcome } = small.rolled(found(), {
    formula: "4dF",
    kind: { SuccessCount: 3 },
  });
  assert.equal(outcome.accepted, false);
  assert.equal(outcome.announcement, "ignored");
});

test("a wall the engine refused on its own is judged as the step would be", () => {
  const start = small.start();
  const door = small.refused(start, "door-hall");
  assert.equal(door.outcome.accepted, false);
  assert.equal(door.outcome.announcement, "blocked-by-door");
  assert.equal(door.outcome.goal, "door-reached");
  assert.equal(door.outcome.rollback, undefined, "the engine never moved");
  assert.deepEqual(door.state.hero, start.hero);
  const wall = small.refused(start, "wall-03");
  assert.equal(wall.outcome.announcement, "blocked-by-wall");
  assert.equal(wall.outcome.goal, null);
  assert.equal(
    small.refused(start, "no-such-wall").outcome.announcement,
    "ignored",
  );
});

test("a move the engine reported and the rule refuses returns the last accepted square", () => {
  const start = small.start();
  const { state, outcome } = small.judgeMove(
    start,
    { x: 0, y: 150 },
    { x: 0, y: 250 },
  );
  assert.equal(outcome.accepted, false);
  assert.equal(outcome.announcement, "rolled-back");
  assert.equal(outcome.wallId, "wall-03");
  assert.deepEqual(outcome.rollback, { x: 0, y: 1.5 });
  assert.deepEqual(state.hero, start.hero);
});

test("a move the engine reported and the rule allows is accepted where it landed", () => {
  const { state, outcome } = small.judgeMove(
    small.start(),
    { x: 0, y: 150 },
    { x: 200, y: -150 },
  );
  assert.equal(outcome.accepted, true);
  assert.equal(outcome.rollback, undefined);
  assert.deepEqual(state.hero, { x: 2, y: -1.5 });
});

test("a drag that stops beside the shut goal door reaches it", () => {
  const { state, outcome } = small.judgeMove(
    small.start(),
    { x: 0, y: 150 },
    { x: 0, y: -150 },
  );
  assert.equal(outcome.accepted, true);
  assert.equal(outcome.goal, "door-reached");
  assert.deepEqual(state.goals, ["door-reached"]);
  // Dragged there again, or pressed into the door, it is marked no more.
  assert.equal(
    small.judgeMove(state, { x: 0, y: -150 }, { x: 0, y: -150 }).outcome.goal,
    null,
  );
  assert.equal(small.refused(state, "door-hall").outcome.goal, null);
});

test("a path is accepted only if every leg is", () => {
  const start = small.start();
  // Out through the east wall and back in: both ends are in the room.
  const outAndBack = [
    { x: 0, y: 150 },
    { x: 300, y: 150 },
    { x: 300, y: 50 },
    { x: 0, y: 50 },
  ];
  const refused = small.judgeMove(
    start,
    outAndBack[0]!,
    outAndBack[3]!,
    outAndBack,
  );
  assert.equal(refused.outcome.accepted, false);
  assert.equal(refused.outcome.wallId, "wall-04");
  assert.deepEqual(refused.state.hero, start.hero);
  const around = [
    { x: 0, y: 150 },
    { x: 100, y: 150 },
    { x: 100, y: 50 },
  ];
  const accepted = small.judgeMove(start, around[0]!, around[2]!, around);
  assert.equal(accepted.outcome.accepted, true);
  assert.deepEqual(accepted.state.hero, { x: 1, y: 0.5 });
});

test("a walk-through room is left through its door: room-left, once", () => {
  const through = createReferee(walkThrough());
  const near = last(
    walk(through, through.start(), ["north", "north", "north"]),
  ).state;
  const open = through.toggleDoor(near).state;
  const out = through.step(open, "north");
  assert.equal(out.outcome.announcement, "room-left");
  assert.equal(out.outcome.goal, "room-left");
  assert.deepEqual(out.state.goals, [
    "door-reached",
    "door-opened",
    "room-left",
  ]);
  const back = through.step(out.state, "south");
  const again = through.step(back.state, "north");
  assert.equal(again.outcome.goal, null, "met once");
});

test("a walk-through room has no chest to find or roll for", () => {
  const through = createReferee(walkThrough());
  const start = through.start();
  assert.equal(
    through.entered(start, "chest-region").outcome.announcement,
    "ignored",
  );
  assert.equal(
    through.rolled(start, total(20)).outcome.announcement,
    "ignored",
  );
});

test("goalsOf: the chest's goals for a chest, room-left for an exit", () => {
  assert.deepEqual(goalsOf(smallRoom()), [
    "door-reached",
    "door-opened",
    "chest-found",
    "chest-opened",
  ]);
  assert.deepEqual(goalsOf(walkThrough()), [
    "door-reached",
    "door-opened",
    "room-left",
  ]);
});

test("every outcome carries one announcement key", () => {
  let state = small.start();
  const outcomes = [];
  const actions: Array<(s: RoomState) => Judged> = [
    (s) => small.step(s, "south"),
    (s) => small.step(s, "north"),
    (s) => small.toggleDoor(s),
    (s) => small.step(s, "north"),
    (s) => small.step(s, "north"),
    (s) => small.step(s, "north"),
    (s) => small.toggleDoor(s),
    (s) => small.toggleDoor(s),
    (s) => small.doorPressed(s, "door-hall"),
    (s) => small.rolled(s, total(5)),
    (s) => small.entered(s, "chest-region"),
    (s) => small.rolled(s, total(5)),
    (s) => small.rolled(s, total(15)),
    (s) => small.judgeMove(s, { x: 0, y: -150 }, { x: 0, y: 500 }),
    () => small.reset(),
  ];
  for (const act of actions) {
    const judged = act(state);
    outcomes.push(judged.outcome);
    state = judged.state;
  }
  for (const outcome of outcomes) {
    assert.equal(typeof outcome.announcement, "string");
    assert.ok(
      ANNOUNCEMENTS.includes(outcome.announcement),
      outcome.announcement,
    );
  }
});

test("reset() returns the starting state exactly", () => {
  const before = small.start();
  const steps = walk(small, before, ["north", "north", "north", "north"]);
  const played = small.rolled(
    small.entered(small.toggleDoor(last(steps).state).state, "chest-region")
      .state,
    total(12),
  );
  assert.deepEqual(played.state.goals, [
    "door-reached",
    "door-opened",
    "chest-found",
    "chest-opened",
  ]);
  const { state, outcome } = small.reset();
  assert.deepEqual(state, small.start());
  assert.deepEqual(state, before);
  assert.equal(outcome.announcement, "reset");
});

test("the referee is pure: an action never changes the state it was given", () => {
  const near = last(
    walk(small, small.start(), ["north", "north", "north"]),
  ).state;
  const copy = structuredClone(near);
  small.toggleDoor(near);
  small.step(near, "north");
  small.entered(near, "chest-region");
  small.judgeMove(near, { x: 0, y: -150 }, { x: 0, y: 150 });
  assert.deepEqual(near, copy);
});
