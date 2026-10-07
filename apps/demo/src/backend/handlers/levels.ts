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

export const levelQueries: Record<string, Handler> = {
  sceneLevels: ({ sceneId }) => {
    const state = demoState();
    return readableLevels(state, sceneId).map((l) => levelRow(state, l));
  },
};

export const levelMutations: Record<string, Handler> = {
  updateSceneLevel: ({ levelId, input }) => updateLevel(levelId, input),
};
