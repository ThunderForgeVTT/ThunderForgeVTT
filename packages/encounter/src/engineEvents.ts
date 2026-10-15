/**
 * Spec 091: the engine's events the room listens to, parsed.
 *
 * The engine hands its event callback a JSON string. This turns the five
 * kinds the room uses into typed events, and everything else into null. The
 * shapes follow `apps/web/src/engine/bevy/engineEvents.ts`'s guards, but each
 * field is checked here, since a malformed event must not move the hero.
 */

import type { Point } from "./walls.ts";

/** A token moved: `emit_token_move_along` (`token_move.rs`). */
export interface TokenMovedEvent {
  type: "upsert_token";
  token: { id: string; x: number; y: number };
  /** The route walked, for a committed Shift route. */
  path?: Point[];
}

/** A move refused at a wall: `refuse_at_wall` (`token_move.rs`). It names no token. */
export interface MovementBlockedEvent {
  type: "movement_blocked";
  /** The token refused; absent from an engine built before it was named. */
  tokenId?: string;
  wallId: string;
  at: Point;
}

/** A door's icon pressed (`door_icons.rs`). */
export interface DoorIconPressedEvent {
  type: "door_icon_pressed";
  wallId: string;
  icon: "open" | "close" | "padlock";
}

/** A region entered: `detect_entries` (`interaction.rs`). */
export interface InteractionTriggeredEvent {
  type: "interactionTriggered";
  /** The token that crossed in; absent from an engine built before it was named. */
  tokenId?: string;
  interactiveId: string;
  trigger: "enter";
}

/** A command the engine could not take (`sdk.rs`). */
export interface SdkErrorEvent {
  type: "sdkError";
  code: string;
  message: string;
  command: string | null;
}

/** An immediate `fit_camera_to` took effect, in the update that draws it. */
export interface CameraFittedEvent {
  type: "cameraFitted";
  centerX: number;
  centerY: number;
}

export type EngineEvent =
  | CameraFittedEvent
  | TokenMovedEvent
  | MovementBlockedEvent
  | DoorIconPressedEvent
  | InteractionTriggeredEvent
  | SdkErrorEvent;

type Fields = Record<string, unknown>;

const isObject = (value: unknown): value is Fields =>
  typeof value === "object" && value !== null && !Array.isArray(value);

const isNumber = (value: unknown): value is number =>
  typeof value === "number" && Number.isFinite(value);

const isString = (value: unknown): value is string => typeof value === "string";

const asPoint = (value: unknown): Point | null =>
  isObject(value) && isNumber(value.x) && isNumber(value.y)
    ? { x: value.x, y: value.y }
    : null;

const ICONS: readonly string[] = ["open", "close", "padlock"];

function tokenMoved(event: Fields): TokenMovedEvent | null {
  const token = event.token;
  if (
    !isObject(token) ||
    !isString(token.id) ||
    !isNumber(token.x) ||
    !isNumber(token.y)
  )
    return null;
  const moved: TokenMovedEvent = {
    type: "upsert_token",
    token: { id: token.id, x: token.x, y: token.y },
  };
  if (event.path === undefined) return moved;
  if (!Array.isArray(event.path)) return null;
  const path = event.path.map(asPoint);
  if (path.some((p) => p === null)) return null;
  return { ...moved, path: path as Point[] };
}

function movementBlocked(event: Fields): MovementBlockedEvent | null {
  const at = asPoint(event.at);
  if (!isString(event.wallId) || !at) return null;
  const blocked: MovementBlockedEvent = {
    type: "movement_blocked",
    wallId: event.wallId,
    at,
  };
  if (isString(event.tokenId)) blocked.tokenId = event.tokenId;
  return blocked;
}

function doorIconPressed(event: Fields): DoorIconPressedEvent | null {
  return isString(event.wallId) &&
    isString(event.icon) &&
    ICONS.includes(event.icon)
    ? {
        type: "door_icon_pressed",
        wallId: event.wallId,
        icon: event.icon as DoorIconPressedEvent["icon"],
      }
    : null;
}

function interactionTriggered(event: Fields): InteractionTriggeredEvent | null {
  if (!isString(event.interactiveId) || event.trigger !== "enter") return null;
  const entered: InteractionTriggeredEvent = {
    type: "interactionTriggered",
    interactiveId: event.interactiveId,
    trigger: "enter",
  };
  if (isString(event.tokenId)) entered.tokenId = event.tokenId;
  return entered;
}

function sdkError(event: Fields): SdkErrorEvent | null {
  const command = event.command ?? null;
  if (
    !isString(event.code) ||
    !isString(event.message) ||
    !(command === null || isString(command))
  )
    return null;
  return {
    type: "sdkError",
    code: event.code,
    message: event.message,
    command,
  };
}

/** One engine event string, typed; null for any other kind and anything malformed. */
function cameraFitted(event: Fields): CameraFittedEvent | null {
  if (!isNumber(event.centerX) || !isNumber(event.centerY)) return null;
  return { type: "cameraFitted", centerX: event.centerX, centerY: event.centerY };
}

export function parseEngineEvent(raw: string): EngineEvent | null {
  let event: unknown;
  try {
    event = JSON.parse(raw);
  } catch {
    return null;
  }
  if (!isObject(event)) return null;
  switch (event.type) {
    case "upsert_token":
      return tokenMoved(event);
    case "movement_blocked":
      return movementBlocked(event);
    case "door_icon_pressed":
      return doorIconPressed(event);
    case "interactionTriggered":
      return interactionTriggered(event);
    case "sdkError":
      return sdkError(event);
    case "cameraFitted":
      return cameraFitted(event);
    default:
      return null;
  }
}
