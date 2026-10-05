/**
 * World events, as the server's pub/sub backplane would deliver them.
 *
 * A handler records what changed; the guard releases the events once the
 * mutation's own answer is on its way, so a subscriber sees them in the order
 * a server gives: acknowledged, then told (spec 074 FR-008).
 */
import { demoState, type Row } from "./state";
import { DEMO_USER } from "../seed/world";

export type WorldEvent = Row & { id: number; eventCode: number };

/** `crates/thunderforge-server/src/world_events.rs`. */
export const EVENT = {
  wall: 10,
  light: 11,
  shape: 12,
  token: 14,
  sceneLaunched: 16,
  chat: 17,
  door: 21,
} as const;

const subscribers = new Set<(event: WorldEvent) => void>();
let pending: WorldEvent[] = [];

export function now(): string {
  // The server's `NaiveDateTime`: no zone, microseconds.
  return `${new Date().toISOString().slice(0, -1)}000`;
}

export function record(eventCode: number, payload: Row): void {
  const state = demoState();
  const at = now();
  pending.push({
    id: state.nextEventId++,
    worldId: state.world.id,
    eventCode,
    tokenEvent: payload,
    createdBy: DEMO_USER.id,
    updatedBy: DEMO_USER.id,
    schemaVersion: 1,
    createdAt: at,
    updatedAt: at,
  });
}

/** Hands every recorded event to every subscriber, oldest first. */
export function releaseEvents(): void {
  const events = pending;
  pending = [];
  for (const event of events) {
    for (const deliver of [...subscribers]) deliver(event);
  }
}

export function subscribeToEvents(
  deliver: (event: WorldEvent) => void,
): () => void {
  subscribers.add(deliver);
  return () => subscribers.delete(deliver);
}
