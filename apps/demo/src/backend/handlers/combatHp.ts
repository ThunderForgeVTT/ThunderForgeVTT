/**
 * Spec 079: hit points, and what each viewer may know of them.
 *
 * The arithmetic is the crate's (`readHitPoints`, `applyHitPoints`,
 * `writeHitPoints`, `standingAfter`); where the numbers live is the server's
 * (`combat/hit_points.rs`): a linked token's on its actor's sheet, a copy's on
 * the token itself. What a player is told is `tokenStatus`'s rule
 * (`graphql/queries/token_status.rs`): exact for a hero, a quarter for a
 * creature the Game Master runs.
 */
import { GraphQLError } from "graphql";
import system from "../../../../../packs/systems/dnd5e/system.json";
import { tokenVisible, viewerIsGm } from "../actors";
import { EVENT, now, record } from "../events";
import { markChanged, type DemoState, type Fight, type Row } from "../state";
import { MANIFEST, actorOf, call, slotsOf, type Rules } from "./combatRules";

export interface HitPoints {
  current: number;
  max: number;
  temporary: number;
}

/**
 * The record a token's hit points are read from: its actor's resources when
 * it is linked, its own when it is a copy. A copy made before there was a
 * fight starts from its actor's, as the server's placement copies them.
 */
export function resourcesOf(
  state: DemoState,
  fight: Fight | undefined,
  token: Row,
): Row | null {
  if (!token.actorId) return null;
  const sheet = (slotsOf(state, token.actorId).resourceData ??
    null) as Row | null;
  if (token.linked) return sheet;
  return fight?.copies[token.tokenId as string] ?? sheet;
}

export function readHitPoints(r: Rules, slot: Row): HitPoints {
  return call(() => r.readHitPoints(MANIFEST, JSON.stringify(slot)));
}

/** Whether `downedBy` came from hit points or from the Game Master. */
const DOWNED: Record<string, string> = {
  hit_points: "HIT_POINTS",
  game_master: "GAME_MASTER",
};
const STORED: Record<string, string> = {
  HIT_POINTS: "hit_points",
  GAME_MASTER: "game_master",
};

/**
 * `follow_zero`: a creature at 0 is out of the order, and one healed from 0
 * is back in it, unless the Game Master put it out by hand. True when a
 * combatant changed.
 */
function followCombatants(
  r: Rules,
  fight: Fight,
  token: Row,
  after: HitPoints,
): string | null {
  const combat = fight.combats.find((c) => !c.endedAt);
  if (!combat) return null;
  let changed = false;
  for (const combatant of fight.combatants) {
    if (combatant.combatId !== combat.id) continue;
    const isThis =
      combatant.tokenId === token.tokenId ||
      (combatant.tokenId == null &&
        token.linked === true &&
        combatant.actorId === token.actorId);
    if (!isThis) continue;
    const before = combatant.downedBy
      ? STORED[combatant.downedBy as string]
      : undefined;
    // Null when the change moves nobody in or out of the order.
    const standing = call<{ active: boolean; downedBy: string | null } | null>(
      () => r.standingAfter(after.current, combatant.active === true, before),
    );
    if (!standing) continue;
    const downedBy = standing.downedBy ? DOWNED[standing.downedBy] : null;
    if (
      standing.active !== combatant.active ||
      downedBy !== combatant.downedBy
    ) {
      combatant.active = standing.active;
      combatant.downedBy = downedBy;
      changed = true;
    }
  }
  return changed ? (combat.id as string) : null;
}

/**
 * `apply_hit_point_change`: one creature's hit points, changed and written
 * back where they live, with the events a server records.
 */
export function applyHitPointChange(
  r: Rules,
  state: DemoState,
  fight: Fight,
  token: Row,
  kind: "DAMAGE" | "HEALING",
  amount: number,
): HitPoints {
  if (amount < 0) {
    throw new GraphQLError("An amount of hit points cannot be negative");
  }
  const slot = resourcesOf(state, fight, token);
  if (!slot) throw new GraphQLError("That creature has no hit points recorded");
  const before = readHitPoints(r, slot);
  const after = call<HitPoints>(() =>
    r.applyHitPoints(
      JSON.stringify(before),
      kind === "DAMAGE" ? "Damage" : "Healing",
      amount,
    ),
  );
  const written = call<Row>(() =>
    r.writeHitPoints(MANIFEST, JSON.stringify(slot), JSON.stringify(after)),
  );

  if (token.linked) {
    const row = state.systemData.find((s) => s.actorId === token.actorId);
    if (!row)
      throw new GraphQLError("That creature has no hit points recorded");
    row.resourceData = written;
    row.updatedAt = now();
    record(EVENT.actorSheet, {
      action: "changed",
      actorId: token.actorId,
      dataType: "resource_data",
    });
  } else {
    fight.copies[token.tokenId as string] = written;
    record(EVENT.token, {
      action: "updated",
      token_id: token.tokenId,
      scene_id: token.sceneId,
    });
  }
  const combatId = followCombatants(r, fight, token, after);
  if (combatId) record(EVENT.combat, { combatId });
  markChanged();
  return after;
}

interface EntrySource {
  current: string;
  max?: string;
  maxValue?: number;
  label?: string;
  optional?: boolean;
}
interface ResourceDefinition {
  id: string;
  label: string;
  kind: string;
  source: { slot: string; entries: EntrySource[] };
}
const RESOURCES =
  (system as unknown as { resources?: ResourceDefinition[] }).resources ?? [];

/** `entries_from` (`resource_display.rs`). */
function entriesFrom(slot: Row, source: ResourceDefinition["source"]) {
  const read = (name: string | undefined) =>
    name !== undefined && Number.isInteger(slot[name])
      ? (slot[name] as number)
      : undefined;
  const built: Array<{
    current: number;
    max: number | null;
    label: string | null;
  }> = [];
  for (const entry of source.entries) {
    const current = read(entry.current);
    if (current === undefined) continue;
    if (entry.optional && current === 0) continue;
    built.push({
      current,
      max: read(entry.max) ?? entry.maxValue ?? null,
      label: entry.label ?? null,
    });
  }
  return built;
}

/** `proportion` and `quarter` (`resource_display.rs`). */
function proportionOf(entries: Array<{ current: number; max: number | null }>) {
  const max = entries.reduce((sum, e) => sum + (e.max ?? 0), 0);
  if (max <= 0) return null;
  const current = entries.reduce((sum, e) => sum + e.current, 0);
  return Math.min(Math.max(current, 0), max) / max;
}
function quarterOf(fraction: number | null): number {
  if (fraction === null || fraction <= 0) return 0;
  if (fraction >= 1) return 4;
  return Math.floor(fraction * 4);
}

/**
 * `tokenStatus(sceneId)`. The Game Master is told every figure; a player is
 * told a hero's exactly and a creature's as a quarter, with no maximum and no
 * figure on the wire (`default_disclosure`). The demo has no per-token
 * disclosure overrides, so the default is the answer.
 */
export function tokenStatus(state: DemoState, sceneId: string): Row[] {
  const gm = viewerIsGm(state);
  const out: Row[] = [];
  for (const token of state.tokens) {
    if (token.sceneId !== sceneId || !tokenVisible(state, token)) continue;
    const actor = actorOf(state, token);
    if (!actor) continue;
    const resources: Row[] = [];
    for (const definition of RESOURCES) {
      const slot =
        definition.source.slot === "resourceData"
          ? resourcesOf(state, state.fight, token)
          : token.linked
            ? ((slotsOf(state, actor.id)[definition.source.slot] ??
                null) as Row | null)
            : null;
      if (!slot) continue;
      const entries = entriesFrom(slot, definition.source);
      if (entries.length === 0) continue;
      const npc = actor.isNpc !== false;
      const configured = npc ? "chunked" : "visible";
      const disclosure = gm ? "visible" : configured;
      resources.push({
        definitionId: definition.id,
        label: definition.label,
        kind: definition.kind,
        disclosure,
        entries: disclosure === "visible" ? entries : null,
        proportion: null,
        quarter:
          disclosure === "chunked" ? quarterOf(proportionOf(entries)) : null,
        configured: gm ? configured : null,
      });
    }
    if (resources.length > 0) out.push({ tokenId: token.tokenId, resources });
  }
  return out;
}
