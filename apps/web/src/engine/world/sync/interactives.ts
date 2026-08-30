import {
  activateInteractive,
  getInteractives,
  type ActivationResult,
  type Interactive,
} from "@/api/interactives";
import type { WorldStore } from "../store";
import type { WorldInteractive } from "../types";
import type { WorldEventLike } from "./subscriptionClient";
import { onInteractionTriggered } from "@/engine/bevy";

/**
 * Getting interactives onto the canvas, and keeping them current.
 *
 * Spec 030. Mirrors the wall/light/status sync path: a world event says
 * *something changed*, and this re-reads the scene's interactives and
 * dispatches them.
 *
 * # Why it re-reads rather than applying a delta
 *
 * The same reason status does. What a viewer receives depends on the viewer —
 * a Game Master gets the effect and its configuration, a player gets neither —
 * so a delta carrying the payload would have to be reduced per subscriber
 * before it left the server. Re-reading means one resolver produces the answer
 * every time.
 */

/** An interactive was authored, edited, deleted, reset or activated. */
const EVENT_CODE_INTERACTIVE_CHANGED = 20;

/** A door's state, lock, secrecy or designation changed. */
const EVENT_CODE_DOOR_CHANGED = 21;

function toWorldInteractive(interactive: Interactive): WorldInteractive {
  return {
    id: interactive.interactiveId,
    subjectKind: interactive.subjectKind,
    subjectRef: interactive.subjectRef,
    geometry: interactive.geometry,
    effectId: interactive.effectId,
    effectConfig: interactive.effectConfig,
    trigger: interactive.trigger,
    canActivate: interactive.canActivate,
    fireMode: interactive.fireMode,
    fired: interactive.firedAt !== null,
  };
}

/**
 * Re-read a scene's interactives and push them to the engine.
 *
 * Exported so a caller can prime the canvas on load rather than waiting for an
 * event — a book should be clickable on the first paint.
 */
export async function refreshInteractives(
  worldStore: WorldStore,
  sceneId: string,
): Promise<void> {
  const interactives = await getInteractives(sceneId);
  for (const interactive of interactives) {
    worldStore.dispatch(
      {
        type: "upsert_interactive",
        interactive: toWorldInteractive(interactive),
      },
      // `sync`, not `ui`: confirmed server state arriving, not a local intent
      // to be echoed back as a mutation. Dispatching it as `ui` would put it
      // through the offline queue.
      "sync",
    );
  }
}

/**
 * Apply one world event, re-reading interactives when they might have changed.
 *
 * No-ops for every other event code, so it is safe to call on the shared
 * stream alongside the other appliers.
 */
export async function applyInteractiveWorldEvent(
  worldStore: WorldStore,
  sceneId: string,
  event: WorldEventLike,
): Promise<void> {
  const eventCode = event.event_code ?? event.eventCode;
  if (
    eventCode !== EVENT_CODE_INTERACTIVE_CHANGED &&
    eventCode !== EVENT_CODE_DOOR_CHANGED
  ) {
    return;
  }

  try {
    await refreshInteractives(worldStore, sceneId);
  } catch (error) {
    // A failed refresh leaves what was drawn in place: the last known state is
    // better than a canvas that quietly stops responding. Silence would not
    // be — a table wondering why a lever stopped working deserves something in
    // the console.
    console.error("Failed to refresh interactives:", error);
  }
}

/**
 * Ask the server, and apply what it permitted.
 *
 * The whole permission decision happens in the mutation. This function does
 * not check anything before calling and does not interpret the answer beyond
 * dispatching a `performed` one into the engine — which is the point: a client
 * that decided for itself would be a second opinion, and the one that drifts
 * is always the one people believe.
 */
export async function activateAndApply(
  worldStore: WorldStore,
  interactiveId: string,
): Promise<ActivationResult> {
  const result = await activateInteractive(interactiveId);

  if (result.outcome === "performed" && result.effectId) {
    worldStore.dispatch(
      {
        type: "dispatch_interaction",
        interactiveId,
        effectId: result.effectId,
        effectConfig: result.effectConfig,
      },
      "sync",
    );
  }

  return result;
}

/**
 * Tell the engine whether the scene is in play.
 *
 * Separate from anything else because it cannot be inferred: preparing a scene
 * and running one look identical from the outside (FR-032).
 */
export function setScenePlaying(
  worldStore: WorldStore,
  playing: boolean,
): void {
  worldStore.dispatch({ type: "set_scene_playing", playing }, "sync");
}

/**
 * Turn engine-detected triggers into the same activation a click makes.
 *
 * Spec 030, US5. The engine notices a token crossed into a region and reports
 * it; this asks the server, which decides. That is one path rather than two:
 * whether a crossing is permitted, whether a `once` has already spent itself,
 * and whether it needs approval are the same questions a click raises, and
 * answering them twice in two places is how the two answers drift.
 *
 * Returns a function that stops listening.
 */
export function startTriggerBridge(
  worldStore: WorldStore,
  onOutcome?: (result: ActivationResult) => void,
): () => void {
  return onInteractionTriggered((event) => {
    void activateAndApply(worldStore, event.interactiveId)
      .then((result) => onOutcome?.(result))
      .catch((error: unknown) => {
        // A crossing the server refused to answer at all. Left in the console
        // rather than surfaced: a player who walked somewhere did not ask for
        // anything, and telling them a request failed would be reporting an
        // error for an action they never took.
        console.error("Failed to resolve a triggered interaction:", error);
      });
  });
}
