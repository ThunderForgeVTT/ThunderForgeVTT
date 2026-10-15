import assert from "node:assert/strict";
import { test } from "node:test";
import {
  HERO_ID,
  chestCommand,
  doorCommand,
  rollbackCommand,
  roomSetup,
  roomTeardown,
  type EngineCommand,
} from "./engineScene.ts";
import { provingGround, smallRoom, walkThrough } from "./fixtures.ts";

const script = provingGround();
const hero = { name: "Sir Pip", art: "/room/sir-pip.webp" };
const setup = roomSetup(script, hero, { reducedMotion: false });

/** Each command's type, with runs of the same type folded into one. */
function folded(commands: EngineCommand[]): string[] {
  return commands
    .map((c) => c.type)
    .filter((type, i, all) => type !== all[i - 1]);
}

const indexOf = (
  commands: EngineCommand[],
  match: (c: EngineCommand) => boolean,
): number => {
  const i = commands.findIndex(match);
  assert.notEqual(i, -1);
  return i;
};

test("the setup runs the plan's fourteen steps, in order", () => {
  assert.deepEqual(folded(setup), [
    "set_reduced_motion",
    "set_scene_grid",
    "set_scene_background",
    "upsert_wall",
    "upsert_light",
    "set_ambient_light",
    "upsert_token",
    "upsert_interactive",
    "upsert_token",
    "set_token_vision",
    "set_carried_light",
    "set_controlled_token",
    "set_scene_playing",
    "fit_camera_to",
  ]);
  assert.equal(setup.filter((c) => c.type === "upsert_wall").length, 33);
  assert.equal(setup.filter((c) => c.type === "upsert_light").length, 12);
});

test("the hero's token comes before its vision", () => {
  const token = indexOf(
    setup,
    (c) => c.type === "upsert_token" && c.token.id === HERO_ID,
  );
  const vision = indexOf(setup, (c) => c.type === "set_token_vision");
  assert.ok(token < vision);
});

test("the scene starts playing after the region is set", () => {
  const region = indexOf(setup, (c) => c.type === "upsert_interactive");
  const playing = indexOf(setup, (c) => c.type === "set_scene_playing");
  assert.ok(region < playing);
});

test("the setup's values", () => {
  assert.deepEqual(setup[0], { type: "set_reduced_motion", reduced: false });
  assert.deepEqual(roomSetup(script, hero, { reducedMotion: true })[0], {
    type: "set_reduced_motion",
    reduced: true,
  });
  assert.deepEqual(setup[1], {
    type: "set_scene_grid",
    gridType: "square",
    size: 117,
    mapWidth: 4095,
    mapHeight: 2340,
    visible: true,
  });
  assert.deepEqual(setup[2], {
    type: "set_scene_background",
    backgroundImagePath: "/maps/demo.webp",
    width: 4095,
    height: 2340,
  });
  const doors = setup.filter(
    (c) => c.type === "upsert_wall" && c.wall.doorState !== "none",
  );
  assert.deepEqual(
    doors.map((c) => c.type === "upsert_wall" && [c.wall.id, c.wall.doorState]),
    [
      ["door-hall", "closed"],
      ["door-west", "closed"],
    ],
  );
  assert.ok(
    setup.some((c) => c.type === "set_ambient_light" && c.level === "dark"),
  );
  const chest = setup.find(
    (c) => c.type === "upsert_token" && c.token.id === "chest",
  );
  assert.deepEqual(chest, {
    type: "upsert_token",
    token: {
      id: "chest",
      x: -117,
      y: -409.5,
      z: 1,
      label: "Chest",
      tokenType: "object",
      photoUrl: "/room/chest.webp",
    },
  });
  const region = setup.find((c) => c.type === "upsert_interactive");
  assert.deepEqual(region, {
    type: "upsert_interactive",
    interactive: {
      id: "chest-region",
      subjectKind: "region",
      geometry: { shape: "rect", x: -292.5, y: -585, width: 351, height: 351 },
      trigger: "enter",
      fireMode: "once",
    },
  });
  assert.ok(
    setup.some(
      (c) =>
        c.type === "set_carried_light" &&
        c.tokenId === HERO_ID &&
        c.bright === 468 &&
        c.dim === 936,
    ),
  );
  const token = setup.find(
    (c) => c.type === "upsert_token" && c.token.id === HERO_ID,
  );
  assert.deepEqual(token, {
    type: "upsert_token",
    token: {
      id: HERO_ID,
      x: -351,
      y: 175.5,
      z: 2,
      label: "Sir Pip",
      tokenType: "character",
      photoUrl: "/room/sir-pip.webp",
    },
  });
  assert.ok(
    setup.some(
      (c) => c.type === "set_controlled_token" && c.tokenId === HERO_ID,
    ),
  );
  assert.ok(
    setup.some((c) => c.type === "set_scene_playing" && c.playing === true),
  );
  assert.deepEqual(setup[setup.length - 1], {
    type: "fit_camera_to",
    centerX: -292.5,
    centerY: 0,
    width: 1638,
    height: 1638,
    immediate: false,
  });
  assert.deepEqual(roomSetup(script, hero, { reducedMotion: true }).at(-1), {
    type: "fit_camera_to",
    centerX: -292.5,
    centerY: 0,
    width: 1638,
    height: 1638,
    immediate: true,
  });
});

test("the teardown removes every id the setup added", () => {
  const down = roomTeardown(script);
  const removed = new Set(
    down.flatMap((c) => {
      switch (c.type) {
        case "remove_token":
          return [`token:${c.tokenId}`];
        case "remove_wall":
          return [`wall:${c.wallId}`];
        case "remove_light":
          return [`light:${c.lightId}`];
        case "remove_interactive":
          return [`interactive:${c.interactiveId}`];
        case "set_carried_light":
          return c.bright === 0 && c.dim === 0 ? [`carried:${c.tokenId}`] : [];
        default:
          return [];
      }
    }),
  );
  const added = setup.flatMap((c) => {
    switch (c.type) {
      case "upsert_token":
        return [`token:${c.token.id}`];
      case "upsert_wall":
        return [`wall:${c.wall.id}`];
      case "upsert_light":
        return [`light:${c.light.id}`];
      case "upsert_interactive":
        return [`interactive:${c.interactive.id}`];
      case "set_carried_light":
        return [`carried:${c.tokenId}`];
      default:
        return [];
    }
  });
  assert.equal(added.length, 2 + 33 + 12 + 1 + 1);
  for (const id of added) assert.ok(removed.has(id), `${id} is left behind`);
});

test("the teardown stops the scene, frees the hero and clears the background", () => {
  const down = roomTeardown(script);
  assert.ok(
    down.some((c) => c.type === "set_scene_playing" && c.playing === false),
  );
  assert.ok(
    down.some((c) => c.type === "set_controlled_token" && c.tokenId === null),
  );
  assert.ok(
    down.some(
      (c) =>
        c.type === "set_scene_background" &&
        c.backgroundImagePath === null &&
        c.width === 0 &&
        c.height === 0,
    ),
  );
});

test("doorCommand sends the door with its new state", () => {
  const door = script.map.walls.find((w) => w.id === "door-hall")!;
  assert.deepEqual(doorCommand(door, "open"), {
    type: "upsert_wall",
    wall: { ...door, doorState: "open" },
  });
  assert.equal(door.doorState, "closed", "the map's wall is not changed");
});

test("chestCommand sends the chest shut or open, in its place", () => {
  const script = smallRoom();
  const [shut] = chestCommand(script, false);
  const [open] = chestCommand(script, true);
  assert.ok(shut && open);
  assert.equal(shut.type, "upsert_token");
  assert.equal(open.type, "upsert_token");
  if (shut.type !== "upsert_token" || open.type !== "upsert_token") return;
  assert.equal(shut.token.photoUrl, "/room/chest.webp");
  assert.equal(open.token.photoUrl, "/room/chest-open.webp");
  assert.deepEqual({ ...open.token, photoUrl: shut.token.photoUrl }, shut.token);
  assert.deepEqual(
    roomSetup(script, hero, { reducedMotion: true }).find(
      (c) => c.type === "upsert_token" && c.token.id === "chest",
    ),
    shut,
  );
});

test("a room without a chest sends no chest and no region, up or down", () => {
  const room = walkThrough();
  assert.deepEqual(chestCommand(room, true), []);
  const sent = [
    ...roomSetup(room, hero, { reducedMotion: true }),
    ...roomTeardown(room),
  ];
  assert.ok(!JSON.stringify(sent).includes("chest"));
});

test("rollbackCommand sends the hero back to a square", () => {
  assert.deepEqual(rollbackCommand(hero, { x: -3, y: -1.5 }, 117), {
    type: "upsert_token",
    token: {
      id: HERO_ID,
      x: -351,
      y: -175.5,
      z: 2,
      label: "Sir Pip",
      tokenType: "character",
      photoUrl: "/room/sir-pip.webp",
    },
  });
});
