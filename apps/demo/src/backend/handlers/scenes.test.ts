/**
 * Scenes and their levels answer as the server's do (`mutations_scenes.rs`,
 * `mutations_party.rs`, `scene_levels.rs`): the same defaults, refusals,
 * visibility and events.
 */
import { beforeEach, describe, expect, it } from "vitest";
import type { DemoState, Row } from "../state";
import {
  data,
  freshWorld,
  heard,
  refusal,
  releaseEvents,
} from "../testing/world";
import { freeSpot } from "./levels";

let state: DemoState;
let sceneId: string;

const CREATE_SCENE = `mutation ($i: GraphQLCreateSceneInput!) {
  createScene(input: $i) { sceneId name hidden gridSize width height ambientLight }
}`;
const SCENES = `query ($w: UUID!) { scenes(worldId: $w) { sceneId } }`;
const HIDE = `mutation ($s: UUID!, $h: Boolean!) { updateSceneHidden(sceneId: $s, hidden: $h) { hidden } }`;
const UPDATE = `mutation ($s: UUID!, $i: GraphQLUpdateSceneInput!) {
  updateScene(sceneId: $s, input: $i) { name width summaryRenderedHtml }
}`;
const BRING = `mutation ($i: BringPartyToSceneInput!) {
  bringPartyToScene(input: $i) { sceneId arrivedActorIds alreadyPresentActorIds }
}`;
const LEVELS = `query ($s: UUID!) { sceneLevels(sceneId: $s) { levelId name sortOrder isEntry tokenCount } }`;
const CREATE_LEVEL = `mutation ($i: GraphQLCreateSceneLevelInput!) {
  createSceneLevel(input: $i) { levelId name sortOrder isEntry width ambientLight }
}`;
const REORDER = `mutation ($s: UUID!, $l: [UUID!]!) { reorderSceneLevels(sceneId: $s, levelIds: $l) { levelId sortOrder } }`;
const DELETE_LEVEL = `mutation ($l: UUID!) { deleteSceneLevel(levelId: $l) }`;
const MOVE = `mutation ($t: [UUID!]!, $l: UUID!, $x: Float, $y: Float) {
  moveTokensToLevel(tokenIds: $t, levelId: $l, x: $x, y: $y) { tokenId levelId x y }
}`;

beforeEach(async () => {
  state = await freshWorld();
  sceneId = state.world.activeSceneId as string;
  releaseEvents();
});

async function newScene(name = "The Sunken Vault"): Promise<Row> {
  return (
    await data<{ createScene: Row }>(CREATE_SCENE, {
      i: { worldId: state.world.id, name },
    })
  ).createScene;
}

describe("createScene", () => {
  it("starts hidden, with the server's defaults and a Ground level", async () => {
    const scene = await newScene();
    expect(scene).toMatchObject({
      name: "The Sunken Vault",
      hidden: true,
      gridSize: 5,
      width: 100,
      height: 100,
      ambientLight: "bright",
    });
    const { sceneLevels } = await data<{ sceneLevels: Row[] }>(LEVELS, {
      s: scene.sceneId,
    });
    expect(sceneLevels).toEqual([
      expect.objectContaining({ name: "Ground", sortOrder: 0, isEntry: true }),
    ]);
  });

  it("is hidden from a player until the Game Master shows it", async () => {
    const scene = await newScene();
    state.viewer = "player";
    const listed = async () =>
      (await data<{ scenes: Row[] }>(SCENES, { w: state.world.id })).scenes.map(
        (s) => s.sceneId,
      );
    expect(await listed()).not.toContain(scene.sceneId);
    expect(await refusal(HIDE, { s: scene.sceneId, h: false })).toBe(
      "Only the DM (Owner or GM) may change a scene's visibility",
    );
    expect(
      await refusal(CREATE_SCENE, {
        i: { worldId: state.world.id, name: "x" },
      }),
    ).toBe("Forbidden");
    state.viewer = "gm";
    await data(HIDE, { s: scene.sceneId, h: false });
    state.viewer = "player";
    expect(await listed()).toContain(scene.sceneId);
  });
});

describe("updateScene", () => {
  it("renames, resizes the entry level with it, and escapes the summary", async () => {
    const scene = await newScene();
    const { updateScene } = await data<{ updateScene: Row }>(UPDATE, {
      s: scene.sceneId,
      i: { name: "Vault", width: 40, summaryMarkdown: "<b>Wet</b>" },
    });
    expect(updateScene).toEqual({
      name: "Vault",
      width: 40,
      summaryRenderedHtml: "<p>&#60;b&#62;Wet&#60;/b&#62;</p>\n",
    });
    const entry = state.levels.find(
      (l) => l.sceneId === scene.sceneId && l.isEntry,
    ) as Row;
    expect(entry.width).toBe(40);
    state.viewer = "player";
    expect(await refusal(UPDATE, { s: scene.sceneId, i: { name: "x" } })).toBe(
      "Failed to update scene",
    );
  });
});

describe("bringPartyToScene", () => {
  it("lines the party up once, and says who was already there", async () => {
    const scene = await newScene();
    const events = heard();
    const heroes = state.actors.filter((a) => !a.isNpc).map((a) => a.id);
    const first = (
      await data<{ bringPartyToScene: Row }>(BRING, {
        i: { sceneId: scene.sceneId },
      })
    ).bringPartyToScene;
    expect(first.arrivedActorIds).toEqual(expect.arrayContaining(heroes));
    expect(first.alreadyPresentActorIds).toEqual([]);
    const placed = state.tokens.filter((t) => t.sceneId === scene.sceneId);
    expect(placed.map((t) => [t.x, t.y]).sort()).toEqual(
      heroes.map((_, i) => [5 * (i + 1), 5]).sort(),
    );
    const again = (
      await data<{ bringPartyToScene: Row }>(BRING, {
        i: { sceneId: scene.sceneId },
      })
    ).bringPartyToScene;
    expect(again.arrivedActorIds).toEqual([]);
    expect(again.alreadyPresentActorIds).toHaveLength(heroes.length);
    releaseEvents();
    expect(events.map((e) => e.eventCode)).toEqual([14]);
  });

  it("refuses a monster and a player", async () => {
    const wolf = state.actors.find((a) => a.castKey === "wolf") as Row;
    expect(await refusal(BRING, { i: { sceneId, actorIds: [wolf.id] } })).toBe(
      `Character ${wolf.id} is not a player character in this world`,
    );
    state.viewer = "player";
    expect(await refusal(BRING, { i: { sceneId } })).toBe(
      "Only the DM (Owner or GM) may bring the party",
    );
  });
});

describe("levels", () => {
  it("adds a level on top with the scene's board, announced as code 33", async () => {
    const events = heard();
    const scene = state.scenes.find((s) => s.sceneId === sceneId) as Row;
    const { createSceneLevel } = await data<{ createSceneLevel: Row }>(
      CREATE_LEVEL,
      { i: { sceneId, name: " Cellar " } },
    );
    expect(createSceneLevel).toMatchObject({
      name: "Cellar",
      sortOrder: 1,
      isEntry: false,
      width: scene.width,
      ambientLight: scene.ambientLight,
    });
    releaseEvents();
    expect(events[0].tokenEvent).toEqual({
      action: "created",
      scene_id: sceneId,
      level_id: createSceneLevel.levelId,
    });
  });

  it("keeps at most twelve, and refuses a player", async () => {
    for (let n = 1; n < 12; n += 1) {
      await data(CREATE_LEVEL, { i: { sceneId, name: `Floor ${n}` } });
    }
    expect(await refusal(CREATE_LEVEL, { i: { sceneId, name: "13" } })).toBe(
      "A scene has at most 12 levels",
    );
    state.viewer = "player";
    expect(await refusal(CREATE_LEVEL, { i: { sceneId, name: "x" } })).toBe(
      "Only the Game Master may change a scene's levels",
    );
  });

  it("reorders when every level is named once", async () => {
    const cellar = (
      await data<{ createSceneLevel: Row }>(CREATE_LEVEL, {
        i: { sceneId, name: "Cellar" },
      })
    ).createSceneLevel.levelId as string;
    const ground = state.levels.find((l) => l.sceneId === sceneId && l.isEntry)
      ?.levelId as string;
    expect(await refusal(REORDER, { s: sceneId, l: [cellar] })).toBe(
      "Name every level of the scene once, in the order you want",
    );
    const { reorderSceneLevels } = await data<{ reorderSceneLevels: Row[] }>(
      REORDER,
      { s: sceneId, l: [cellar, ground] },
    );
    expect(reorderSceneLevels).toEqual([
      { levelId: cellar, sortOrder: 0 },
      { levelId: ground, sortOrder: 1 },
    ]);
  });

  it("deletes only an empty level that is not the way in", async () => {
    const ground = state.levels.find((l) => l.sceneId === sceneId && l.isEntry)
      ?.levelId as string;
    expect(await refusal(DELETE_LEVEL, { l: ground })).toBe(
      "A scene keeps at least one level",
    );
    const cellar = (
      await data<{ createSceneLevel: Row }>(CREATE_LEVEL, {
        i: { sceneId, name: "Cellar" },
      })
    ).createSceneLevel.levelId as string;
    expect(await refusal(DELETE_LEVEL, { l: ground })).toBe(
      "This is the scene's entry level. Make another level the entry before deleting it",
    );
    const token = state.tokens.find((t) => t.sceneId === sceneId) as Row;
    await data(MOVE, { t: [token.tokenId], l: cellar });
    expect(await refusal(DELETE_LEVEL, { l: cellar })).toBe(
      "Move the tokens off this level before deleting it",
    );
    await data(MOVE, { t: [token.tokenId], l: ground });
    expect(await data(DELETE_LEVEL, { l: cellar })).toEqual({
      deleteSceneLevel: cellar,
    });
  });

  it("lifts tokens to a level, fanned out around a point", async () => {
    const cellar = (
      await data<{ createSceneLevel: Row }>(CREATE_LEVEL, {
        i: { sceneId, name: "Cellar" },
      })
    ).createSceneLevel.levelId as string;
    const two = state.tokens
      .filter((t) => t.sceneId === sceneId)
      .slice(0, 2)
      .map((t) => t.tokenId as string);
    releaseEvents();
    const events = heard();
    expect(await refusal(MOVE, { t: two, l: cellar, x: 1 })).toBe(
      "Give both x and y, or neither",
    );
    const { moveTokensToLevel } = await data<{ moveTokensToLevel: Row[] }>(
      MOVE,
      { t: two, l: cellar, x: 500, y: 500 },
    );
    const step = (state.scenes.find((s) => s.sceneId === sceneId) as Row)
      .gridSize as number;
    expect(moveTokensToLevel).toEqual([
      { tokenId: two[0], levelId: cellar, x: 500, y: 500 },
      { tokenId: two[1], levelId: cellar, x: 500 - step, y: 500 - step },
    ]);
    releaseEvents();
    expect(events.map((e) => e.eventCode).slice(0, 2)).toEqual([34, 14]);
    const elsewhere = await newScene();
    const ground = state.levels.find((l) => l.sceneId === elsewhere.sceneId)
      ?.levelId as string;
    expect(await refusal(MOVE, { t: two, l: ground })).toBe(
      "Every token must be in the same scene as the level",
    );
  });
});

describe("freeSpot", () => {
  it("takes the point when free, else the first free edge of a ring", () => {
    expect(freeSpot([0, 0], 10, [])).toEqual([0, 0]);
    expect(freeSpot([0, 0], 10, [[0, 0]])).toEqual([-10, -10]);
    expect(
      freeSpot([0, 0], 10, [
        [0, 0],
        [-10, -10],
      ]),
    ).toEqual([0, -10]);
  });
});
