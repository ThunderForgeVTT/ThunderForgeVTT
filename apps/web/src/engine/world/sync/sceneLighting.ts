/**
 * sceneLighting.ts — a scene's ambient light, live (playtest 2026-09-10 P9;
 * `world_events` code 25, see `src/server/src/world_events.rs`).
 *
 * The Game Master sets a scene bright, dim or dark, and every client in that
 * scene hands the level to the engine, which draws its darkness layer from it
 * (`plugins/darkness.rs`). Before this nothing in the product told the engine
 * anything, so every scene rendered in permanent daylight and no wall ever
 * cast a shadow.
 *
 * The payload carries the level itself rather than asking for a re-read: a
 * scene's light is no secret, and everyone in the scene is about to see it.
 */
import type { WorldStore } from "../store";

export const SCENE_LIGHTING_CHANGED_EVENT_CODE = 25;

export type AmbientLevel = "bright" | "dim" | "dark";

export const AMBIENT_LEVELS: readonly AmbientLevel[] = [
  "bright",
  "dim",
  "dark",
];

/** A stored or received level, read defensively: anything unknown is bright,
 * as the engine itself treats it — a typo must not black out a scene. */
export function asAmbientLevel(value: unknown): AmbientLevel {
  return value === "dim" || value === "dark" ? value : "bright";
}

/** Tell the engine how lit the scene is. */
export function setSceneAmbient(
  worldStore: WorldStore,
  level: AmbientLevel,
): void {
  worldStore.dispatch({ type: "set_ambient_light", level }, "sync");
}

type WorldEventLike = {
  event_code?: number;
  eventCode?: number;
  token_event?: unknown;
  tokenEvent?: unknown;
};

/**
 * Apply a code-25 event if it is about `sceneId`. Returns the level it
 * applied, or `null` for any other event — including one about a scene this
 * client is not showing, whose light is not this canvas's business.
 */
export function applySceneLightingWorldEvent(
  worldStore: WorldStore,
  sceneId: string,
  event: WorldEventLike,
): AmbientLevel | null {
  const eventCode = event.event_code ?? event.eventCode;
  if (eventCode !== SCENE_LIGHTING_CHANGED_EVENT_CODE) {
    return null;
  }
  const payload = (event.token_event ?? event.tokenEvent) as
    | Record<string, unknown>
    | undefined;
  if (payload?.sceneId !== sceneId) {
    return null;
  }
  const level = asAmbientLevel(payload.ambientLight);
  setSceneAmbient(worldStore, level);
  return level;
}
