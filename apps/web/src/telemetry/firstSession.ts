/**
 * Spec 088's browser events (`contracts/telemetry.md`), against spec 086's
 * events table (`contracts/browser-events.md`).
 *
 * Every attribute is a bounded enum or a bucket: a link's use limit becomes
 * `1`, `2-5` or `6-50`, never the number; a world's starting map is
 * `default`, `other` or `none`, never its id. `telemetry` records nothing
 * until the telemetry chunk has loaded, and nothing at all with
 * `TELEMETRY=false`.
 */

import { telemetry } from "@thunderforge/telemetry";

export type FirstSessionEvent =
  | "world_link.created"
  | "world_link.revoked"
  | "world_link.join"
  | "world.created"
  | "rolls.cleared"
  | "map.imported"
  | "settings.saved"
  | "settings.unsaved_warning"
  | "hero.opened_from_players";

export type FirstSessionAttributes = Record<string, string | boolean>;

type Sink = (
  name: FirstSessionEvent,
  attributes: FirstSessionAttributes,
) => void;

const recordToTelemetry: Sink = (name, attributes) =>
  telemetry.event(name, { ...attributes });

let sink: Sink = recordToTelemetry;

/** For the tests: `undefined` puts the package's recording back. */
export function setFirstSessionSink(next: Sink | undefined): void {
  sink = next ?? recordToTelemetry;
}

function record(
  name: FirstSessionEvent,
  attributes: FirstSessionAttributes = {},
): void {
  sink(name, attributes);
}

/**
 * A link's use limit as its bucket. A link with no limit is `none`, which the
 * contract's buckets leave out: the panel offers "no limit", and leaving
 * those links uncounted would hide the most common choice.
 */
export function maxUsesBucket(limit: number | null): string {
  if (limit === null) return "none";
  if (limit <= 1) return "1";
  if (limit <= 5) return "2-5";
  return "6-50";
}

const EXPIRIES = new Set(["1d", "7d", "30d"]);

/** A link's expiry choice; "never", or anything unforeseen, is `none`. */
export function expiryBucket(expiry: string): string {
  return EXPIRIES.has(expiry) ? expiry : "none";
}

export function recordLinkCreated(limit: number | null, expiry: string): void {
  record("world_link.created", {
    max_uses: maxUsesBucket(limit),
    expiry: expiryBucket(expiry),
  });
}

export function recordLinkRevoked(): void {
  record("world_link.revoked");
}

export type JoinOutcome =
  | "joined"
  | "already_member"
  | "revoked"
  | "expired"
  | "used_up"
  | "unknown"
  | "signed_out";

const JOIN_BY_CODE: Record<string, JoinOutcome> = {
  ALREADY_MEMBER: "already_member",
  LINK_REVOKED: "revoked",
  LINK_EXPIRED: "expired",
  LINK_USED_UP: "used_up",
  LINK_UNKNOWN: "unknown",
};

/** A refusal's code as a join outcome; any other code is not counted. */
export function joinOutcomeOf(code: string): JoinOutcome | undefined {
  return JOIN_BY_CODE[code];
}

export function recordJoin(outcome: JoinOutcome): void {
  record("world_link.join", { outcome });
}

/**
 * The map a new world asked for: `undefined` and the instance's default id
 * are `default`, `null` (the GM chose none) is `none`, any other is `other`.
 */
export function startingMapBucket(
  chosen: string | null | undefined,
  defaultId: string | null,
): string {
  if (chosen === null) return "none";
  if (chosen === undefined || chosen === defaultId) return "default";
  return "other";
}

export function recordWorldCreated(
  chosen: string | null | undefined,
  defaultId: string | null,
  mapError: boolean,
): void {
  record("world.created", {
    starting_map: startingMapBucket(chosen, defaultId),
    map_error: mapError,
  });
}

export function recordRollsCleared(): void {
  record("rolls.cleared");
}

export function recordMapImported(
  source: "uvtt" | "image",
  wallEdges: boolean,
): void {
  record("map.imported", { source, wall_edges: wallEdges });
}

/** How many of a save's changes landed: `all`, `partial` or `none`. */
export function saveOutcome(saved: number, total: number): string {
  if (saved === total) return "all";
  return saved === 0 ? "none" : "partial";
}

export function changedBucket(total: number): string {
  if (total <= 1) return "1";
  return total <= 3 ? "2-3" : "4+";
}

/** A save of `total` changes with `saved` of them landed; none is no save. */
export function recordSettingsSaved(
  section: "mail",
  saved: number,
  total: number,
): void {
  if (total === 0) return;
  record("settings.saved", {
    section,
    outcome: saveOutcome(saved, total),
    changed: changedBucket(total),
  });
}

export function recordUnsavedWarning(left: boolean): void {
  record("settings.unsaved_warning", { action: left ? "left" : "stayed" });
}

export function recordHeroOpened(target: "sheet" | "builder"): void {
  record("hero.opened_from_players", { target });
}
