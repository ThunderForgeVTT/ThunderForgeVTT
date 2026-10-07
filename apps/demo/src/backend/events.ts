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
  combat: 18,
  door: 21,
  sceneLighting: 25,
  actorSheet: 26,
  explorationReset: 27,
  attack: 29,
  offer: 30,
  sceneLevel: 33,
  tokenTravelled: 34,
} as const;

const subscribers = new Set<(event: WorldEvent) => void>();
let pending: WorldEvent[] = [];

/**
 * What `worldEventsSince` answers from: the events this page has recorded,
 * newest last. A reload starts the client afresh from the saved world, so the
 * log need only outlive a dropped subscription, not the page.
 */
const LOG_LIMIT = 1000;
const log: WorldEvent[] = [];

/** The server's page size for a catch-up. */
const CATCH_UP_LIMIT = 200;

export function now(): string {
  // The server's `NaiveDateTime`: no zone, microseconds.
  return `${new Date().toISOString().slice(0, -1)}000`;
}

export function record(eventCode: number, payload: Row): void {
  const state = demoState();
  const at = now();
  const event: WorldEvent = {
    id: state.nextEventId++,
    worldId: state.world.id,
    eventCode,
    tokenEvent: payload,
    createdBy: DEMO_USER.id,
    updatedBy: DEMO_USER.id,
    schemaVersion: 1,
    createdAt: at,
    updatedAt: at,
  };
  pending.push(event);
  log.push(event);
  if (log.length > LOG_LIMIT) log.splice(0, log.length - LOG_LIMIT);
}

/**
 * `worldEventsSince`, as the server answers it: what came after `afterId`,
 * oldest first, at most a page of it. `truncated` says the gap was more than
 * one page — or older than this page remembers — and the client must
 * resynchronise rather than apply what it was given.
 */
export function eventsSince(afterId: number): Row {
  const latestId = demoState().nextEventId - 1;
  const after = log.filter((event) => event.id > afterId);
  const oldestKept = log[0]?.id ?? latestId + 1;
  const forgotten = afterId + 1 < oldestKept && afterId < latestId;
  return {
    events: after.slice(0, CATCH_UP_LIMIT),
    truncated: forgotten || after.length > CATCH_UP_LIMIT,
    latestId: Math.max(latestId, 0),
  };
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
