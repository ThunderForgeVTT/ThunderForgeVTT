/**
 * Players draw their own shapes in the demo as they do on a server (spec 082
 * US6, research R10): whoever the tab is viewing as is the shape's creator, a
 * player edits only their own and cannot hide it, and the Game Master clears
 * a scene's shapes, every one or only some players'.
 */
import { beforeEach, describe, expect, it } from "vitest";

import { DEMO_PLAYER, DEMO_USER } from "../../seed/world";
import { demoState } from "../state";
import {
  ask,
  data,
  freshWorld,
  heard,
  refusal,
  releaseEvents,
  viewAs,
} from "../testing/world";

const CREATE = `mutation ($input: GraphQLCreateShapeInput!) {
  createShape(input: $input) { shapeId createdBy updatedBy visibleToPlayers }
}`;
const UPDATE = `mutation ($id: UUID!, $input: GraphQLUpdateShapeInput!) {
  updateShape(shapeId: $id, input: $input) { shapeId updatedBy visibleToPlayers }
}`;
const DELETE = `mutation ($id: UUID!) { deleteShape(shapeId: $id) }`;
const CLEAR = `mutation ($scene: UUID!, $createdBy: [UUID!]) {
  clearShapes(sceneId: $scene, createdBy: $createdBy)
}`;
const CREATORS = `query ($scene: UUID!) {
  shapeCreators(sceneId: $scene) { userId displayName isMember shapeCount }
}`;
const SHAPES = `query ($scene: UUID!) { shapes(sceneId: $scene) { shapeId createdBy } }`;
const TOOLS = `query ($world: UUID!) { authoringTools(worldId: $world) }`;
const GRANTS = `query ($world: UUID!) {
  authoringToolGrants(worldId: $world) { worldMemberId userId tools }
}`;

let sceneId: string;

async function draw(visibleToPlayers = true): Promise<string> {
  const answer = await data<{ createShape: { shapeId: string } }>(CREATE, {
    input: {
      sceneId,
      kind: "RECT",
      geometry: { x: 1, y: 2, w: 3, h: 4 },
      visibleToPlayers,
    },
  });
  return answer.createShape.shapeId;
}

const idsOnScene = () =>
  demoState()
    .shapes.filter((shape) => shape.sceneId === sceneId)
    .map((shape) => shape.shapeId as string)
    .sort();

beforeEach(async () => {
  const state = await freshWorld();
  sceneId = state.world.activeSceneId as string;
  state.shapes = state.shapes.filter((shape) => shape.sceneId !== sceneId);
  releaseEvents();
});

describe("drawing", () => {
  it("stamps the shape with whoever the tab is viewing as", async () => {
    const gm = await data(CREATE, {
      input: { sceneId, kind: "RECT", geometry: {}, visibleToPlayers: false },
    });
    expect(gm.createShape).toMatchObject({
      createdBy: DEMO_USER.id,
      updatedBy: DEMO_USER.id,
      visibleToPlayers: false,
    });

    viewAs("player");
    const player = await data(CREATE, {
      input: { sceneId, kind: "RECT", geometry: {}, visibleToPlayers: false },
    });
    expect(player.createShape).toMatchObject({
      createdBy: DEMO_PLAYER.id,
      updatedBy: DEMO_PLAYER.id,
      visibleToPlayers: true,
    });
  });

  it("lets a player edit their own shape but not hide it", async () => {
    viewAs("player");
    const mine = await draw();
    const edited = await data(UPDATE, {
      id: mine,
      input: { geometry: { x: 9 }, visibleToPlayers: false },
    });
    expect(edited.updateShape).toMatchObject({
      updatedBy: DEMO_PLAYER.id,
      visibleToPlayers: true,
    });
  });

  it("refuses a player's edit or delete of someone else's shape", async () => {
    const gms = await draw();
    const before = JSON.stringify(demoState().shapes);
    viewAs("player");
    releaseEvents();
    const events = heard();

    expect(
      await refusal(UPDATE, { id: gms, input: { geometry: { x: 9 } } }),
    ).toBe("Failed to update shape (not found or not owned by you)");
    expect((await data(DELETE, { id: gms })).deleteShape).toBe(false);
    releaseEvents();
    expect(JSON.stringify(demoState().shapes)).toBe(before);
    expect(events).toEqual([]);
  });

  it("lets the GM edit and delete a player's shape", async () => {
    viewAs("player");
    const theirs = await draw();
    viewAs("gm");
    const edited = await data(UPDATE, {
      id: theirs,
      input: { visibleToPlayers: false },
    });
    expect(edited.updateShape).toMatchObject({
      updatedBy: DEMO_USER.id,
      visibleToPlayers: false,
    });
    expect((await data(DELETE, { id: theirs })).deleteShape).toBe(true);
  });

  it("reads a stored shape with no creator as the Game Master's", async () => {
    demoState().shapes.push({
      shapeId: "old",
      sceneId,
      visibleToPlayers: true,
    });
    const answer = await data<{ shapes: Array<{ createdBy: string }> }>(
      SHAPES,
      { scene: sceneId },
    );
    expect(answer.shapes).toEqual([
      { shapeId: "old", createdBy: DEMO_USER.id },
    ]);

    viewAs("player");
    expect((await data(DELETE, { id: "old" })).deleteShape).toBe(false);
  });
});

describe("clearShapes", () => {
  it("clears every shape on the scene, one deleted event each", async () => {
    await draw(false);
    viewAs("player");
    await draw();
    viewAs("gm");
    const elsewhere = { shapeId: "elsewhere", sceneId: "another-scene" };
    demoState().shapes.push(elsewhere);
    releaseEvents();
    const events = heard();

    const answer = await data(CLEAR, { scene: sceneId });
    releaseEvents();
    expect(answer.clearShapes).toBe(2);
    expect(idsOnScene()).toEqual([]);
    expect(demoState().shapes).toContainEqual(elsewhere);
    expect(
      events.map((event) => (event.tokenEvent as { action: string }).action),
    ).toEqual(["deleted", "deleted"]);
  });

  it("clears only the chosen players' shapes, and [] clears nothing", async () => {
    const gms = await draw();
    viewAs("player");
    await draw();
    await draw();
    viewAs("gm");

    expect(
      (await data(CLEAR, { scene: sceneId, createdBy: [] })).clearShapes,
    ).toBe(0);
    expect(idsOnScene()).toHaveLength(3);

    const answer = await data(CLEAR, {
      scene: sceneId,
      createdBy: [DEMO_PLAYER.id],
    });
    expect(answer.clearShapes).toBe(2);
    expect(idsOnScene()).toEqual([gms]);
  });

  it("refuses a player, and nothing is deleted", async () => {
    await draw();
    viewAs("player");
    await draw();
    for (const createdBy of [undefined, [DEMO_PLAYER.id]]) {
      expect(await refusal(CLEAR, { scene: sceneId, createdBy })).toBe(
        "Failed to clear shapes (scene not found)",
      );
    }
    expect(idsOnScene()).toHaveLength(2);
  });
});

describe("shapeCreators", () => {
  it("lists the player who drew, not the Game Master", async () => {
    await draw();
    viewAs("player");
    await draw();
    await draw();
    viewAs("gm");

    const answer = await data(CREATORS, { scene: sceneId });
    expect(answer.shapeCreators).toEqual([
      {
        userId: DEMO_PLAYER.id,
        displayName: DEMO_PLAYER.username,
        isMember: true,
        shapeCount: 2,
      },
    ]);
  });

  it("refuses a player", async () => {
    viewAs("player");
    expect(await refusal(CREATORS, { scene: sceneId })).toBe("Scene not found");
  });
});

describe("authoring tools", () => {
  it("gives the GM every tool and the player select and shapes", async () => {
    const world = demoState().world.id;
    expect((await data(TOOLS, { world })).authoringTools).toEqual([
      "select",
      "walls",
      "lights",
      "shapes",
      "tokens",
      "interactions",
    ]);
    const grants = await data(GRANTS, { world });
    expect(grants.authoringToolGrants).toEqual([
      expect.objectContaining({
        userId: DEMO_PLAYER.id,
        tools: ["select", "shapes"],
      }),
    ]);

    viewAs("player");
    expect((await data(TOOLS, { world })).authoringTools).toEqual([
      "select",
      "shapes",
    ]);
    expect((await ask(GRANTS, { world })).errors?.length).toBeGreaterThan(0);
  });
});
