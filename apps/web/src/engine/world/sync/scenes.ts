/**
 * scenes.ts
 * Spec 022 (FR-002a/FR-002b, ADR-046): the "which scene is currently
 * launched" half of live world-event sync. Unlike walls/tokens/lights/
 * shapes (which mutate world-store content for an already-selected
 * scene), a scene-launch event changes *which scene is selected* — so
 * this doesn't dispatch into `WorldStore` at all; it just tells the
 * caller (`WorldPage.tsx`) the newly-active scene id so it can update its
 * own `selectedSceneId` state, which every existing scene-content loader
 * effect already reacts to.
 */

import type { WorldEventLike } from "./subscriptionClient";

/** `src/server/src/world_events.rs::EVENT_CODE_SCENE_LAUNCHED`. */
const SCENE_LAUNCHED_EVENT_CODE = 16;

/**
 * Returns the newly-launched scene id if `event` is a scene-launch event,
 * or `null` if it's a different event code (the caller should ignore it).
 */
export function parseSceneLaunchedEvent(event: WorldEventLike): string | null {
  const eventCode = event.event_code ?? event.eventCode;
  if (eventCode !== SCENE_LAUNCHED_EVENT_CODE) {
    return null;
  }

  const payload = (event.token_event ?? event.tokenEvent) as
    | { sceneId?: string; scene_id?: string }
    | undefined;

  return payload?.sceneId ?? payload?.scene_id ?? null;
}
