/**
 * A scene's light and its exploration memory answer as the server's do
 * (`mutations_scenes.rs`, `exploration.rs`, `scene_levels.rs`): the same
 * refusals, the same numbers, the same events.
 */
import { beforeEach, describe, expect, it } from "vitest";
import { DEMO_PLAYER } from "../../seed/world";
import type { DemoState, Row } from "../state";
import {
  data,
  freshWorld,
  heard,
  refusal,
  releaseEvents,
} from "../testing/world";

let state: DemoState;
let sceneId: string;

const AMBIENT = `mutation ($s: UUID!, $a: String!) {
  updateSceneAmbientLight(sceneId: $s, ambientLight: $a) { sceneId ambientLight }
}`;
const EXPLORATION = `query ($s: UUID!) { sceneExploration(sceneId: $s) { enabled epoch mine } }`;
const SET = `mutation ($s: UUID!, $e: Boolean!) { setSceneExploration(sceneId: $s, enabled: $e) }`;
const RESET = `mutation ($s: UUID!, $u: UUID) { resetSceneExploration(sceneId: $s, forUser: $u) }`;
const LEVEL = `mutation ($l: UUID!, $i: GraphQLUpdateSceneLevelInput!) {
  updateSceneLevel(levelId: $l, input: $i) { levelId ambientLight name isEntry }
}`;

beforeEach(async () => {
  state = await freshWorld();
  sceneId = state.world.activeSceneId as string;
  releaseEvents();
});

function entryOf(id: string): Row {
  return state.levels.find((l) => l.sceneId === id && l.isEntry) as Row;
}

describe("updateSceneAmbientLight", () => {
  it("lights the scene and its entry level, and says so with code 25", async () => {
    const events = heard();
    const answer = await data<{ updateSceneAmbientLight: Row }>(AMBIENT, {
      s: sceneId,
      a: " Dark ",
    });
    expect(answer.updateSceneAmbientLight.ambientLight).toBe("dark");
    expect(entryOf(sceneId).ambientLight).toBe("dark");
    releaseEvents();
    expect(events).toHaveLength(1);
    expect(events[0].eventCode).toBe(25);
    expect(events[0].tokenEvent).toEqual({
      action: "changed",
      sceneId,
      ambientLight: "dark",
    });
  });

  it("refuses a light that is not one, and a player", async () => {
    expect(await refusal(AMBIENT, { s: sceneId, a: "dusk" })).toBe(
      "A scene's light is bright, dim or dark",
    );
    state.viewer = "player";
    expect(await refusal(AMBIENT, { s: sceneId, a: "dim" })).toBe(
      "Only the DM (Owner or GM) may change a scene's light",
    );
  });
});

describe("updateSceneLevel", () => {
  it("lights the entry level and so the scene, announced as code 33", async () => {
    const events = heard();
    const entry = entryOf(sceneId);
    await data(LEVEL, { l: entry.levelId, i: { ambientLight: "dim" } });
    const scene = state.scenes.find((s) => s.sceneId === sceneId) as Row;
    expect(scene.ambientLight).toBe("dim");
    releaseEvents();
    expect(events.map((e) => e.eventCode)).toEqual([33]);
    expect(events[0].tokenEvent).toEqual({
      action: "updated",
      scene_id: sceneId,
      level_id: entry.levelId,
    });
  });

  it("refuses as the server refuses", async () => {
    const entry = entryOf(sceneId);
    expect(await refusal(LEVEL, { l: entry.levelId, i: { name: "  " } })).toBe(
      "A level needs a name",
    );
    expect(
      await refusal(LEVEL, { l: entry.levelId, i: { ambientLight: "noon" } }),
    ).toBe("A level's light is bright, dim or dark");
    expect(await refusal(LEVEL, { l: entry.levelId, i: { width: 0 } })).toBe(
      "A level's width and height must be more than zero",
    );
    expect(
      await refusal(LEVEL, {
        l: entry.levelId,
        i: { backgroundAssetId: "d0000000-0000-4000-9999-000000000001" },
      }),
    ).toBe("That image is not in this world");
    state.viewer = "player";
    expect(await refusal(LEVEL, { l: entry.levelId, i: { name: "x" } })).toBe(
      "Only the Game Master may change a scene's levels",
    );
  });
});

describe("exploration", () => {
  it("is off until the Game Master turns it on", async () => {
    expect(await data(EXPLORATION, { s: sceneId })).toEqual({
      sceneExploration: { enabled: false, epoch: 0, mine: 0 },
    });
    expect(await data(SET, { s: sceneId, e: true })).toEqual({
      setSceneExploration: true,
    });
    state.viewer = "player";
    expect(
      ((await data(EXPLORATION, { s: sceneId })) as { sceneExploration: Row })
        .sceneExploration.enabled,
    ).toBe(true);
    expect(await refusal(SET, { s: sceneId, e: false })).toBe(
      "Only the DM (Owner or GM) may change or reset exploration",
    );
  });

  it("resets one member past the scene, and everyone past them all", async () => {
    const events = heard();
    expect(await data(RESET, { s: sceneId, u: DEMO_PLAYER.id })).toEqual({
      resetSceneExploration: 1,
    });
    expect(await data(RESET, { s: sceneId, u: DEMO_PLAYER.id })).toEqual({
      resetSceneExploration: 2,
    });
    state.viewer = "player";
    expect(await data(EXPLORATION, { s: sceneId })).toEqual({
      sceneExploration: { enabled: false, epoch: 0, mine: 2 },
    });
    state.viewer = "gm";
    expect(await data(RESET, { s: sceneId })).toEqual({
      resetSceneExploration: 3,
    });
    state.viewer = "player";
    expect(await data(EXPLORATION, { s: sceneId })).toEqual({
      sceneExploration: { enabled: false, epoch: 3, mine: 3 },
    });
    releaseEvents();
    expect(events.map((e) => e.eventCode)).toEqual([27, 27, 27]);
    expect(events[2].tokenEvent).toEqual({
      action: "reset",
      sceneId,
      forUser: null,
      epoch: 3,
    });
  });
});
