/**
 * sceneTime.ts — a scene's time of day, live (spec 099 US5; `world_events`
 * code 43, see `crates/thunderforge-server/src/world_events.rs`).
 *
 * A Game Master sets the hour, at once or over a few seconds, and every
 * client in the scene hands it to the engine's clock
 * (`plugins/time_of_day.rs`), which eases the darkness toward it. A scene
 * with no hour has no clock, and its light is its ambient level as before.
 *
 * The payload carries the hour and the transition, so every client runs the
 * same transition rather than jumping to the stored hour.
 */
import type { WorldStore } from "../store";

export const SCENE_TIME_CHANGED_EVENT_CODE = 43;

/** An hour as stored or received: a finite number in 0..24, or none. */
export function asHour(value: unknown): number | null {
  return typeof value === "number" &&
    Number.isFinite(value) &&
    value >= 0 &&
    value <= 24
    ? value % 24
    : null;
}

/** Tell the engine the hour; `overSecs` passes time, without it the hour jumps. */
export function setSceneTime(
  worldStore: WorldStore,
  hour: number | null,
  overSecs?: number,
): void {
  worldStore.dispatch(
    overSecs && overSecs > 0 && hour !== null
      ? { type: "set_time_of_day", hour, overSecs }
      : { type: "set_time_of_day", hour },
    "sync",
  );
}

type WorldEventLike = {
  event_code?: number;
  eventCode?: number;
  token_event?: unknown;
  tokenEvent?: unknown;
};

export type SceneTimeChange = { hour: number | null; overSecs: number };

/**
 * Apply a code-43 event if it is about `sceneId`. Returns what it applied,
 * or `null` for any other event, or one about a scene this client is not
 * showing.
 */
export function applySceneTimeWorldEvent(
  worldStore: WorldStore,
  sceneId: string,
  event: WorldEventLike,
): SceneTimeChange | null {
  const eventCode = event.event_code ?? event.eventCode;
  if (eventCode !== SCENE_TIME_CHANGED_EVENT_CODE) {
    return null;
  }
  const payload = (event.token_event ?? event.tokenEvent) as
    | Record<string, unknown>
    | undefined;
  if (payload?.sceneId !== sceneId) {
    return null;
  }
  const hour = asHour(payload.hour);
  const over = payload.overSecs;
  const overSecs =
    typeof over === "number" && Number.isFinite(over) && over > 0 ? over : 0;
  setSceneTime(worldStore, hour, overSecs);
  return { hour, overSecs };
}
