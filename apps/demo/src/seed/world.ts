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
import { artUrl, tokenPhotoUrl, type ArtRole } from "./art";
import { CAST, slotRows } from "./cast";
import { seedLore } from "./lore";

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

/**
 * The seeded player, for "view as player": a member who owns the heroes and
 * nothing else, so the real client renders their view of the table honestly.
 */
export const DEMO_PLAYER = {
  id: seedId(1, 2),
  username: "player",
  email: "player@demo.invalid",
};

export const DEMO_WORLD_ID = seedId(2, 1);

/** Whose eyes the visitor is looking through. */
export type Viewer = "gm" | "player";

/**
 * The scenes, in the order the dashboard lists them. The first is the one
 * "Enter world" opens. `map` is a file name in `examples/maps`.
 */
export const DEMO_SCENES: Array<{
  map: string;
  name: string;
  /** Cells from the map's centre, where a token with no actor starts. */
  tokens?: Array<[number, number]>;
  /**
   * Who stands where, as a member of the cast and a cell counted from the
   * map's top-left corner, the way the map's own grid is read.
   */
  encounter?: Array<{ who: string; name?: string; cell: [number, number] }>;
}> = [
  {
    map: "grassy-path-ambush",
    name: "Grassy Path Ambush",
    // The heroes are on the road where it crosses the middle of the map, so
    // the fight is on screen where the camera opens. The ambushers are in the
    // brush above the road and behind the rocks below it, and hidden until
    // the Game Master says otherwise. Cells count from the top-left corner.
    encounter: [
      { who: "fighter", cell: [20, 12] },
      { who: "wizard", cell: [18, 13] },
      { who: "goblin", name: "Goblin 1", cell: [22, 8] },
      { who: "goblin", name: "Goblin 2", cell: [27, 7] },
      { who: "goblin", name: "Goblin 3", cell: [18, 16] },
      { who: "hobgoblin", cell: [29, 9] },
      { who: "wolf", cell: [25, 17] },
    ],
  },
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
  { map: "azheim-meeting", name: "Azheim Meeting" },
];

export function buildSeed(maps: MapListing[], base: string): DemoState {
  const stamp = { createdAt: SEEDED_AT, updatedAt: SEEDED_AT };
  const by = { createdBy: DEMO_USER.id, updatedBy: DEMO_USER.id };
  const state: DemoState = {
    version: 4,
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
    lore: seedLore(DEMO_WORLD_ID, DEMO_USER.id, SEEDED_AT, seedId),
    chat: [],
    actors: [],
    systemData: [],
    assets: {},
    art: {},
    viewer: "gm",
    claimedActorId: null,
    nextEventId: 1,
  };

  // The cast lives on the first scene, as the server would have it; an actor
  // belongs to a scene by column even when its tokens are elsewhere.
  const homeSceneId = seedId(3, 1);
  CAST.forEach((member, index) => {
    const actorId = seedId(9, index + 1);
    const owner = member.isNpc ? DEMO_USER.id : DEMO_PLAYER.id;
    // Spec 044: a token and a portrait, as the hero builder would have saved
    // them. The token's art is what the engine draws on the board.
    const roles: ArtRole[] = ["token", "portrait"];
    const images = roles.map((role, r) => {
      const assetId = seedId(11 + r, index + 1);
      state.art[assetId] = { role, look: member.look };
      return {
        id: seedId(13 + r, index + 1),
        actorId,
        role,
        assetId,
        url: artUrl(assetId),
        thumbnailUrl: `${artUrl(assetId)}/thumb`,
        heroSpec: member.look,
      };
    });
    state.actors.push({
      id: actorId,
      worldId: DEMO_WORLD_ID,
      sceneId: homeSceneId,
      actorType: "character",
      gameSystemId: world.gameSystemId,
      label: member.label,
      description: member.description,
      isPublic: false,
      isNpc: member.isNpc,
      createdBy: DEMO_USER.id,
      ownedBy: owner,
      ...stamp,
      // Spec 017: the heroes may be claimed; the player ships with one.
      availableForClaim: !member.isNpc,
      isUnique: member.isUnique,
      visibleToPlayers: member.visibleToPlayers,
      artLocked: false,
      images,
      loreLinkedFrom: [],
      claimedBy: null,
      // The demo's own key, so the encounter and the tests can find it.
      castKey: member.key,
    });
    const slots = Object.fromEntries(
      slotRows(member).map(([dataType, data]) => [
        dataType.replace(/_([a-z])/g, (_, c: string) => c.toUpperCase()),
        data,
      ]),
    );
    state.systemData.push({
      id: seedId(10, index + 1),
      actorId,
      gameSystemId: world.gameSystemId,
      ...slots,
      ...stamp,
    });
  });
  // The player arrives already playing the fighter, so "view as player"
  // opens on a character and not on a choice.
  state.claimedActorId =
    (state.actors.find((a) => a.castKey === "fighter")?.id as string) ?? null;

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
    for (const placed of entry.encounter ?? []) {
      const actor = state.actors.find((a) => a.castKey === placed.who);
      if (!actor) {
        throw new Error(
          `the encounter names nobody in the cast: ${placed.who}`,
        );
      }
      token += 1;
      // The scene's origin is its centre with y growing up; the map's grid is
      // read from the top-left with y growing down (map_import/geometry.rs).
      const [col, row] = placed.cell;
      state.tokens.push(
        newToken({
          tokenId: seedId(8, token),
          sceneId,
          levelId,
          x: (col + 0.5) * map.gridSize - map.width / 2,
          y: map.height / 2 - (row + 0.5) * map.gridSize,
          at: SEEDED_AT,
          rest: tokenOf(actor, placed.name),
        }),
      );
    }
  });

  state.world.activeSceneId =
    (state.scenes[0]?.sceneId as string | undefined) ?? null;
  return state;
}

/**
 * What a token takes from its actor, as the server fills it in at read time
 * (`token_art.rs`) and at creation (`mutations_token_links.rs`): a named
 * individual or a hero is linked, a kind is placed as a copy; the name is the
 * actor's unless the token has one of its own.
 */
export function tokenOf(actor: Row, name?: string): Row {
  const linked = !actor.isNpc || actor.isUnique === true;
  const art = (actor.images as Row[] | undefined)?.find(
    (image) => image.role === "token",
  );
  return {
    // The server resolves a token's art to its actor's token image when the
    // token is read (`token_art.rs`); the demo writes it in when it is made.
    photoUrl: art ? tokenPhotoUrl(art.assetId as string) : null,
    actorId: actor.id,
    ownerUserId: actor.isNpc ? null : actor.ownedBy,
    tokenType: actor.isNpc ? "npc" : "character",
    linked,
    isPrimary: linked,
    name: name ?? actor.label,
    nameVisibleToPlayers: !actor.isNpc,
    metadata: name ? { label: name } : null,
  };
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
