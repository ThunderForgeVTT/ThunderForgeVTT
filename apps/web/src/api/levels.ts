import { postGraphQL } from "@/api/graphqlClient";

/**
 * The levels of a scene: a tavern, the rooms above it, the street outside.
 *
 * A scene owns up to twelve ordered levels, and everything on the board
 * stands on exactly one. Every rule this module appears to describe is
 * enforced on the server — who may read a level, that the last one cannot be
 * deleted, that an occupied one cannot — so nothing here checks. See
 * `viewedLevel.ts` for how the level on screen reaches the per-scene reads.
 */

/** One level, as this viewer may see it. */
export interface SceneLevel {
  levelId: string;
  sceneId: string;
  name: string;
  /** Position among the scene's levels, lowest floor first. */
  sortOrder: number;
  /** The level a scene opens on. Exactly one per scene. */
  isEntry: boolean;
  /** A Game Master's note that the floor is not ready. Not what keeps players out. */
  hidden: boolean;
  backgroundAssetId: string | null;
  /**
   * Where the level's art can be fetched. The board has to use this and not
   * the scene's own: a player upstairs is refused the scene's background.
   */
  backgroundUrl: string | null;
  width: number;
  height: number;
  /** `bright`, `dim` or `dark`. */
  ambientLight: string;
  /** How many tokens stand on it. Game Master only; `null` for anyone else. */
  tokenCount: number | null;
}

const LEVEL_FIELDS = `
  levelId
  sceneId
  name
  sortOrder
  isEntry
  hidden
  backgroundAssetId
  backgroundUrl
  width
  height
  ambientLight
  tokenCount
`;

/**
 * The levels of a scene this viewer may read, lowest first.
 *
 * A Game Master is answered with all of them. A player is answered only with
 * the levels a token of theirs stands on — the entry level if they have none —
 * so for a player this is not a list of what exists.
 */
export function getSceneLevels(sceneId: string): Promise<SceneLevel[]> {
  return postGraphQL<{ sceneLevels: SceneLevel[] }>(
    `
      query SceneLevels($sceneId: UUID!) {
        sceneLevels(sceneId: $sceneId) {
          ${LEVEL_FIELDS}
        }
      }
    `,
    { sceneId },
  ).then((data) => data.sceneLevels);
}

export interface CreateSceneLevelInput {
  sceneId: string;
  name: string;
  hidden?: boolean;
  /** Omitted, the scene's own size. */
  width?: number;
  height?: number;
  /** Omitted, the scene's own light. */
  ambientLight?: string;
  backgroundAssetId?: string;
}

/** Add a level above the scene's others. Game Masters only. */
export function createSceneLevel(
  input: CreateSceneLevelInput,
): Promise<SceneLevel> {
  return postGraphQL<{ createSceneLevel: SceneLevel }>(
    `
      mutation CreateSceneLevel($input: GraphQLCreateSceneLevelInput!) {
        createSceneLevel(input: $input) {
          ${LEVEL_FIELDS}
        }
      }
    `,
    { input },
  ).then((data) => data.createSceneLevel);
}

export interface UpdateSceneLevelInput {
  name?: string;
  hidden?: boolean;
  width?: number;
  height?: number;
  ambientLight?: string;
  backgroundAssetId?: string;
  /**
   * Take the art away. Needed because an absent `backgroundAssetId` means
   * "leave it alone" in a partial update.
   */
  clearBackground?: boolean;
  /** Make this the level the scene opens on; the old entry level stops being it. */
  makeEntry?: boolean;
}

export function updateSceneLevel(
  levelId: string,
  input: UpdateSceneLevelInput,
): Promise<SceneLevel> {
  return postGraphQL<{ updateSceneLevel: SceneLevel }>(
    `
      mutation UpdateSceneLevel(
        $levelId: UUID!
        $input: GraphQLUpdateSceneLevelInput!
      ) {
        updateSceneLevel(levelId: $levelId, input: $input) {
          ${LEVEL_FIELDS}
        }
      }
    `,
    { levelId, input },
  ).then((data) => data.updateSceneLevel);
}

/** Put a scene's levels in a new order, lowest first. Every level, once. */
export function reorderSceneLevels(
  sceneId: string,
  levelIds: string[],
): Promise<SceneLevel[]> {
  return postGraphQL<{ reorderSceneLevels: SceneLevel[] }>(
    `
      mutation ReorderSceneLevels($sceneId: UUID!, $levelIds: [UUID!]!) {
        reorderSceneLevels(sceneId: $sceneId, levelIds: $levelIds) {
          ${LEVEL_FIELDS}
        }
      }
    `,
    { sceneId, levelIds },
  ).then((data) => data.reorderSceneLevels);
}

/**
 * Delete a level. Refused for a scene's last level, its entry level, and one
 * that still has anything standing on it — the refusal is the server's, in
 * words written to be shown.
 */
export function deleteSceneLevel(levelId: string): Promise<string> {
  return postGraphQL<{ deleteSceneLevel: string }>(
    `
      mutation DeleteSceneLevel($levelId: UUID!) {
        deleteSceneLevel(levelId: $levelId)
      }
    `,
    { levelId },
  ).then((data) => data.deleteSceneLevel);
}

/** A token after it was put on another level: where, and on which. */
export interface MovedToken {
  tokenId: string;
  levelId: string;
  x: number;
  y: number;
}

/**
 * Put tokens on another level of their scene. Game Masters only.
 *
 * This is the deliberate way across: a Game Master dragging a token over the
 * stairs never travels, because they are arranging the board. With no
 * position the tokens keep the one they had.
 */
export function moveTokensToLevel(
  tokenIds: string[],
  levelId: string,
  at?: { x: number; y: number },
): Promise<MovedToken[]> {
  return postGraphQL<{ moveTokensToLevel: MovedToken[] }>(
    `
      mutation MoveTokensToLevel(
        $tokenIds: [UUID!]!
        $levelId: UUID!
        $x: Float
        $y: Float
      ) {
        moveTokensToLevel(tokenIds: $tokenIds, levelId: $levelId, x: $x, y: $y) {
          tokenId
          levelId
          x
          y
        }
      }
    `,
    { tokenIds, levelId, x: at?.x, y: at?.y },
  ).then((data) => data.moveTokensToLevel);
}
