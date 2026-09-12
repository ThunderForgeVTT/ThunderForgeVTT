/**
 * tokenVision.ts — how far each token sees, as its game system declares it.
 *
 * Spec 045 US6, owner decision 2. The server resolves a character's sheet
 * against its system's `vision` block and answers in world units; this hands
 * each answer to the engine through `set_token_vision`, which has existed
 * since spec 001 and which nothing in the product has ever called.
 *
 * Read rather than pushed. A token's sight changes when its *sheet* changes —
 * a pair of goggles, a level, a race — which is an actor edit, not a scene
 * event. Sheet edits announced nothing at all until spec 045 gave them a code
 * of their own, so this listens for that as well as loading on scene entry.
 */
import { postGraphQL } from "@/api/graphqlClient";
import type { WorldStore } from "../store";

export type TokenVision = {
  tokenId: string;
  darkvision: number;
  carriedBright: number;
  carriedDim: number;
};

type TokenVisionQuery = { tokenVision: TokenVision[] };

/** What every token in this scene sees, for the tokens that see unusually. */
export async function getTokenVision(sceneId: string): Promise<TokenVision[]> {
  const answer = await postGraphQL<TokenVisionQuery>(
    `
      query TokenVision($sceneId: UUID!) {
        tokenVision(sceneId: $sceneId) {
          tokenId
          darkvision
          carriedBright
          carriedDim
        }
      }
    `,
    { sceneId },
  );
  return answer.tokenVision;
}

/**
 * Read the scene's vision and tell the engine.
 *
 * Tokens the server omits are the ones that see by the default rules, and
 * they are told so explicitly — `darkvision: 0`. Leaving them out instead
 * would mean a character who *loses* their darkvision keeps it on every board
 * until a reload, because the engine would never hear that it had gone.
 */
export async function loadTokenVisionIntoEngine(
  worldStore: WorldStore,
  sceneId: string,
  tokenIds: Iterable<string>,
): Promise<void> {
  let seeing: TokenVision[];
  try {
    seeing = await getTokenVision(sceneId);
  } catch (error) {
    // A world whose system declares no vision answers with an empty list, not
    // an error — so an error here is a real failure, and the honest response
    // is to leave every token's sight as it was rather than to blank it.
    console.error("Failed to read token vision:", error);
    return;
  }

  const byToken = new Map(seeing.map((vision) => [vision.tokenId, vision]));
  for (const tokenId of tokenIds) {
    const vision = byToken.get(tokenId);
    worldStore.dispatch(
      {
        type: "set_token_vision",
        tokenId,
        darkvision: vision?.darkvision ?? 0,
      },
      "sync",
    );
  }
}

/** A sheet changed somewhere in this world (`world_events` code 26). */
export const ACTOR_SHEET_CHANGED_EVENT_CODE = 26;

type WorldEventLike = { event_code?: number; eventCode?: number };

/**
 * Re-read the scene's vision when any character's sheet changes (FR-067).
 *
 * Shaped like the other `apply*WorldEvent` functions so it joins the one
 * fan-out in `WorldPage` rather than opening a second subscription to the
 * same bus — a second subscriber is a second thing to remember to cancel.
 *
 * Re-reads the whole scene rather than the one actor: the payload names an
 * actor, and mapping that to the tokens standing for it costs another query.
 * One query already answers for every token, and a sheet edit happens at the
 * speed somebody types, not per frame.
 */
export async function applyTokenVisionWorldEvent(
  worldStore: WorldStore,
  sceneId: string,
  event: WorldEventLike,
  tokenIdsNow: () => Iterable<string>,
): Promise<void> {
  const code = event.event_code ?? event.eventCode;
  if (code !== ACTOR_SHEET_CHANGED_EVENT_CODE) {
    return;
  }
  await loadTokenVisionIntoEngine(worldStore, sceneId, tokenIdsNow());
}
