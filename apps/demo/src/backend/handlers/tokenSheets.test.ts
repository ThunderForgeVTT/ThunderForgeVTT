/**
 * A token's size, sight and scores come from its sheet as the server reads
 * them (`token_grid.rs`, `token_vision.rs`, `token_attributes.rs`).
 */
import { beforeEach, describe, expect, it } from "vitest";
import type { DemoState, Row } from "../state";
import { data, freshWorld, refusal } from "../testing/world";

let state: DemoState;
let sceneId: string;

beforeEach(async () => {
  state = await freshWorld();
  sceneId = state.world.activeSceneId as string;
});

function tokenNamed(castKey: string): Row {
  const actor = state.actors.find((a) => a.castKey === castKey) as Row;
  return state.tokens.find(
    (t) => t.sceneId === sceneId && t.actorId === actor.id,
  ) as Row;
}

describe("tokenGrid", () => {
  it("gives the Large dire wolf two squares and omits the one-square rest", async () => {
    const { tokenGrid } = await data<{ tokenGrid: Row[] }>(
      "query ($s: UUID!) { tokenGrid(sceneId: $s) { tokenId footprint } }",
      { s: sceneId },
    );
    expect(tokenGrid).toEqual([
      { tokenId: tokenNamed("wolf").tokenId, footprint: 2 },
    ]);
  });

  it("refuses a scene that is not there", async () => {
    expect(
      await refusal(
        "query ($s: UUID!) { tokenGrid(sceneId: $s) { tokenId } }",
        { s: "d0000000-0000-4000-9999-000000000001" },
      ),
    ).toBe("Scene not found");
  });
});

describe("tokenVision", () => {
  it("gives Elowen 60 ft of darkvision, twelve squares of the scene's grid", async () => {
    const { tokenVision } = await data<{ tokenVision: Row[] }>(
      "query ($s: UUID!) { tokenVision(sceneId: $s) { tokenId darkvision carriedBright carriedDim } }",
      { s: sceneId },
    );
    const scene = state.scenes.find((s) => s.sceneId === sceneId) as Row;
    const elowen = tokenVision.find(
      (v) => v.tokenId === tokenNamed("wizard").tokenId,
    );
    expect(elowen).toEqual({
      tokenId: tokenNamed("wizard").tokenId,
      darkvision: 12 * (scene.gridSize as number),
      carriedBright: 0,
      carriedDim: 0,
    });
    const fighter = tokenNamed("fighter").tokenId;
    expect(tokenVision.some((v) => v.tokenId === fighter)).toBe(false);
  });
});

describe("tokenAttributes", () => {
  it("reads the six scores and the walking speed from the sheet", async () => {
    const { tokenAttributes } = await data<{ tokenAttributes: Row[] }>(
      `query ($s: UUID!) { tokenAttributes(sceneId: $s) {
        tokenId attributes { id abbreviation value } speeds { id value } } }`,
      { s: sceneId },
    );
    const fighter = tokenAttributes.find(
      (a) => a.tokenId === tokenNamed("fighter").tokenId,
    ) as Row;
    expect((fighter.attributes as Row[]).map((a) => a.value)).toEqual([
      16, 13, 14, 10, 12, 8,
    ]);
    expect((fighter.attributes as Row[])[0]).toMatchObject({
      id: "strength",
      abbreviation: "STR",
    });
    expect(fighter.speeds).toEqual([{ id: "walk", value: 30 }]);
  });
});
