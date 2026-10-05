/**
 * Spec 074 FR-011: the world the demo ships as.
 *
 * This file says what the world *is* — who the visitor is, what the world is
 * called, which map becomes which scene and what stands on it. The maps'
 * geometry is not here: `thunderforge-demo-maps` writes it to `maps.json`
 * from the same importer the server runs, and `buildSeed` joins the two.
 */
// Which game system and interface the demo world uses is data, not code:
// shared code names no game system (spec 032 FR-029).
import world from "../../world.json";
import { MAP_CREDIT_LINE } from "../credit";
import type { DemoState, Row } from "../backend/state";

/** One map as `thunderforge-demo-maps` lists it. */
export interface MapListing {
  name: string;
  width: number;
  height: number;
  gridSize: number;
  ambientLight: string;
  hasPreview: boolean;
  byteSize: number;
  walls: Array<{
    x1: number;
    y1: number;
    x2: number;
    y2: number;
    blocksVision: boolean;
    blocksMovement: boolean;
    doorState: string;
  }>;
  lights: Array<{
    x: number;
    y: number;
    radius: number;
    brightRadius: number;
    intensity: number;
    color: string;
    castsShadows: boolean;
  }>;
}

/** Fixed, so a saved world and a fresh one agree on what everything is called. */
const SEEDED_AT = "2026-10-05T00:00:00.000000";

/**
 * Seed identifiers are spelled out rather than drawn, so the same seed is the
 * same world in every browser and in a test. `kind` keeps them apart.
 */
export function seedId(kind: number, index: number): string {
  const k = kind.toString(16).padStart(4, "0");
  const i = index.toString(16).padStart(12, "0");
  return `d0000000-0000-4000-${k}-${i}`;
}

export const DEMO_USER = {
  id: seedId(1, 1),
  username: "game-master",
  email: "game-master@demo.invalid",
};

export const DEMO_WORLD_ID = seedId(2, 1);

/**
 * The scenes, in the order the dashboard lists them. The first is the one
 * "Enter world" opens. `map` is a file name in `examples/maps`.
 */
export const DEMO_SCENES: Array<{
  map: string;
  name: string;
  /** Cells from the map's centre, where a token starts. */
  tokens?: Array<[number, number]>;
}> = [
  {
    map: "demo",
    name: "The Proving Ground",
    tokens: [
      [0, 0],
      [1, 0],
      [0, 1],
    ],
  },
  { map: "road-side-in", name: "Roadside Inn" },
  { map: "little-fish-academy", name: "Little Fish Academy" },
  { map: "dwarven-forge", name: "Dwarven Forge" },
  { map: "chamber-of-echoing-grief", name: "Chamber of Echoing Grief" },
  { map: "grassy-path-ambush", name: "Grassy Path Ambush" },
  { map: "azheim-meeting", name: "Azheim Meeting" },
];

export function buildSeed(maps: MapListing[], base: string): DemoState {
  const stamp = { createdAt: SEEDED_AT, updatedAt: SEEDED_AT };
  const by = { createdBy: DEMO_USER.id, updatedBy: DEMO_USER.id };
  const state: DemoState = {
    version: 1,
    world: {
      id: DEMO_WORLD_ID,
      name: "A World To Try",
      description:
        "A table of your own. You are its Game Master, and nothing you do here leaves this browser.",
      gameSystemId: world.gameSystemId,
      interfacePackId: world.interfacePackId,
      scenes: [],
      actors: [],
      tokens: [],
      events: [],
      gameSystem: null,
      interfacePack: null,
      ...by,
      ...stamp,
      sessionNotes: null,
      allowPlayerCreatedActors: false,
      genieResourceCarryoverEnabled: false,
      defaultSceneGridType: "square",
      activeSceneId: null,
      autoApplyNpcDamage: false,
      allowPlayerActorArt: true,
    },
    scenes: [],
    levels: [],
    walls: [],
    tokens: [],
    lights: [],
    shapes: [],
    lore: [],
    chat: [],
    assets: {},
    nextEventId: 1,
  };

  let wall = 0;
  let light = 0;
  let token = 0;
  DEMO_SCENES.forEach((entry, index) => {
    const map = maps.find((candidate) => candidate.name === entry.map);
    if (!map) {
      throw new Error(
        `the demo seed names a map that was not built: ${entry.map}`,
      );
    }
    const sceneId = seedId(3, index + 1);
    const levelId = seedId(4, index + 1);
    const assetId = seedId(5, index + 1);
    // The engine recognises a background by this shape of address and no
    // other, so the demo keeps the shape and answers it from a static file.
    const backgroundUrl = `/api/canvas-assets/${assetId}.webp`;
    state.assets[assetId] = {
      file: `${map.name}.webp`,
      byteSize: map.byteSize,
    };
    state.scenes.push({
      sceneId,
      worldId: DEMO_WORLD_ID,
      name: entry.name,
      // FR-018: the credit travels with every scene made from a map.
      description: MAP_CREDIT_LINE,
      type: "battlemap",
      gridSize: map.gridSize,
      gridType: "square",
      width: map.width,
      height: map.height,
      metadata: null,
      ownerId: DEMO_USER.id,
      ...stamp,
      backgroundImagePath: null,
      backgroundAssetId: assetId,
      backgroundUrl,
      summaryMarkdown: null,
      summaryRenderedHtml: null,
      hidden: false,
      previewUrl: map.hasPreview ? `${base}maps/${map.name}.thumb.webp` : null,
      backgroundGridMismatch: null,
      ambientLight: map.ambientLight,
    });
    state.levels.push({
      levelId,
      sceneId,
      name: "Ground",
      sortOrder: 0,
      isEntry: true,
      hidden: false,
      backgroundAssetId: assetId,
      backgroundUrl,
      width: map.width,
      height: map.height,
      ambientLight: map.ambientLight,
    });
    for (const w of map.walls) {
      wall += 1;
      state.walls.push({
        wallId: seedId(6, wall),
        sceneId,
        levelId,
        x1: w.x1,
        y1: w.y1,
        x2: w.x2,
        y2: w.y2,
        blocksVision: w.blocksVision,
        blocksMovement: w.blocksMovement,
        doorState: w.doorState.toUpperCase(),
        locked: false,
        secret: false,
        metadata: null,
        ...by,
        ...stamp,
      });
    }
    for (const l of map.lights) {
      light += 1;
      state.lights.push({
        lightId: seedId(7, light),
        sceneId,
        levelId,
        ...l,
        attachedTokenId: null,
        metadata: null,
        ...by,
        ...stamp,
      });
    }
    for (const [cx, cy] of entry.tokens ?? []) {
      token += 1;
      state.tokens.push(
        newToken({
          tokenId: seedId(8, token),
          sceneId,
          levelId,
          x: (cx + 0.5) * map.gridSize,
          y: (cy + 0.5) * map.gridSize,
          at: SEEDED_AT,
        }),
      );
    }
  });

  state.world.activeSceneId =
    (state.scenes[0]?.sceneId as string | undefined) ?? null;
  return state;
}

/** A token row with the server's defaults for everything not given. */
export function newToken(given: {
  tokenId: string;
  sceneId: string;
  levelId: string;
  x: number;
  y: number;
  at: string;
  rest?: Row;
}): Row {
  return {
    tokenId: given.tokenId,
    sceneId: given.sceneId,
    levelId: given.levelId,
    actorId: null,
    x: given.x,
    y: given.y,
    rotation: 0,
    scale: 1,
    metadata: null,
    createdAt: given.at,
    updatedAt: given.at,
    ownerUserId: null,
    isPrimary: false,
    photoUrl: null,
    tokenType: "character",
    name: null,
    nameVisibleToPlayers: false,
    linked: false,
    conditions: [],
    ...given.rest,
  };
}
