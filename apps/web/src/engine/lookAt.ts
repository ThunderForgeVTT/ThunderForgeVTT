/**
 * "Look at this creature", and the camera that follows a turn (owner decision
 * 2026-09-15).
 *
 * Every spatial decision here is the engine's (Constitution Principle I). This
 * file asks by name and remembers one preference; it does not pan, zoom, or
 * work out where anything is.
 */

import { getBoundWorldStore } from "@/engine/bevy";
import type { TokenRecord } from "@/types/token";

/**
 * How many cells of surroundings the camera frames when it follows a turn.
 *
 * Six. The owner asked to be shown "what's around me in a decent radius": six
 * cells is 30 feet on a 5-foot grid, which reaches past every ordinary weapon
 * and most movement in a turn, so the creatures that can act on you next are
 * in frame. The engine counts them *around* the creature rather than through
 * it, so a Large token gets the same six cells of context a Medium one does
 * (`framed_extent`) — 14 cells across for a Large creature, 13 for a Medium.
 *
 * A plain locate passes nothing at all and keeps the viewer's own zoom: they
 * chose it, and "show me Boblin" is not a request to re-frame the board.
 */
export const FOLLOW_SURROUND_CELLS = 6;

/**
 * Whether this viewer asked the system for less movement.
 *
 * Read per call rather than cached: the setting can change under a running
 * page, and there is nothing expensive about asking. A browser without
 * `matchMedia` — or one that throws on it — is treated as not having asked,
 * which is the pre-existing behaviour of every other animation in the app.
 */
export function prefersReducedMotion(): boolean {
  try {
    return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  } catch {
    return false;
  }
}

/**
 * Asks the engine to put a token on screen.
 *
 * Dispatched through the bound world store, which is the application's one
 * channel to the engine — not a second way in. A page with no engine bound yet
 * (the store is bound after mount) drops the request rather than throwing:
 * there is no camera to move.
 */
export function lookAtToken(
  tokenId: string,
  options: { surroundCells?: number } = {},
): void {
  const store = getBoundWorldStore();
  if (!store) return;
  store.dispatch(
    {
      type: "focus_token",
      tokenId,
      ...(options.surroundCells === undefined
        ? {}
        : { surroundCells: options.surroundCells }),
      immediate: prefersReducedMotion(),
    },
    "ui",
  );
}

/**
 * Whether this viewer's chrome should offer to look at a creature.
 *
 * The rule the engine enforces is in `systems::camera_focus`: the board has to
 * draw the creature, and the viewer has to be allowed to read its name. This
 * answers the half the web can see — the name — and leaves the other half to
 * the engine, which is the only thing that knows frame by frame what a
 * viewer's board is currently drawing.
 *
 * Offering a control the engine will refuse is the failure this avoids. Hiding
 * one the engine would have allowed is the failure it must not cause, which is
 * why sight is deliberately *not* checked here: a creature that steps behind a
 * pillar must not have its control flicker away mid-fight.
 *
 * A Game Master may look at anything. `nameVisibleToPlayers` is their own
 * switch, not a restriction on them — the server sends a Game Master the
 * switch as they set it, and everyone else the effective rule.
 */
export function mayLookAt(
  token: Pick<TokenRecord, "nameVisibleToPlayers"> | undefined,
  isGm: boolean,
): boolean {
  if (!token) return false;
  if (isGm) return true;
  return token.nameVisibleToPlayers !== false;
}
