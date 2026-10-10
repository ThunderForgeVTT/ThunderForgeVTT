/**
 * Spec 086 US1: what the demo's backend tells telemetry.
 *
 * `record()` in `events.ts` is the one place every accepted change passes,
 * so the funnel's `token_moved` and `dice_rolled` and the `demo.action`
 * counts are read off it here, and no handler needs to know. Only the kind
 * of thing that changed is sent: never a name, a message, a position or an
 * identifier.
 */
import {
  telemetry,
  type Attrs,
  type FunnelStep,
} from "@thunderforge/telemetry";
import type { Row } from "./state";

/** The `demo.action` kinds (contracts/browser-events.md). */
export type DemoAction =
  | "token_moved"
  | "wall_drawn"
  | "door_toggled"
  | "light_placed"
  | "shape_drawn"
  | "dice_rolled"
  | "scene_changed"
  | "map_imported"
  | "start_over"
  | "view_switched";

/** What the tap sends through. The app's telemetry, or a test's. */
export interface TapSink {
  event(name: "demo.action" | "demo.not_in_demo", attrs: Attrs): void;
  funnel(step: FunnelStep): void;
}

let sink: TapSink = telemetry;

/** For a test: where the tap sends. Returns the previous sink. */
export function setTapSink(next: TapSink): TapSink {
  const previous = sink;
  sink = next;
  return previous;
}

/** `crates/thunderforge-server/src/world_events.rs`, the codes read here. */
const CODE = {
  wall: 10,
  light: 11,
  shape: 12,
  mapImported: 13,
  token: 14,
  sceneLaunched: 16,
  door: 21,
  tokenTravelled: 34,
  rollMade: 36,
} as const;

/**
 * The action an event announces, or `null` for one the funnel does not
 * count. A token that arrives with the party, or is created or removed, is
 * not a move; a door is its own event, not a wall.
 */
export function actionOf(eventCode: number, payload: Row): DemoAction | null {
  const action = payload.action;
  switch (eventCode) {
    case CODE.token:
      return action === "updated" ? "token_moved" : null;
    case CODE.tokenTravelled:
      return "token_moved";
    case CODE.wall:
      return action === "created" ? "wall_drawn" : null;
    case CODE.door:
      return "door_toggled";
    case CODE.light:
      return action === "created" ? "light_placed" : null;
    case CODE.shape:
      return action === "created" ? "shape_drawn" : null;
    case CODE.rollMade:
      return "dice_rolled";
    case CODE.sceneLaunched:
      return "scene_changed";
    case CODE.mapImported:
      return "map_imported";
    default:
      return null;
  }
}

const FUNNEL: Partial<Record<DemoAction, FunnelStep>> = {
  token_moved: "token_moved",
  dice_rolled: "dice_rolled",
  view_switched: "view_switched",
};

/** One accepted action: counted, and a funnel step if it is one. */
export function countAction(action: DemoAction): void {
  try {
    sink.event("demo.action", { action });
    const step = FUNNEL[action];
    if (step) sink.funnel(step);
  } catch {
    // Telemetry never stops the world from changing.
  }
}

/** Called from `record()` with every event the backend announces. */
export function tapEvent(eventCode: number, payload: Row): void {
  const action = actionOf(eventCode, payload);
  if (action) countAction(action);
}

/** A schema field name, or `unknown` for anything else that was refused. */
export function refusedRootField(what: string): string {
  return /^[a-z][A-Za-z0-9]{0,63}$/.test(what) ? what : "unknown";
}

/** `reportNotInDemo`'s half: what was refused, as a field name at most. */
export function tapNotInDemo(what: string): void {
  try {
    sink.event("demo.not_in_demo", { root_field: refusedRootField(what) });
  } catch {
    // As above.
  }
}
