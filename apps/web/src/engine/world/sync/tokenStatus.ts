import { getTokenStatus } from "@/api/tokenStatus";
import type { WorldStore } from "../store";
import type { WorldEventLike } from "./subscriptionClient";
import type { StatusCommand } from "@/engine/sdk/commands";

/**
 * Getting each token's resources onto the canvas, and keeping them current.
 *
 * Spec 029. Mirrors the wall/light/shape sync path: a world event says
 * *something changed*, and this re-reads the scene's status and dispatches it.
 *
 * # Why it re-reads rather than applying a delta
 *
 * The event carries an id, not a payload, and that is the right shape here for
 * a reason beyond convention: what a viewer may see depends on the viewer. A
 * delta carrying values would have to be computed per subscriber and coarsened
 * per subscriber before it left the server, and the moment one of those is
 * missed the wrong person sees an exact number. Re-reading means the answer is
 * always produced by the one resolver that knows how to coarsen.
 *
 * The cost is a query per relevant event, against a table read the server
 * already does for the scene. That is the cheaper mistake.
 */

/** Token values changed. */
const EVENT_CODE_TOKEN_CHANGED = 14;

/**
 * What a token *discloses* changed — a Game Master revealing or hiding.
 *
 * Separate from a value change because they are different facts: one moves a
 * bar, the other can make one appear, vanish, or stop being an estimate. Both
 * land here because both mean "re-read", but a client that later wants to
 * treat them differently has the distinction available rather than having to
 * reintroduce it.
 */
const EVENT_CODE_TOKEN_DISCLOSURE_CHANGED = 19;

/**
 * Re-read a scene's status and push it to the engine.
 *
 * Exported so a caller can prime the canvas on load without waiting for an
 * event — the first paint should already carry bars.
 */
export async function refreshTokenStatus(
  worldStore: WorldStore,
  sceneId: string,
): Promise<void> {
  const statuses = await getTokenStatus(sceneId);

  for (const status of statuses) {
    const command: StatusCommand = {
      type: "set_token_status",
      tokenId: status.tokenId,
      resources: status.resources,
    };
    worldStore.dispatch(
      command,
      // `sync`, not `ui`: this is confirmed server state arriving, not a local
      // intent to be echoed back as a mutation. Dispatching it as `ui` would
      // put it through the offline queue, which once cost this codebase an
      // attribution it needed.
      "sync",
    );
  }
}

/**
 * Apply one world event, re-reading status when it might have changed.
 *
 * No-ops for every other event code, so it is safe to call on the shared
 * stream alongside the other appliers.
 */
export async function applyTokenStatusWorldEvent(
  worldStore: WorldStore,
  sceneId: string,
  event: WorldEventLike,
): Promise<void> {
  const eventCode = event.event_code ?? event.eventCode;
  if (
    eventCode !== EVENT_CODE_TOKEN_CHANGED &&
    eventCode !== EVENT_CODE_TOKEN_DISCLOSURE_CHANGED
  ) {
    return;
  }

  try {
    await refreshTokenStatus(worldStore, sceneId);
  } catch (error) {
    // A failed refresh leaves the previously drawn bars in place, which is
    // correct: the last known state is better than blanking a display that
    // was right a second ago. Silence would not be — a table wondering why
    // health stopped moving deserves something in the console.
    console.error("Failed to refresh token status:", error);
  }
}
