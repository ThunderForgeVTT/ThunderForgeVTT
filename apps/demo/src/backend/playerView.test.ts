/**
 * What a player is answered about a scene's doors and shapes, as the server
 * answers it (`interactives.rs`, `scene.rs`): a door without its authoring,
 * a locked door they may not use, and no shape kept for the Game Master.
 */
import { beforeEach, describe, expect, it } from "vitest";
import type { DemoState, Row } from "./state";
import { freshWorld, must, viewAs } from "./testing/world";

let state: DemoState;
let sceneId: string;

const INTERACTIVES = `query ($s: UUID!) {
  interactives(sceneId: $s) {
    subjectRef effectId effectConfig activation fireMode available canActivate
  }
}`;
const SHAPES = `query ($s: UUID!) { shapes(sceneId: $s) { shapeId } }`;

function door(wallId: string, locked: boolean): Row {
  return { wallId, sceneId, doorState: "CLOSED", locked, secret: false };
}

beforeEach(async () => {
  state = await freshWorld();
  sceneId = state.world.activeSceneId as string;
});

describe("interactives", () => {
  beforeEach(() => {
    state.walls.push(door("open-door", false), door("locked-door", true));
  });

  it("gives the Game Master the authoring view of every door", async () => {
    const { interactives } = await must(INTERACTIVES, { s: sceneId });
    expect(interactives).toHaveLength(2);
    for (const one of interactives) {
      expect(one).toMatchObject({
        effectId: "door.set_state",
        activation: "anyone",
        available: true,
        canActivate: true,
      });
    }
  });

  it("tells a player only that a door is there, and that a lock stops them", async () => {
    viewAs("player");
    const { interactives } = await must(INTERACTIVES, { s: sceneId });
    expect(interactives).toEqual([
      {
        subjectRef: "open-door",
        effectId: null,
        effectConfig: null,
        activation: null,
        fireMode: null,
        available: null,
        canActivate: true,
      },
      {
        subjectRef: "locked-door",
        effectId: null,
        effectConfig: null,
        activation: null,
        fireMode: null,
        available: null,
        canActivate: false,
      },
    ]);
  });
});

describe("shapes", () => {
  it("keeps a shape the Game Master has not shown from a player", async () => {
    state.shapes.push(
      { shapeId: "shown", sceneId, visibleToPlayers: true },
      { shapeId: "kept", sceneId, visibleToPlayers: false },
    );
    const gm = await must(SHAPES, { s: sceneId });
    expect(gm.shapes).toHaveLength(2);
    viewAs("player");
    const player = await must(SHAPES, { s: sceneId });
    expect(player.shapes).toEqual([{ shapeId: "shown" }]);
  });
});
