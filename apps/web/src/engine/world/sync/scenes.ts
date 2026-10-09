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

/** `crates/thunderforge-server/src/world_events.rs::EVENT_CODE_SCENE_LAUNCHED`. */
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

/** `crates/thunderforge-server/src/world_events.rs::EVENT_CODE_MAP_IMPORTED`. */
const MAP_IMPORTED_EVENT_CODE = 13;

/**
 * Returns the scene id a map was just imported into, or `null` for any other
 * event.
 *
 * A map import writes walls, doors, lights and the scene's art straight to
 * Postgres in one request and announces all of it with this one event — no
 * per-wall or per-light events follow. So a client that is not the importer
 * learns about the new map only here, and has to re-read the scene's content
 * itself.
 */
export function parseMapImportedEvent(event: WorldEventLike): string | null {
  const eventCode = event.event_code ?? event.eventCode;
  if (eventCode !== MAP_IMPORTED_EVENT_CODE) {
    return null;
  }

  const payload = (event.token_event ?? event.tokenEvent) as
    | { sceneId?: string; scene_id?: string }
    | undefined;

  return payload?.scene_id ?? payload?.sceneId ?? null;
}
