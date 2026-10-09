/**
 * World events, as the server's pub/sub backplane would deliver them.
 *
 * A handler records what changed; the guard releases the events once the
 * mutation's own answer is on its way, so a subscriber sees them in the order
 * a server gives: acknowledged, then told (spec 074 FR-008).
 */
import { demoState, type Row } from "./state";
import { DEMO_USER } from "../seed/world";
import { tapEvent } from "./telemetryTap";

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
  rollMade: 36,
  rollRevealed: 37,
  rollsCleared: 39,
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
  // Spec 086: the kind of change, counted; never what it was.
  tapEvent(eventCode, payload);
}

/**
 * `worldEventsSince`, as the server answers it: what came after `afterId`,
 * oldest first, at most a page of it. `truncated` says the gap was more than
 * one page — or older than this page remembers — and the client must
 * resynchronise rather than apply what it was given.
 */
export function eventsSince(afterId: number): Row {
  const latestId = demoState().nextEventId - 1;
  const isGm = viewerIsGm();
  const after = log.filter(
    (event) => event.id > afterId && eventReaches(event, isGm),
  );
  const oldestKept = log[0]?.id ?? latestId + 1;
  const forgotten = afterId + 1 < oldestKept && afterId < latestId;
  return {
    events: after.slice(0, CATCH_UP_LIMIT),
    truncated: forgotten || after.length > CATCH_UP_LIMIT,
    latestId: Math.max(latestId, 0),
  };
}

/**
 * `roll_event_reaches`: a GM only roll is never delivered to a player at all
 * (spec 081 FR-005a). Every other event reaches every viewer; a reveal is
 * for the whole table.
 */
export function eventReaches(event: WorldEvent, isGm: boolean): boolean {
  if (isGm || event.eventCode !== EVENT.rollMade) return true;
  // No visibility reads as the most hidden, as `Visibility::parse` does.
  const visibility = (event.tokenEvent as Row | null)?.visibility;
  return visibility === "everyone" || visibility === "gm_eyes";
}

/** Whether the operation in flight is the GM's, as `viewerIsGm` judges it. */
const viewerIsGm = () => demoState().viewer !== "player";

/** Hands every recorded event to every subscriber, oldest first. */
export function releaseEvents(): void {
  const events = pending;
  pending = [];
  for (const event of events) deliverEvent(event);
}

/**
 * Hands one event to every subscriber: one this tab released, or one the
 * tab holding the world released and posted (spec 081 R6).
 */
export function deliverEvent(event: WorldEvent): void {
  for (const deliver of [...subscribers]) deliver(event);
}

export function subscribeToEvents(
  deliver: (event: WorldEvent) => void,
): () => void {
  subscribers.add(deliver);
  return () => subscribers.delete(deliver);
}
