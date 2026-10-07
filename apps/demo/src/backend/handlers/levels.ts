/**
 * A scene's levels: the floors of one place, as the server keeps them.
 *
 * Mirrors `crates/thunderforge-server/src/scene_levels.rs` (the rules and
 * their refusals) and `graphql/mutations_levels.rs`; who may read which floor
 * is `auth/level_visibility.rs`. Every change is the Game Master's alone and
 * is announced as code 33, naming the level and nothing about it, so each
 * client re-reads `sceneLevels` and is answered for who it is.
 *
 * The entry level and the scene are one board: an edit to the entry level's
 * background, size or light is written to the scene as well.
 */
import { DEMO_PLAYER } from "../../seed/world";
import { viewerIsGm, viewerUser } from "../actors";
import { EVENT, record } from "../events";
import { demoState, markChanged, type DemoState, type Row } from "../state";
import {
  ambientOf,
  levelsOf,
  refuse,
  sceneOf,
  type Args,
  type Handler,
} from "./common";

function authorize(state: DemoState): void {
  if (!viewerIsGm(state)) {
    refuse("Only the Game Master may change a scene's levels");
  }
}

function loadLevel(state: DemoState, levelId: string): Row {
  return (
    state.levels.find((l) => l.levelId === levelId) ?? refuse("Level not found")
  );
}

function checkedName(name: string): string {
  const trimmed = name.trim();
  if (!trimmed) refuse("A level needs a name");
  if ([...trimmed].length > 80) {
    refuse("A level's name is at most 80 characters");
  }
  return trimmed;
}

function checkedSize(width?: number | null, height?: number | null): void {
  if ((width != null && width <= 0) || (height != null && height <= 0)) {
    refuse("A level's width and height must be more than zero");
  }
}

function checkedLight(light?: string | null): string | null {
  if (light == null) return null;
  return ambientOf(light) ?? refuse("A level's light is bright, dim or dark");
}

/** An asset id is a bare reference; another world's art is not reachable. */
function checkedBackground(state: DemoState, assetId?: string | null): void {
  if (assetId != null && !state.assets[assetId]) {
    refuse("That image is not in this world");
  }
}

export function backgroundUrlOf(assetId: string | null): string | null {
  return assetId ? `/api/canvas-assets/${assetId}.webp` : null;
}

function announce(sceneId: string, action: string, levelId: string | null) {
  record(EVENT.sceneLevel, {
    action,
    scene_id: sceneId,
    level_id: levelId,
  });
}

/** The tokens a player controls: their own, and their claimed hero's. */
function controls(state: DemoState, token: Row): boolean {
  const me = viewerUser(state).id;
  return (
    token.ownerUserId === me ||
    (me === DEMO_PLAYER.id &&
      token.actorId != null &&
      token.actorId === state.claimedActorId)
  );
}

/**
 * `level_visibility::readable`: a Game Master reads every floor; anyone else
 * the floors their tokens stand on, or, with none in the scene, the entry.
 */
export function readableLevels(state: DemoState, sceneId: string): Row[] {
  const levels = levelsOf(state, sceneId);
  if (viewerIsGm(state)) return levels;
  const standingOn = new Set(
    state.tokens
      .filter((t) => t.sceneId === sceneId && controls(state, t))
      .map((t) => t.levelId),
  );
  const occupied = levels.filter((l) => standingOn.has(l.levelId));
  return occupied.length > 0 ? occupied : levels.filter((l) => l.isEntry);
}

/** A level as `sceneLevels` answers it: counted for a Game Master only. */
export function levelRow(state: DemoState, level: Row): Row {
  return {
    ...level,
    tokenCount: viewerIsGm(state)
      ? state.tokens.filter((t) => t.levelId === level.levelId).length
      : null,
  };
}

function updateLevel(levelId: string, input: Args): Row {
  const state = demoState();
  const level = loadLevel(state, levelId);
  const sceneId = level.sceneId as string;
  authorize(state);
  const name = input.name != null ? checkedName(input.name) : null;
  checkedSize(input.width, input.height);
  const light = checkedLight(input.ambientLight);
  if (!input.clearBackground) checkedBackground(state, input.backgroundAssetId);

  if (name != null) level.name = name;
  if (input.hidden != null) level.hidden = input.hidden;

  const asset: string | null = input.clearBackground
    ? null
    : ((input.backgroundAssetId ?? level.backgroundAssetId) as string | null);
  level.backgroundAssetId = asset;
  level.backgroundUrl = backgroundUrlOf(asset);
  level.width = input.width ?? level.width;
  level.height = input.height ?? level.height;
  level.ambientLight = light ?? level.ambientLight;

  const becomesEntry = input.makeEntry === true && !level.isEntry;
  if (becomesEntry) {
    for (const other of levelsOf(state, sceneId)) other.isEntry = false;
    level.isEntry = true;
  }
  if (level.isEntry) {
    // The scene leads: what the entry level shows is the scene's board.
    const scene = sceneOf(state, sceneId);
    scene.backgroundAssetId = asset;
    scene.backgroundUrl = level.backgroundUrl;
    scene.backgroundImagePath = null;
    scene.width = level.width;
    scene.height = level.height;
    scene.ambientLight = level.ambientLight;
  }

  announce(sceneId, "updated", levelId);
  markChanged();
  return levelRow(state, level);
}

const MAX_LEVELS_PER_SCENE = 12;

function createLevel(input: Args): Row {
  const state = demoState();
  const sceneId = input.sceneId as string;
  const scene = sceneOf(state, sceneId);
  authorize(state);
  const name = checkedName(String(input.name ?? ""));
  checkedSize(input.width, input.height);
  const light = checkedLight(input.ambientLight);
  checkedBackground(state, input.backgroundAssetId);
  const existing = levelsOf(state, sceneId);
  if (existing.length >= MAX_LEVELS_PER_SCENE) {
    refuse("A scene has at most 12 levels");
  }
  const top = existing.reduce(
    (highest, l) => Math.max(highest, (l.sortOrder as number) + 1),
    0,
  );
  const asset = (input.backgroundAssetId as string | null | undefined) ?? null;
  const level: Row = {
    levelId: crypto.randomUUID(),
    sceneId,
    name,
    sortOrder: top,
    isEntry: false,
    hidden: input.hidden ?? false,
    backgroundAssetId: asset,
    backgroundUrl: backgroundUrlOf(asset),
    width: input.width ?? scene.width,
    height: input.height ?? scene.height,
    ambientLight: light ?? scene.ambientLight,
  };
  state.levels.push(level);
  announce(sceneId, "created", level.levelId as string);
  markChanged();
  return levelRow(state, level);
}

function reorderLevels(sceneId: string, ordered: string[]): Row[] {
  const state = demoState();
  sceneOf(state, sceneId);
  authorize(state);
  const current = levelsOf(state, sceneId)
    .map((l) => l.levelId as string)
    .sort();
  const named = [...ordered].sort();
  if (
    current.length !== named.length ||
    current.some((id, i) => id !== named[i])
  ) {
    refuse("Name every level of the scene once, in the order you want");
  }
  ordered.forEach((levelId, position) => {
    loadLevel(state, levelId).sortOrder = position;
  });
  announce(sceneId, "reordered", null);
  markChanged();
  return levelsOf(state, sceneId).map((l) => levelRow(state, l));
}

/** Walls, lights and shapes go with the floor, by foreign key. */
function deleteLevel(levelId: string): string {
  const state = demoState();
  const level = loadLevel(state, levelId);
  const sceneId = level.sceneId as string;
  authorize(state);
  if (levelsOf(state, sceneId).length <= 1) {
    refuse("A scene keeps at least one level");
  }
  if (level.isEntry) {
    refuse(
      "This is the scene's entry level. Make another level the entry before deleting it",
    );
  }
  if (state.tokens.some((t) => t.levelId === levelId)) {
    refuse("Move the tokens off this level before deleting it");
  }
  const off = (row: Row) => row.levelId !== levelId;
  state.walls = state.walls.filter(off);
  state.lights = state.lights.filter(off);
  state.shapes = state.shapes.filter(off);
  state.levels = state.levels.filter(off);
  announce(sceneId, "deleted", levelId);
  markChanged();
  return levelId;
}

/** `level_travel::free_spot`: the point, else the nearest free grid step. */
export function freeSpot(
  centre: [number, number],
  step: number,
  taken: Array<[number, number]>,
): [number, number] {
  const near = step / 2;
  const isFree = ([x, y]: [number, number]) =>
    !taken.some(
      ([ox, oy]) => Math.abs(ox - x) < near && Math.abs(oy - y) < near,
    );
  if (isFree(centre)) return centre;
  for (let ring = 1; ring <= 2; ring += 1) {
    for (let dy = -ring; dy <= ring; dy += 1) {
      for (let dx = -ring; dx <= ring; dx += 1) {
        if (Math.abs(dx) !== ring && Math.abs(dy) !== ring) continue;
        const spot: [number, number] = [
          centre[0] + dx * step,
          centre[1] + dy * step,
        ];
        if (isFree(spot)) return spot;
      }
    }
  }
  return centre;
}

/** The Game Master's lift (`scene_levels::move_tokens_to_level`). */
function moveTokensToLevel(args: Args): Row[] {
  const { tokenIds, levelId, x, y } = args as {
    tokenIds: string[];
    levelId: string;
    x?: number | null;
    y?: number | null;
  };
  if ((x == null) !== (y == null)) refuse("Give both x and y, or neither");
  const state = demoState();
  const level = loadLevel(state, levelId);
  const sceneId = level.sceneId as string;
  authorize(state);
  if (tokenIds.length === 0) return [];
  const at: [number, number] | null = x != null && y != null ? [x, y] : null;
  if (at && (!Number.isFinite(at[0]) || !Number.isFinite(at[1]))) {
    refuse("That is not a place on the level");
  }
  const moving = tokenIds.map(
    (id) =>
      state.tokens.find((t) => t.tokenId === id && t.sceneId === sceneId) ??
      refuse("Every token must be in the same scene as the level"),
  );
  const step = Math.max(1, sceneOf(state, sceneId).gridSize as number);
  const taken: Array<[number, number]> = state.tokens
    .filter(
      (t) => t.levelId === levelId && !tokenIds.includes(t.tokenId as string),
    )
    .map((t) => [t.x as number, t.y as number]);
  const stamp = now();
  for (const token of moving) {
    token.levelId = levelId;
    if (at) {
      const [fx, fy] = freeSpot(at, step, taken);
      taken.push([fx, fy]);
      token.x = fx;
      token.y = fy;
    }
    token.updatedAt = stamp;
    const carried = state.lights.filter(
      (l) => l.attachedTokenId === token.tokenId,
    );
    for (const light of carried) {
      light.levelId = levelId;
      light.x = token.x;
      light.y = token.y;
    }
    // `level_travel::announce`.
    record(EVENT.tokenTravelled, {
      token_id: token.tokenId,
      scene_id: sceneId,
    });
    record(EVENT.token, {
      action: "updated",
      token_id: token.tokenId,
      scene_id: sceneId,
    });
    if (carried.length > 0) {
      record(EVENT.light, { action: "updated", scene_id: sceneId });
    }
  }
  markChanged();
  return moving;
}

function now(): string {
  return new Date().toISOString();
}

export const levelQueries: Record<string, Handler> = {
  sceneLevels: ({ sceneId }) => {
    const state = demoState();
    return readableLevels(state, sceneId).map((l) => levelRow(state, l));
  },
};

export const levelMutations: Record<string, Handler> = {
  createSceneLevel: ({ input }) => createLevel(input),
  updateSceneLevel: ({ levelId, input }) => updateLevel(levelId, input),
  reorderSceneLevels: ({ sceneId, levelIds }) =>
    reorderLevels(sceneId, levelIds),
  deleteSceneLevel: ({ levelId }) => deleteLevel(levelId),
  moveTokensToLevel: (args) => moveTokensToLevel(args),
};
