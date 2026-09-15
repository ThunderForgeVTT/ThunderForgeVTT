/**
 * tokenGrid.ts — how many squares each token fills, as its game system
 * declares sizes.
 *
 * Spec 046 US4, FR-031 (research R10, contract §5). The server resolves each
 * token's size — a linked token's from its actor, a copy's from its NPC —
 * against the system's `combat.sizes`, and this hands every answer to the
 * engine through `set_token_grid`. That command has existed since spec 001,
 * and everything that should follow a creature's size already reads what it
 * sets: snapping, hit-testing, keyboard moves, nameplates and bars. Until now
 * nothing in the product sent it, so a Large ogre was a one-square token drawn
 * twice as big.
 *
 * Shaped like `tokenVision.ts`, for the same reasons: read rather than pushed,
 * because a size is on a *sheet* and a sheet edit is not a scene event; and
 * every token told, because a token left out would keep a size it lost.
 */
import { postGraphQL } from "@/api/graphqlClient";
import type { WorldStore } from "../store";
import { ACTOR_SHEET_CHANGED_EVENT_CODE } from "./tokenVision";

export type TokenGrid = {
  tokenId: string;
  /** Squares a side. */
  footprint: number;
};

type TokenGridQuery = { tokenGrid: TokenGrid[] };

/** The footprint of every token in this scene that fills other than one square. */
export async function getTokenGrid(sceneId: string): Promise<TokenGrid[]> {
  const answer = await postGraphQL<TokenGridQuery>(
    `
      query TokenGrid($sceneId: UUID!) {
        tokenGrid(sceneId: $sceneId) {
          tokenId
          footprint
        }
      }
    `,
    { sceneId },
  );
  return answer.tokenGrid;
}

/**
 * Read the scene's footprints and tell the engine.
 *
 * Tokens the server omits fill one square, and are told so explicitly —
 * `footprint: 1`. A creature whose size goes from Large to Medium has to
 * shrink on every board, and the engine would never hear that it had if the
 * token were simply left out.
 */
export async function loadTokenGridIntoEngine(
  worldStore: WorldStore,
  sceneId: string,
  tokenIds: Iterable<string>,
): Promise<void> {
  let sized: TokenGrid[];
  try {
    sized = await getTokenGrid(sceneId);
  } catch (error) {
    // A system that declares no sizes answers with an empty list, not an
    // error. An error is a real failure, and the honest response is to leave
    // every token as it is rather than to shrink them all to one square.
    console.error("Failed to read token sizes:", error);
    return;
  }

  const byToken = new Map(sized.map((grid) => [grid.tokenId, grid.footprint]));
  for (const tokenId of tokenIds) {
    worldStore.dispatch(
      {
        type: "set_token_grid",
        tokenId,
        footprint: byToken.get(tokenId) ?? 1,
      },
      "sync",
    );
  }
}

type WorldEventLike = { event_code?: number; eventCode?: number };

/**
 * Re-read the scene's footprints when any sheet changes (event 26).
 *
 * Event 14 (a token changed) is handled where the tokens are re-read
 * (`tokens.ts`), after them, so a new token is in the engine before it is
 * sized. This joins `WorldPage`'s one fan-out for the sheet event, as
 * `applyTokenVisionWorldEvent` does.
 */
export async function applyTokenGridWorldEvent(
  worldStore: WorldStore,
  sceneId: string,
  event: WorldEventLike,
  tokenIdsNow: () => Iterable<string>,
): Promise<void> {
  const code = event.event_code ?? event.eventCode;
  if (code !== ACTOR_SHEET_CHANGED_EVENT_CODE) {
    return;
  }
  await loadTokenGridIntoEngine(worldStore, sceneId, tokenIdsNow());
}
