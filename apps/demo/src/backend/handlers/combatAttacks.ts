/**
 * Spec 079: attacks and offers, as the server makes and tells them.
 *
 * `makeAttack` and `previewAttack` follow `combat/attack.rs` step for step:
 * whose turn it is, the weapon, reach and range, the budget spent once for
 * the whole attack, to-hit against the target's defence, damage only on a
 * hit, and an offer of that damage which applies itself when the encounter
 * says so. What each viewer is told of an attack is
 * `graphql/types_attacks.rs`'s `build_attacks`, judged by
 * `combat/redaction.rs` — see `mayKnow` below for the one place the demo
 * judges less than the server does.
 */
import { GraphQLError } from "graphql";
import system from "../../../../../packs/systems/dnd5e/system.json";
import { viewerIsGm } from "../actors";
import { EVENT, now, record } from "../events";
import {
  demoState,
  markChanged,
  type DemoState,
  type Fight,
  type Row,
} from "../state";
import { DEMO_PLAYER } from "../../seed/world";
import { abilityFor, actorAbilities, worldAbilities } from "./combatAbilities";
import { applyHitPointChange } from "./combatHp";
import { recordRoll } from "./dice";
import { shapeD20, shapeDamage, type Advantage } from "./facets";
import {
  MANIFEST,
  actorOf,
  budgetOf,
  call,
  controls,
  dice,
  enumOf,
  fightOf,
  nameReadable,
  playerControls,
  resolutionRow,
  rules,
  slotsOf,
  tokenById,
  tokenLabel,
  type Args,
  type Rules,
  type WasmResolution,
} from "./combatRules";

type Handler = (args: Args) => unknown;

/** `combat/attack.rs` and `combat/offers.rs`, word for word. */
const NOT_CONTROLLED = "You do not control that creature";
const NOT_ON_BOARD = "That creature is not on the board";
const OFFER_NOT_THERE = "That offer is not there";
const ALREADY_RESOLVED = "That offer has already been resolved";
const RELINKED_SINCE =
  "That creature was relinked after this offer was made, so its hit points are a different record now. Decline the offer, and change its hit points by hand if the hit should stand.";
const LAIR_NOT_THERE = "That lair is not in a running encounter";
const LAIR_NO_ADVANTAGE = "A lair does not roll with advantage.";
/** `rolls::reroll`'s refusals that an attack's reroll can meet. */
const NOT_A_D20_TEST = "Only a d20 test can be rerolled.";
const A_HIT = "A hit cannot be rerolled.";

/** `queries/attacks.rs`: a page of a scene's attacks. */
const SCENE_ATTACKS_PAGE = 50;

const DEFENCE = (
  system as unknown as { combat: { defence: { slot: string; field: string } } }
).combat.defence;

/** The schema's `ActionCost` as the crate's `Spend::for_attack` reads it. */
const COST: Record<string, string> = {
  ACTION: "action",
  BONUS_ACTION: "bonusAction",
  REACTION: "reaction",
  LEGENDARY: "legendary",
  FREE: "free",
};

const UNKNOWN = "Unknown";

// ---------------------------------------------------------------------------
// Where everything is.

/** `running_combat`: the encounter on this scene, or on none in particular. */
function combatOn(fight: Fight | undefined, sceneId: unknown): Row | undefined {
  return fight?.combats.find(
    (c) => !c.endedAt && (c.sceneId == null || c.sceneId === sceneId),
  );
}

/** `combatant_of_token`: by its token, else the one seat its actor holds. */
function seatOf(fight: Fight, combat: Row, token: Row): Row | undefined {
  const seats = fight.combatants.filter((c) => c.combatId === combat.id);
  const byToken = seats.find((c) => c.tokenId === token.tokenId);
  if (byToken) return byToken;
  const byActor = seats.filter(
    (c) =>
      c.kind !== "LAIR" &&
      c.tokenId == null &&
      token.actorId != null &&
      c.actorId === token.actorId,
  );
  return byActor.length === 1 ? byActor[0] : undefined;
}

/** `turn_check`: whether it is this creature's turn, for anyone but the GM. */
function turnCheck(r: Rules, state: DemoState, token: Row): Row {
  const allowed = { allowed: true, activeLabel: null };
  if (viewerIsGm(state)) return allowed;
  const combat = combatOn(state.fight, token.sceneId);
  if (!combat?.activeCombatantId || !state.fight) return allowed;
  const seats = state.fight.combatants
    .filter((c) => c.combatId === combat.id)
    .map((c) => ({
      id: c.id,
      tokenId: c.tokenId,
      actorId: c.actorId,
      active: c.active,
      label: c.label,
    }));
  const holder = r.heldBy(
    JSON.stringify(seats),
    combat.activeCombatantId as string,
    token.tokenId as string,
    (token.actorId as string | null) ?? undefined,
  );
  if (holder === undefined) return allowed;
  const active = state.fight.combatants.find(
    (c) => c.id === combat.activeCombatantId,
  );
  const hidden =
    active?.tokenId != null &&
    !nameReadable(state, tokenById(state, active.tokenId as string));
  return { allowed: false, activeLabel: hidden ? UNKNOWN : holder };
}

function refusalOf(r: Rules, check: Row): string | null {
  return check.allowed
    ? null
    : r.turnRefusal((check.activeLabel as string) ?? UNKNOWN);
}

// ---------------------------------------------------------------------------
// Reach and range.

interface Reach {
  reach: number | null;
  rangeNormal: number | null;
  rangeLong: number | null;
  needsLineOfSight: boolean;
}

/** `reach::is_melee`: a reach, and a target measured within it. */
export function isMelee(distance: number | null, reach: Reach): boolean {
  return (
    distance !== null && reach.reach !== null && distance <= reach.reach + 1e-6
  );
}

/**
 * The damage formulas shaped as `roll_hit_damage` shapes them (spec 084
 * R6). The demo has no items, so a weapon's `properties` are the row's own.
 */
export function shapedDamage(
  formulas: string[],
  traitData: unknown,
  melee: boolean,
  properties: readonly string[],
): { formulas: string[]; facets: string[] } {
  const shaped = formulas.map((f) =>
    shapeDamage(f, "NORMAL", { traitData, melee, properties }),
  );
  return {
    formulas: shaped.map((x) => x.formula),
    facets: [...new Set(shaped.flatMap((x) => x.facets))],
  };
}

function reachOf(ability: Row): Reach {
  return {
    reach: (ability.reach as number | null) ?? null,
    rangeNormal: (ability.rangeNormal as number | null) ?? null,
    rangeLong: (ability.rangeLong as number | null) ?? null,
    needsLineOfSight: ability.needsLineOfSight === true,
  };
}

/** `SceneMeasure::measure`, by the crate's `measure`. */
function measure(
  r: Rules,
  state: DemoState,
  attacker: Row,
  target: Row | undefined,
  reach: Reach,
): { distance: number | null; flags: string[] } {
  if (!target) return { distance: null, flags: [] };
  const scene = state.scenes.find((s) => s.sceneId === attacker.sceneId);
  if (!scene) return { distance: null, flags: [] };
  const footprint = (token: Row) =>
    r.footprintFrom(
      MANIFEST,
      JSON.stringify(slotsOf(state, token.actorId).traitData ?? {}),
    );
  return call(() =>
    r.measure(
      JSON.stringify({
        gridType: scene.gridType ?? "square",
        gridSize: scene.gridSize,
        width: scene.width ?? 0,
        height: scene.height ?? 0,
        units: { perCell: 5, label: "ft" },
        walls: state.walls
          .filter((w) => w.sceneId === attacker.sceneId)
          .map((w) => ({
            id: w.wallId,
            x1: w.x1,
            y1: w.y1,
            x2: w.x2,
            y2: w.y2,
            blocksVision: w.blocksVision !== false,
            blocksMovement: w.blocksMovement !== false,
            doorState: String(w.doorState ?? "none").toLowerCase(),
          })),
        from: { x: attacker.x, y: attacker.y, footprint: footprint(attacker) },
        to: { x: target.x, y: target.y, footprint: footprint(target) },
        reach,
      }),
    ),
  );
}

// ---------------------------------------------------------------------------
// The budget.

/** Flags the attack's spend puts on it, spending it when `spend` is set. */
function spendFlags(
  r: Rules,
  state: DemoState,
  attacker: Row,
  cost: string,
  legendaryCost: number,
  spend: boolean,
): string[] {
  const what = r.spendForAttack(COST[cost] ?? "action", legendaryCost);
  if (what === "null") return [];
  const fight = state.fight;
  const combat = combatOn(fight, attacker.sceneId);
  if (!fight || !combat) return [];
  const seat = seatOf(fight, combat, attacker);
  if (!seat || seat.kind === "LAIR") return [];
  if (spend) {
    seat.spent = call(() => r.takeSpend(JSON.stringify(seat.spent), what));
    combat.updatedAt = now();
    record(EVENT.combat, { combatId: combat.id });
  }
  const ownTurn = cost === "LEGENDARY" && combat.activeCombatantId === seat.id;
  return call<string[]>(() =>
    r.spendFlags(
      JSON.stringify(budgetOf(r, state, seat)),
      what,
      ownTurn,
      spend,
    ),
  );
}

// ---------------------------------------------------------------------------
// The weapon.

function formulasOf(
  r: Rules,
  ability: Row,
): { toHit: string; damage: string[] } {
  const effects = [...(ability.effects as Row[])]
    .sort((a, b) => (a.sortOrder as number) - (b.sortOrder as number))
    .map((e) => [String(e.effectType).toLowerCase(), e.formula]);
  return call(() =>
    r.attackFormulas(ability.name as string, JSON.stringify(effects)),
  );
}

function targetOf(input: Args, index: number): string | null {
  return (
    (input.targets?.[index] as string | undefined) ??
    input.targetTokenId ??
    null
  );
}

/** `defence_of`: the declared field off the target's sheet. */
function defenceOf(state: DemoState, target: Row): number | null {
  const value = slotsOf(state, target.actorId)[DEFENCE.slot] as Row | undefined;
  const field = value?.[DEFENCE.field];
  return Number.isInteger(field) ? (field as number) : null;
}

function totalOf(resolution: WasmResolution, fallback: number): number {
  return typeof resolution.kind === "object" &&
    resolution.kind.Total !== undefined
    ? resolution.kind.Total
    : fallback;
}

// ---------------------------------------------------------------------------
// What a viewer is told.

/**
 * `SceneSight::may_know`. The Game Master knows every party; a player knows
 * a creature they control, and one whose name they may read. The server also
 * asks whether one of the player's creatures can see it; the demo does not
 * light the board for the server's sight rule, so it stops at the name — a
 * difference recorded in spec 079's report, and one that only ever tells a
 * player less than the GM, never a hidden name.
 */
function mayKnow(state: DemoState, tokenId: unknown): boolean {
  if (viewerIsGm(state)) return true;
  if (tokenId == null) return false;
  const token = tokenById(state, tokenId as string);
  if (!token) return false;
  return playerControls(state, token) || nameReadable(state, token);
}

function party(state: DemoState, tokenId: unknown, label: string): Row {
  return mayKnow(state, tokenId)
    ? { tokenId: tokenId ?? null, label }
    : { tokenId: null, label: UNKNOWN };
}

/** `may_resolve`: the GM, or a controller of the offer's creature. */
function mayResolve(state: DemoState, offer: Row): boolean {
  if (viewerIsGm(state)) return true;
  return playerControls(state, tokenById(state, offer.targetTokenId as string));
}

function offerRow(state: DemoState, offer: Row): Row {
  const token = tokenById(state, offer.targetTokenId as string);
  const label = token ? tokenLabel(state, token) : UNKNOWN;
  return {
    id: offer.id,
    sceneId: offer.sceneId,
    attackId: offer.attackId,
    kind: offer.kind,
    amount: offer.amount,
    target: party(state, offer.targetTokenId, label),
    status: offer.status,
    resolvedBy: offer.resolvedOnBehalf
      ? "Game Master"
      : offer.resolvedBy === DEMO_PLAYER.id
        ? DEMO_PLAYER.username
        : offer.resolvedBy
          ? "Game Master"
          : null,
    resolvedOnBehalf: offer.resolvedOnBehalf,
    mayResolve: mayResolve(state, offer) && offer.status === "PENDING",
    createdAt: offer.createdAt,
  };
}

/** `build_attacks`, for one attack. */
function attackRow(state: DemoState, attack: Row): Row {
  const gm = viewerIsGm(state);
  const lair = attack.attackerKind === "LAIR";
  const attackerKnown = lair || mayKnow(state, attack.attackerTokenId);
  const targetKnown =
    attack.targetLabel != null && mayKnow(state, attack.targetTokenId);
  const offer = state.fight?.offers.find((o) => o.attackId === attack.id);
  return {
    id: attack.id,
    sceneId: attack.sceneId,
    attacker: lair
      ? { tokenId: null, label: attack.attackerLabel }
      : party(state, attack.attackerTokenId, attack.attackerLabel as string),
    target:
      attack.targetLabel == null
        ? null
        : party(state, attack.targetTokenId, attack.targetLabel as string),
    abilityName: attackerKnown ? attack.abilityName : null,
    toHit: attack.toHit,
    damage: attack.damage,
    defence: targetKnown ? attack.defence : null,
    outcome: attack.outcome,
    distance: attack.distance,
    // "No reach declared" is a note to the GM about their own content.
    flags: (attack.flags as string[])
      .map(enumOf)
      .filter((f) => gm || f !== "NO_REACH_DECLARED"),
    actionCost: attack.actionCost,
    offer: offer ? offerRow(state, offer) : null,
    multiattackOf: attack.multiattackOf,
    rerollOf: attack.rerollOf ?? null,
    createdAt: attack.createdAt,
  };
}

// ---------------------------------------------------------------------------
// Making one.

interface Attacker {
  token: Row | null;
  lair: Row | null;
  sceneId: string;
  label: string;
}

/** `lair_in_fight`, then the Game Master's alone to act for. */
function lairOf(state: DemoState, input: Args): Attacker {
  const fight = fightOf(state);
  const lair = fight.combatants.find(
    (c) => c.id === input.lairCombatantId && c.kind === "LAIR",
  );
  const combat =
    lair && fight.combats.find((c) => c.id === lair.combatId && !c.endedAt);
  if (!lair || !combat) throw new GraphQLError(LAIR_NOT_THERE);
  if (!viewerIsGm(state)) throw new GraphQLError(NOT_CONTROLLED);
  let sceneId = combat.sceneId as string | null;
  if (!sceneId) {
    const target = targetOf(input, 0);
    if (!target) {
      throw new GraphQLError(
        "A lair's action needs a target when its encounter is not in one scene",
      );
    }
    const token = tokenById(state, target);
    if (!token) throw new GraphQLError("That target is not on the board");
    sceneId = token.sceneId as string;
  }
  return { token: null, lair, sceneId, label: lair.label as string };
}

function attackerOf(state: DemoState, input: Args): Attacker {
  if (input.lairCombatantId) return lairOf(state, input);
  const token = tokenById(state, input.attackerTokenId);
  if (!token) throw new GraphQLError(NOT_ON_BOARD);
  return {
    token,
    lair: null,
    sceneId: token.sceneId as string,
    label: tokenLabel(state, token),
  };
}

async function makeAttack({ input }: Args): Promise<Row[]> {
  const r = await rules();
  const state = demoState();
  const fight = fightOf(state);
  const attacker = attackerOf(state, input);
  if (attacker.token && !controls(state, attacker.token)) {
    throw new GraphQLError(NOT_CONTROLLED);
  }
  const ability = abilityFor(
    state,
    attacker.token?.actorId,
    input.abilityId,
    input.itemId,
  );
  const cost =
    (input.actionCost as string | undefined) ?? (ability.actionCost as string);
  if (attacker.token && cost !== "REACTION") {
    const refusal = refusalOf(r, turnCheck(r, state, attacker.token));
    if (refusal) throw new GraphQLError(refusal);
  }
  const declared = formulasOf(r, ability);
  // Spec 084: `record_attack` shapes the to-hit for advantage; a lair never
  // rolls with it.
  const advantage = (input.advantage as Advantage | undefined) ?? "NORMAL";
  if (attacker.lair && advantage !== "NORMAL") {
    throw new GraphQLError(LAIR_NO_ADVANTAGE);
  }
  const traitData = attacker.token
    ? slotsOf(state, attacker.token.actorId).traitData
    : undefined;
  const toHit = shapeD20(declared.toHit, advantage, traitData);
  const target = targetOf(input, 0);
  const targetToken = target ? tokenById(state, target) : undefined;
  if (target && targetToken?.sceneId !== attacker.sceneId) {
    throw new GraphQLError("That target is not on this scene");
  }
  const reach = reachOf(ability);
  const measured = attacker.token
    ? measure(r, state, attacker.token, targetToken, reach)
    : { distance: null, flags: [] };
  const melee = isMelee(measured.distance, reach);
  const properties = (ability.properties as string[] | undefined) ?? [];
  const damage = shapedDamage(declared.damage, traitData, melee, properties);
  const formulas = { toHit: toHit.formula, damage: damage.formulas };

  const combat = combatOn(fight, attacker.sceneId);
  const effectiveAutoApply =
    (combat?.autoApply as boolean | null | undefined) ??
    state.world.autoApplyNpcDamage === true;
  const spent = attacker.token
    ? spendFlags(
        r,
        state,
        attacker.token,
        cost,
        ability.legendaryCost as number,
        true,
      )
    : [];
  const flags = [...measured.flags, ...spent];
  const roller = dice(r, fight);
  const at = now();
  const part = call<{
    toHit: WasmResolution;
    toHitTotal: number;
    outcome: string;
    damage: WasmResolution | null;
    amount: number | null;
  }>(() =>
    r.attackPart(
      roller,
      JSON.stringify({
        toHit: formulas.toHit,
        damage: formulas.damage,
        hasTarget: targetToken !== undefined,
        defence: targetToken ? defenceOf(state, targetToken) : null,
      }),
    ),
  );
  roller.free();
  // `roll_and_record`: each roll is in the table's history and on every
  // board, in the open, named for what it was (spec 081 FR-012).
  const actorId = (attacker.token?.actorId as string | undefined) ?? null;
  const toHitRollId = recordRoll(part.toHit, {
    label: ability.name as string,
    meta: { actorId, rollKind: "to_hit", facets: toHit.facets },
  });
  if (part.damage) {
    recordRoll(part.damage, {
      label: `${ability.name as string} damage`,
      meta: { actorId, rollKind: "damage", facets: damage.facets },
    });
  }
  const attack: Row = {
    id: crypto.randomUUID(),
    worldId: state.world.id,
    sceneId: attacker.sceneId,
    combatId: combat?.id ?? null,
    attackerTokenId: attacker.token?.tokenId ?? null,
    targetTokenId: targetToken?.tokenId ?? null,
    attackerLabel: attacker.label,
    targetLabel: targetToken ? tokenLabel(state, targetToken) : null,
    abilityName: ability.name,
    multiattackOf: null,
    rerollOf: null,
    // What `reroll_attack` needs to judge and settle this attack again.
    actorId,
    toHitRollId,
    damageFormulas: declared.damage,
    melee,
    properties,
    needsLineOfSight: reach.needsLineOfSight,
    toHit: resolutionRow(part.toHit, part.toHitTotal),
    damage: part.damage
      ? resolutionRow(part.damage, totalOf(part.damage, part.amount ?? 0))
      : null,
    defence: targetToken ? defenceOf(state, targetToken) : null,
    outcome: enumOf(part.outcome),
    distance: measured.distance,
    flags,
    actionCost: cost,
    attackerKind: attacker.lair ? "LAIR" : "CREATURE",
    createdAt: at,
  };
  fight.attacks.push(attack);
  record(EVENT.attack, { attackId: attack.id });

  // A hit on a creature with hit points is an offer of that damage.
  if (targetToken && part.amount !== null) {
    settleHit(r, state, fight, attack, targetToken, part.amount, {
      autoApply: effectiveAutoApply,
      needsLineOfSight: reach.needsLineOfSight,
      at,
    });
  }
  markChanged();
  return [attackRow(state, attack)];
}

/**
 * `combat/attack_hit.rs` `settle_hit`: a hit's damage offered to whoever
 * controls the target, or applied when auto-apply holds, and event 30.
 */
function settleHit(
  r: Rules,
  state: DemoState,
  fight: Fight,
  attack: Row,
  targetToken: Row,
  amount: number,
  hit: { autoApply: boolean; needsLineOfSight: boolean; at: string },
): void {
  if (!actorOf(state, targetToken)) return;
  const offer: Row = {
    id: crypto.randomUUID(),
    worldId: state.world.id,
    sceneId: attack.sceneId,
    attackId: attack.id,
    targetTokenId: targetToken.tokenId,
    targetLinked: targetToken.linked === true,
    kind: "DAMAGE",
    amount,
    status: "PENDING",
    resolvedBy: null,
    resolvedOnBehalf: false,
    resolvedAt: null,
    createdAt: hit.at,
  };
  fight.offers.push(offer);
  const holds = r.autoApplyHolds(
    hit.autoApply,
    playerControls(state, targetToken),
    JSON.stringify(attack.flags),
    hit.needsLineOfSight,
  );
  if (holds) {
    try {
      applyHitPointChange(r, state, fight, targetToken, "DAMAGE", amount);
      offer.status = "APPLIED";
      offer.resolvedAt = hit.at;
    } catch {
      // `auto-apply left an offer pending`: the GM decides it by hand.
    }
  }
  record(EVENT.offer, { offerId: offer.id });
}

/** `rolls::reroll::attack_hit`: whether this to-hit's attack landed. */
export function attackHit(rollId: unknown): boolean {
  return (demoState().fight?.attacks ?? []).some(
    (a) => a.toHitRollId === rollId && a.outcome === "HIT",
  );
}

/** `thunderforge_combat::attack::judge`. */
function judge(hasTarget: boolean, defence: number | null, total: number) {
  if (!hasTarget) return "NO_TARGET";
  if (defence === null) return "NO_DEFENCE";
  return total >= defence ? "HIT" : "MISS";
}

/**
 * `combat/attack_reroll.rs` `reroll_attack` (spec 084 research R7): the
 * attack `oldRollId` was the to-hit of, judged again with `newRollId` as its
 * to-hit, against the defence stored with it. A new row points back at the
 * miss; a hit rolls its damage and settles it as a first-time hit; nothing
 * is spent from the budget.
 */
export async function rerollAttack(
  oldRollId: string,
  newRollId: string,
  resolution: WasmResolution,
): Promise<void> {
  const r = await rules();
  const state = demoState();
  const fight = fightOf(state);
  const first = fight.attacks.find((a) => a.toHitRollId === oldRollId);
  if (!first) throw new GraphQLError(NOT_A_D20_TEST);
  if (first.outcome === "HIT") throw new GraphQLError(A_HIT);
  const total = totalOf(resolution, 0);
  const outcome = judge(
    first.targetTokenId != null,
    (first.defence as number | null) ?? null,
    total,
  );
  const at = now();
  let damage: { resolution: WasmResolution; value: number } | null = null;
  const formulas = (first.damageFormulas as string[] | undefined) ?? [];
  const actorId = (first.actorId as string | null) ?? null;
  // Shaped as the first hit's damage would have been (spec 084 R6).
  const shaped = shapedDamage(
    formulas,
    actorId ? slotsOf(state, actorId).traitData : undefined,
    first.melee === true,
    (first.properties as string[] | undefined) ?? [],
  );
  if (outcome === "HIT" && formulas.length > 0) {
    const source =
      shaped.formulas.length === 1
        ? shaped.formulas[0]
        : shaped.formulas.map((f) => `(${f})`).join("+");
    const roller = dice(r, fight);
    damage = call(() => r.roll(roller, source, "{}"));
    roller.free();
    recordRoll(damage!.resolution, {
      label: `${first.abilityName as string} damage`,
      meta: { actorId, rollKind: "damage", facets: shaped.facets },
    });
  }
  const amount = damage === null ? null : Math.max(0, Math.round(damage.value));
  const attack: Row = {
    ...first,
    id: crypto.randomUUID(),
    toHitRollId: newRollId,
    toHit: resolutionRow(resolution, total),
    damage: damage ? resolutionRow(damage.resolution, damage.value) : null,
    outcome,
    rerollOf: first.id,
    createdAt: at,
  };
  fight.attacks.push(attack);
  record(EVENT.attack, { attackId: attack.id });
  const targetToken =
    first.targetTokenId != null
      ? tokenById(state, first.targetTokenId as string)
      : undefined;
  if (targetToken && amount !== null) {
    const combat = combatOn(fight, first.sceneId);
    settleHit(r, state, fight, attack, targetToken, amount, {
      autoApply:
        (combat?.autoApply as boolean | null | undefined) ??
        state.world.autoApplyNpcDamage === true,
      needsLineOfSight: first.needsLineOfSight !== false,
      at,
    });
  }
  markChanged();
}

async function previewAttack({ input }: Args): Promise<Row> {
  const r = await rules();
  const state = demoState();
  const preview: Row = {
    distance: null,
    flags: [],
    turn: { allowed: true, activeLabel: null },
    reach: null,
    rangeNormal: null,
    rangeLong: null,
    unit: "",
  };
  if (input.lairCombatantId) {
    lairOf(state, input);
    return preview;
  }
  const token = tokenById(state, input.attackerTokenId);
  if (!token) throw new GraphQLError(NOT_ON_BOARD);
  if (input.actionCost !== "REACTION")
    preview.turn = turnCheck(r, state, token);
  let ability: Row;
  try {
    ability = abilityFor(state, token.actorId, input.abilityId, input.itemId);
    formulasOf(r, ability);
  } catch {
    return preview;
  }
  const reach = reachOf(ability);
  const target = targetOf(input, 0);
  const measured = measure(
    r,
    state,
    token,
    target ? tokenById(state, target) : undefined,
    reach,
  );
  const cost =
    (input.actionCost as string | undefined) ?? (ability.actionCost as string);
  const spent = spendFlags(
    r,
    state,
    token,
    cost,
    ability.legendaryCost as number,
    false,
  );
  return {
    ...preview,
    distance: measured.distance,
    flags: [...new Set(measured.flags), ...spent].map(enumOf),
    reach: reach.reach,
    rangeNormal: reach.rangeNormal,
    rangeLong: reach.rangeLong,
    unit: "ft",
  };
}

async function resolveOffer({ offerId, take }: Args): Promise<Row> {
  const r = await rules();
  const state = demoState();
  const fight = fightOf(state);
  const offer = fight.offers.find((o) => o.id === offerId);
  if (!offer) throw new GraphQLError(OFFER_NOT_THERE);
  const token = tokenById(state, offer.targetTokenId as string);
  if (!token) throw new GraphQLError(OFFER_NOT_THERE);
  const gm = viewerIsGm(state);
  if (!gm && !playerControls(state, token))
    throw new GraphQLError(NOT_CONTROLLED);
  if (offer.status !== "PENDING") throw new GraphQLError(ALREADY_RESOLVED);
  const viewer = gm ? "gm" : DEMO_PLAYER.id;
  if (take) {
    if ((token.linked === true) !== offer.targetLinked) {
      throw new GraphQLError(RELINKED_SINCE);
    }
    applyHitPointChange(
      r,
      state,
      fight,
      token,
      offer.kind as "DAMAGE" | "HEALING",
      offer.amount as number,
    );
  }
  offer.status = take ? "TAKEN" : "DECLINED";
  offer.resolvedBy = viewer;
  offer.resolvedOnBehalf = gm && playerControls(state, token);
  offer.resolvedAt = now();
  record(EVENT.offer, { offerId: offer.id });
  markChanged();
  return offerRow(state, offer);
}

export const attackQueries: Record<string, Handler> = {
  previewAttack,
  attack: ({ id }) => {
    const state = demoState();
    const attack = state.fight?.attacks.find((a) => a.id === id);
    return attack ? attackRow(state, attack) : null;
  },
  sceneAttacks: ({ sceneId, before }) => {
    const state = demoState();
    const newestFirst = (state.fight?.attacks ?? [])
      .filter((a) => a.sceneId === sceneId)
      .reverse();
    const from = before ? newestFirst.findIndex((a) => a.id === before) + 1 : 0;
    return newestFirst
      .slice(from, from + SCENE_ATTACKS_PAGE)
      .map((a) => attackRow(state, a));
  },
  pendingOffers: () => {
    const state = demoState();
    return (state.fight?.offers ?? [])
      .filter((o) => o.status === "PENDING" && mayResolve(state, o))
      .map((o) => offerRow(state, o));
  },
  worldAbilities: ({ search }) => {
    const state = demoState();
    const gm = viewerIsGm(state);
    return worldAbilities(state, search).filter((a) => gm || !a.gmOnly);
  },
  actorAbilities: ({ actorId }) => actorAbilities(demoState(), actorId),
};

export const attackMutations: Record<string, Handler> = {
  makeAttack,
  resolveOffer,
};
