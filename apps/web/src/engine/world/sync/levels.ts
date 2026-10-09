import { getSceneLevels, type SceneLevel } from "@/api/levels";
import { getTokens } from "@/api/tokens";
import type { WorldEventLike } from "./subscriptionClient";

/**
 * Which level of a scene a viewer is shown, and when to ask again.
 *
 * The engine knows nothing about levels: it is shown one at a time, and
 * changing which is a reload of the board, exactly as changing scene is. So
 * the whole of "levels" on this side is deciding *which one*, and that is
 * decided differently for the two kinds of viewer:
 *
 * - a **player** is on the level their token stands on. They do not choose;
 *   walking onto the stairs chooses for them, and the server is what moved
 *   the token. This module only finds out where it now is.
 * - a **Game Master** is on whichever level they picked, wherever any token
 *   is — they are arranging floors nobody has reached yet.
 *
 * # Why it asks the server rather than watching the token
 *
 * A level is a privacy boundary: a player is answered only for a level a
 * token of theirs stands on. The moment their token travels, the level they
 * were watching answers empty and the token's new row is somewhere this
 * client has not looked. "Where is my token" is therefore not something the
 * board on screen can say — the list of levels the server will now answer for
 * is.
 */

/** A level was added, changed, reordered or deleted (`world_events` code 33). */
export const SCENE_LEVEL_CHANGED_EVENT_CODE = 33;

/**
 * A token changed level (`world_events` code 34), by walking onto a
 * transition or by a Game Master's hand. An ordinary token change (14)
 * follows it, which is what moves the token on the boards that can see it.
 */
export const TOKEN_TRAVELLED_EVENT_CODE = 34;

const TOKEN_EVENT_CODE = 14;

/**
 * A map was imported into a scene (`world_events` code 13,
 * `EVENT_CODE_MAP_IMPORTED`). The import writes the scene's art and size, and
 * the database mirrors them onto the entry level — which is where the board
 * takes its art from. So for the board it is a level change: without reading
 * the levels again, no client's board showed the new map.
 */
export const MAP_IMPORTED_EVENT_CODE = 13;

/** What a world event means for the level on screen. */
export type LevelEventKind =
  /** The list of levels may differ: re-read it. */
  | "levels"
  /** A token changed level: a player may now belong somewhere else. */
  | "travel"
  /** A token changed: a player may have been given one, or lost one. */
  | "token";

/**
 * Classify one world event, or `null` when it says nothing about levels.
 *
 * An event that names another scene is ignored. One that names none is not:
 * a deleted level's event may carry no level, and missing a change is worse
 * than one spare read.
 */
export function levelEventKind(
  event: WorldEventLike,
  sceneId: string,
): LevelEventKind | null {
  const code = event.event_code ?? event.eventCode;
  if (
    code !== SCENE_LEVEL_CHANGED_EVENT_CODE &&
    code !== MAP_IMPORTED_EVENT_CODE &&
    code !== TOKEN_TRAVELLED_EVENT_CODE &&
    code !== TOKEN_EVENT_CODE
  ) {
    return null;
  }

  const payload = (event.token_event ?? event.tokenEvent) as
    | { scene_id?: string; sceneId?: string }
    | undefined;
  const eventSceneId = payload?.scene_id ?? payload?.sceneId;
  if (eventSceneId && eventSceneId !== sceneId) {
    return null;
  }

  if (
    code === SCENE_LEVEL_CHANGED_EVENT_CODE ||
    code === MAP_IMPORTED_EVENT_CODE
  ) {
    return "levels";
  }
  if (code === TOKEN_TRAVELLED_EVENT_CODE) return "travel";
  return "token";
}

/** The level a scene opens on: its entry level, else its lowest. */
export function entryLevel(levels: readonly SceneLevel[]): SceneLevel | null {
  return levels.find((level) => level.isEntry) ?? levels[0] ?? null;
}

/**
 * The level a Game Master is shown: the one they picked while it still
 * exists, else the one the scene opens on.
 */
export function pickedLevel(
  levels: readonly SceneLevel[],
  picked: string | null,
): string | null {
  if (picked && levels.some((level) => level.levelId === picked)) {
    return picked;
  }
  return entryLevel(levels)?.levelId ?? null;
}

/** Who is looking, as far as choosing a level is concerned. */
export interface LevelViewer {
  userId: string | null;
  isGm: boolean;
}

export interface LevelView {
  /** Every level this viewer may read, lowest first. */
  levels: SceneLevel[];
  /** The one to show. `null` only for a scene that answered no levels at all. */
  levelId: string | null;
}

/**
 * Read a scene's levels and decide which one this viewer is on.
 *
 * `picked` is a Game Master's own choice and is ignored for a player.
 *
 * A player is nearly always answered with exactly one level, and that is the
 * answer. More than one means they control tokens on several floors, and then
 * it is their primary token's — the same token the board already looks
 * through (`viewerTokenId` in `WorldPage`) — else the lowest floor one of
 * theirs is on.
 */
export async function resolveLevelView(
  sceneId: string,
  viewer: LevelViewer,
  picked: string | null,
): Promise<LevelView> {
  const levels = await getSceneLevels(sceneId);

  if (viewer.isGm) {
    return { levels, levelId: pickedLevel(levels, picked) };
  }

  if (levels.length <= 1 || !viewer.userId) {
    return { levels, levelId: entryLevel(levels)?.levelId ?? null };
  }

  const userId = viewer.userId;
  const mine = await Promise.all(
    levels.map(async (level) => {
      const tokens = await getTokens(sceneId, level.levelId);
      const owned = tokens.filter((token) => token.ownerUserId === userId);
      return {
        levelId: level.levelId,
        owned: owned.length > 0,
        primary: owned.some((token) => token.isPrimary),
      };
    }),
  );
  const standing =
    mine.find((level) => level.primary) ?? mine.find((level) => level.owned);

  return {
    levels,
    levelId: standing?.levelId ?? entryLevel(levels)?.levelId ?? null,
  };
}

/**
 * Whether a player should be shown the name of the level they are on.
 *
 * A scene with one level shows no level chrome at all — a scene made before
 * levels existed has to look exactly as it did. But a player is told only
 * about the levels they stand on, so "does this scene have more than one" is
 * not something the list can answer for them directly. What it can say: more
 * than one readable level, or a single one that is not where the scene opens.
 * A one-level scene's only level is always its entry level.
 *
 * The case this cannot see is a player on the entry level of a scene whose
 * other floors they do not stand on. They are shown nothing, which is the
 * same as a one-level scene — and is also all the server has told them.
 */
export function playerSeesLevelName(levels: readonly SceneLevel[]): boolean {
  if (levels.length > 1) {
    return true;
  }
  const [only] = levels;
  return only !== undefined && !only.isEntry;
}

/**
 * The key a board's contents are loaded under.
 *
 * Scene and level together, because switching level reloads the board the
 * way switching scene does, and what a player has explored on one floor is
 * not what they have explored on another.
 */
export function boardKey(sceneId: string, levelId: string | null): string {
  return levelId ? `${sceneId}:${levelId}` : sceneId;
}
