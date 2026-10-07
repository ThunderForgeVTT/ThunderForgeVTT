/**
 * Spec 079: a fight in the browser, answered as the server answers it.
 *
 * The encounter's life — start, the order, the turn, the budget, the end — is
 * `graphql/mutations_combat.rs` and `mutations_combat_lair.rs`; attacks and
 * offers are `combatAttacks.ts`; hit points are `combatHp.ts`. The rules are
 * the crate's, loaded on first use (`combatRules.ts`), so a visitor who never
 * opens the fight never downloads them.
 *
 * Where the demo differs from the server, it says so beside the difference.
 */
import { GraphQLError } from "graphql";
import { modifierFor, viewerIsGm } from "../actors";
import { EVENT, now, record } from "../events";
import {
  demoState,
  markChanged,
  type DemoState,
  type Fight,
  type Row,
} from "../state";
import { attackMutations, attackQueries } from "./combatAttacks";
import { applyHitPointChange, tokenStatus } from "./combatHp";
import {
  MANIFEST,
  NOTHING_SPENT,
  budgetOf,
  call,
  dice,
  fightOf,
  nameReadable,
  rules,
  slotsOf,
  tokenById,
  tokenLabel,
  type Args,
  type Rules,
} from "./combatRules";

type Handler = (args: Args) => unknown;

/** `combat/lair.rs`: a lair acts on initiative count 20 and loses ties. */
const LAIR_INITIATIVE = 20;
const LAIR_TIEBREAK = -1;

function requireGm(state: DemoState, refusal: string): void {
  if (!viewerIsGm(state)) throw new GraphQLError(refusal);
}

function combatById(fight: Fight, combatId: string): Row {
  const combat = fight.combats.find((c) => c.id === combatId);
  if (!combat) throw new GraphQLError("Combat not found");
  return combat;
}

export function runningCombat(fight: Fight | undefined): Row | undefined {
  return fight?.combats.find((c) => !c.endedAt);
}

/** `sort_combatants`, by the crate's `sortSeats`. */
export function ordered(r: Rules, fight: Fight, combatId: string): Row[] {
  const seats = fight.combatants.filter((c) => c.combatId === combatId);
  const sorted = call<Array<{ id: string }>>(() =>
    r.sortSeats(
      JSON.stringify(
        seats.map((c) => ({
          id: c.id,
          initiative: c.initiative,
          tiebreak: c.tiebreak,
          active: c.active,
          tokenId: c.tokenId,
          actorId: c.actorId,
          label: c.label,
        })),
      ),
    ),
  );
  return sorted.map((seat) => seats.find((c) => c.id === seat.id) as Row);
}

function combatantRow(r: Rules, state: DemoState, combatant: Row): Row {
  const { spent: _spent, ...row } = combatant;
  let budget: Row | null = null;
  if (combatant.kind !== "LAIR") {
    const b = budgetOf(r, state, combatant);
    budget = {
      action: b.action,
      bonusAction: b.bonus_action,
      reaction: b.reaction,
      movement: b.movement,
      legendary: b.legendary,
      unit: b.unit,
    };
  }
  // `load_combat`: a player is not told the name of a creature whose name
  // they may not read.
  const hidden =
    !viewerIsGm(state) &&
    combatant.tokenId != null &&
    !nameReadable(state, tokenById(state, combatant.tokenId as string));
  return { ...row, label: hidden ? "Unknown" : row.label, budget };
}

export function combatRow(r: Rules, state: DemoState, combat: Row): Row {
  const fight = fightOf(state);
  return {
    ...combat,
    effectiveAutoApply:
      (combat.autoApply as boolean | null) ??
      state.world.autoApplyNpcDamage === true,
    roundLabel: r.roundLabel(MANIFEST) ?? null,
    combatants: ordered(r, fight, combat.id as string).map((c) =>
      combatantRow(r, state, c),
    ),
  };
}

/** The event every change to an encounter records, and the answer. */
function changed(r: Rules, state: DemoState, combat: Row): Row {
  combat.updatedAt = now();
  record(EVENT.combat, { combatId: combat.id });
  markChanged();
  return combatRow(r, state, combat);
}

/** `start_turn`: the new active combatant has its whole turn back. */
function startTurn(r: Rules, combatant: Row | undefined): void {
  if (!combatant || combatant.kind === "LAIR") return;
  combatant.spent = call(() =>
    r.startTurn(JSON.stringify(combatant.spent ?? NOTHING_SPENT)),
  );
}

/**
 * Hands the turn on from the combatant who holds it, the way `advance_turn`
 * does. Returns false when nobody is left to take it.
 */
function handOn(
  r: Rules,
  fight: Fight,
  combat: Row,
  without?: string,
): boolean {
  const order = ordered(r, fight, combat.id as string).filter(
    (c) => c.id !== without,
  );
  const active =
    without !== undefined && combat.activeCombatantId === without
      ? undefined
      : ((combat.activeCombatantId as string | null) ?? undefined);
  const next = call<{ index: number; newRound: boolean } | null>(() =>
    r.nextTurn(
      JSON.stringify(order.map((c) => ({ id: c.id, active: c.active }))),
      active,
    ),
  );
  if (!next) return false;
  combat.activeCombatantId = order[next.index].id;
  if (next.newRound) combat.round = (combat.round as number) + 1;
  startTurn(r, order[next.index]);
  return true;
}

/**
 * Which token an actor joins with when the panel names only the actor. The
 * server files such a combatant with no token; the demo picks the actor's
 * first token on the encounter's scene not already in the order, so the
 * combatant's hit points and attacks are the creature on the board. A
 * difference from the server, recorded in spec 079's report.
 */
function tokenFor(
  state: DemoState,
  fight: Fight,
  combat: Row,
  actorId: string,
) {
  const sceneId =
    (combat.sceneId as string | null) ?? state.world.activeSceneId;
  const taken = new Set(
    fight.combatants
      .filter((c) => c.combatId === combat.id)
      .map((c) => c.tokenId),
  );
  return state.tokens.find(
    (t) =>
      t.sceneId === sceneId && t.actorId === actorId && !taken.has(t.tokenId),
  );
}

/**
 * Initiative, when the panel sends none: a d20 and the creature's Dexterity,
 * thrown by the crate's dice. The server files 0 and leaves it to the Game
 * Master; the demo's visitor has no table to roll at, so it is rolled for
 * them. Recorded in spec 079's report.
 */
function rollInitiative(
  r: Rules,
  state: DemoState,
  fight: Fight,
  actorId: unknown,
) {
  const modifier = actorId
    ? modifierFor(slotsOf(state, actorId), "dexterity")
    : 0;
  const formula = `1d20 ${modifier < 0 ? "-" : "+"} ${Math.abs(modifier)}`;
  const answer = call<{ value: number }>(() =>
    r.roll(dice(r, fight), formula, "{}"),
  );
  return Math.round(answer.value);
}

const queries: Record<string, Handler> = {
  activeCombat: async () => {
    const r = await rules();
    const state = demoState();
    const combat = runningCombat(state.fight);
    return combat ? combatRow(r, state, combat) : null;
  },
  tokenStatus: ({ sceneId }) => tokenStatus(demoState(), sceneId),
  ...attackQueries,
};

const mutations: Record<string, Handler> = {
  startCombat: async ({ input }) => {
    const r = await rules();
    const state = demoState();
    requireGm(state, "Only the GM may start combat");
    const fight = fightOf(state);
    const running = runningCombat(fight);
    if (running) return combatRow(r, state, running);
    const at = now();
    const combat: Row = {
      id: crypto.randomUUID(),
      worldId: state.world.id,
      sceneId: input.sceneId ?? null,
      round: 1,
      activeCombatantId: null,
      endedAt: null,
      autoApply: null,
      createdAt: at,
      updatedAt: at,
    };
    fight.combats.push(combat);
    return changed(r, state, combat);
  },

  endCombat: async ({ combatId }) => {
    const r = await rules();
    const state = demoState();
    requireGm(state, "Only the GM may end combat");
    const combat = combatById(fightOf(state), combatId);
    combat.endedAt ??= now();
    return changed(r, state, combat);
  },

  addCombatant: async ({ input }) => {
    const r = await rules();
    const state = demoState();
    const label = String(input.label ?? "").trim();
    if (!label) throw new GraphQLError("Combatant needs a name");
    requireGm(state, "Only the GM may change combat");
    const fight = fightOf(state);
    const combat = combatById(fight, input.combatId);
    let tokenId = (input.tokenId as string | null | undefined) ?? null;
    let actorId = (input.actorId as string | null | undefined) ?? null;
    let named = label;
    if (!tokenId && actorId) {
      const token = tokenFor(state, fight, combat, actorId);
      if (token) {
        tokenId = token.tokenId as string;
        const actor = state.actors.find((a) => a.id === actorId);
        if (label === actor?.label) named = tokenLabel(state, token);
      }
    }
    if (tokenId && !actorId) {
      actorId = (tokenById(state, tokenId)?.actorId as string | null) ?? null;
    }
    fight.combatants.push({
      id: crypto.randomUUID(),
      combatId: combat.id,
      actorId,
      tokenId,
      label: named,
      initiative: input.initiative ?? rollInitiative(r, state, fight, actorId),
      tiebreak: input.tiebreak ?? 0,
      isNpc: input.isNpc ?? false,
      active: true,
      downedBy: null,
      kind: "CREATURE",
      spent: { ...NOTHING_SPENT },
    });
    return changed(r, state, combat);
  },

  addLairCombatant: async ({ combatId, label }) => {
    const r = await rules();
    const state = demoState();
    const name = String(label ?? "").trim();
    if (!name) throw new GraphQLError("A lair needs a name");
    requireGm(state, "Only the GM may change combat");
    const fight = fightOf(state);
    const combat = combatById(fight, combatId);
    if (combat.endedAt) throw new GraphQLError("This combat has already ended");
    fight.combatants.push({
      id: crypto.randomUUID(),
      combatId: combat.id,
      actorId: null,
      tokenId: null,
      label: name,
      initiative: LAIR_INITIATIVE,
      tiebreak: LAIR_TIEBREAK,
      isNpc: true,
      active: true,
      downedBy: null,
      kind: "LAIR",
      spent: null,
    });
    return changed(r, state, combat);
  },

  updateCombatant: async ({ input }) => {
    const r = await rules();
    const state = demoState();
    requireGm(state, "Only the GM may change combat");
    const fight = fightOf(state);
    const combatant = fight.combatants.find((c) => c.id === input.combatantId);
    if (!combatant) throw new GraphQLError("Combatant not found");
    if (input.label != null) {
      const label = String(input.label).trim();
      if (!label) throw new GraphQLError("Combatant needs a name");
      combatant.label = label;
    }
    if (input.initiative != null) combatant.initiative = input.initiative;
    if (input.tiebreak != null) combatant.tiebreak = input.tiebreak;
    if (input.active != null) {
      // Put out by hand is the Game Master's; brought back clears either.
      combatant.active = input.active;
      combatant.downedBy = input.active ? null : "GAME_MASTER";
    }
    return changed(r, state, combatById(fight, combatant.combatId as string));
  },

  removeCombatant: async ({ combatantId }) => {
    const r = await rules();
    const state = demoState();
    requireGm(state, "Only the GM may change combat");
    const fight = fightOf(state);
    const combatant = fight.combatants.find((c) => c.id === combatantId);
    if (!combatant) throw new GraphQLError("Combatant not found");
    const combat = combatById(fight, combatant.combatId as string);
    if (combat.activeCombatantId === combatantId) {
      if (!handOn(r, fight, combat, combatantId as string)) {
        combat.activeCombatantId = null;
      }
    }
    fight.combatants = fight.combatants.filter((c) => c.id !== combatantId);
    return changed(r, state, combat);
  },

  advanceTurn: async ({ combatId }) => {
    const r = await rules();
    const state = demoState();
    const fight = fightOf(state);
    const combat = combatById(fight, combatId);
    requireGm(state, "Only the GM may advance the turn");
    if (combat.endedAt) throw new GraphQLError("This combat has already ended");
    if (!handOn(r, fight, combat)) {
      throw new GraphQLError("No active combatants to advance to");
    }
    return changed(r, state, combat);
  },

  changeHitPoints: async ({ tokenId, kind, amount }) => {
    const r = await rules();
    const state = demoState();
    requireGm(state, "Only the Game Master may change a creature's hit points");
    const token = tokenById(state, tokenId);
    if (!token) throw new GraphQLError("That creature is not on the board");
    const after = applyHitPointChange(
      r,
      state,
      fightOf(state),
      token,
      kind,
      amount,
    );
    return { tokenId, ...after };
  },

  setCombatAutoApply: async ({ combatId, enabled }) => {
    const r = await rules();
    const state = demoState();
    requireGm(state, "Only the GM may change combat");
    const combat = combatById(fightOf(state), combatId);
    combat.autoApply = enabled ?? null;
    return changed(r, state, combat);
  },

  updateWorldAutoApplyNpcDamage: ({ input }) => {
    const state = demoState();
    requireGm(state, "Only the Game Master may change this setting");
    state.world.autoApplyNpcDamage = input.enabled === true;
    state.world.updatedAt = now();
    const combat = runningCombat(state.fight);
    if (combat) record(EVENT.combat, { combatId: combat.id });
    markChanged();
    return state.world;
  },
  ...attackMutations,
};

/**
 * Spread at the end of `handlers.ts`'s lists, so these replace the empty
 * answers the demo gave before it had a fight.
 */
export const combatQueries = queries;
export const combatMutations = mutations;
