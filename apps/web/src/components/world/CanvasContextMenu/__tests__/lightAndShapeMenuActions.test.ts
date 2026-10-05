import { describe, expect, it } from "vitest";
import type { WorldLight, WorldShape } from "@/engine/world/types";
import {
  actionLabel,
  lightMenuActions,
  shapeMenuActions,
  shapeName,
} from "../canvasMenuActions";
import { menuSubjectOf } from "../useCanvasContextMenu";

const gm = { isGameMaster: true, userId: "gm" };
const player = { isGameMaster: false, userId: "player" };

function light(overrides: Partial<WorldLight> = {}): WorldLight {
  return {
    id: "light-1",
    sceneId: "scene-1",
    x: 0,
    y: 0,
    radius: 400,
    brightRadius: 200,
    intensity: 1,
    color: null,
    attachedTokenId: null,
    castsShadows: true,
    ...overrides,
  } as WorldLight;
}

function shape(overrides: Partial<WorldShape> = {}): WorldShape {
  return {
    id: "shape-1",
    sceneId: "scene-1",
    kind: "rect",
    geometry: { x: 0, y: 0, w: 10, h: 10 },
    text: null,
    style: null,
    visibleToPlayers: false,
    ...overrides,
  } as WorldShape;
}

const labels = (actions: Parameters<typeof actionLabel>[0][]) =>
  actions.map((action) => actionLabel(action, ""));

describe("lightMenuActions", () => {
  it("offers a Game Master a burning light put out, shone through walls, or removed", () => {
    const actions = lightMenuActions({ viewer: gm, light: light() });
    expect(actions).toEqual([
      { kind: "light-power", on: false },
      { kind: "light-shadows", casts: false },
      { kind: "light-remove" },
    ]);
    expect(labels(actions)).toEqual([
      "Put it out",
      "Shine through walls",
      "Remove this light",
    ]);
  });

  it("offers a light that is out, and shines through walls, the other way round", () => {
    const actions = lightMenuActions({
      viewer: gm,
      light: light({ intensity: 0, castsShadows: false }),
    });
    expect(labels(actions)).toEqual([
      "Light it",
      "Stop at walls",
      "Remove this light",
    ]);
  });

  it("offers a player nothing", () => {
    expect(lightMenuActions({ viewer: player, light: light() })).toEqual([]);
  });

  it("offers nothing on a light a game system hangs from a token", () => {
    expect(
      lightMenuActions({ viewer: gm, light: light({ id: "carried:token-1" }) }),
    ).toEqual([]);
  });
});

describe("shapeMenuActions", () => {
  it("offers a Game Master a hidden drawing shown, or removed", () => {
    const actions = shapeMenuActions({ viewer: gm, shape: shape() });
    expect(actions).toEqual([
      { kind: "shape-visibility", visible: true },
      { kind: "shape-remove" },
    ]);
    expect(labels(actions)).toEqual([
      "Show to players",
      "Remove from the board",
    ]);
  });

  it("offers a shown drawing hidden", () => {
    const actions = shapeMenuActions({
      viewer: gm,
      shape: shape({ visibleToPlayers: true }),
    });
    expect(labels(actions)[0]).toBe("Hide from players");
  });

  it("offers a player nothing", () => {
    expect(shapeMenuActions({ viewer: player, shape: shape() })).toEqual([]);
  });

  it("names a drawing for what it is", () => {
    expect(
      (["rect", "ellipse", "line", "text", "stroke"] as const).map((kind) =>
        shapeName(shape({ kind })),
      ),
    ).toEqual(["Rectangle", "Ellipse", "Line", "Text", "Drawing"]);
  });
});

describe("menuSubjectOf", () => {
  const all = {
    tokenIds: ["token-1"],
    lightId: "light-1",
    wallId: "wall-1",
    shapeId: "shape-1",
  };
  const none = { tokenId: null, lightId: null, wallId: null, shapeId: null };

  it("is about the token, whatever else is under the pointer", () => {
    expect(menuSubjectOf(all)).toEqual({ ...none, tokenId: "token-1" });
  });

  it("is about a light before a wall, and a wall before a drawing", () => {
    expect(menuSubjectOf({ ...all, tokenIds: [] })).toEqual({
      ...none,
      lightId: "light-1",
    });
    expect(menuSubjectOf({ ...all, tokenIds: [], lightId: null })).toEqual({
      ...none,
      wallId: "wall-1",
    });
    expect(
      menuSubjectOf({ tokenIds: [], wallId: null, shapeId: "shape-1" }),
    ).toEqual({ ...none, shapeId: "shape-1" });
  });

  it("is about bare board when nothing is there", () => {
    expect(menuSubjectOf({ tokenIds: [] })).toEqual(none);
  });
});
