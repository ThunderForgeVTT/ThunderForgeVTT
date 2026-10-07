/**
 * Spec 079: the rules of a fight, as the server plays them.
 *
 * Every rule — the dice, the turn order, the budget, reach and range, hit and
 * miss, hit points and who is down — is `crates/thunderforge-combat`, the
 * crate the server calls, compiled to wasm (ADR-113). This file only loads it,
 * keeps the fight's rows in the demo's state, and turns the crate's answers
 * into the shapes the schema gives them. Nothing here decides a rule.
 */
import type * as CombatRules from "@thunderforge/combat";
import { GraphQLError } from "graphql";
import system from "../../../../../packs/systems/dnd5e/system.json";
import { viewerIsGm } from "../actors";
import type { DemoState, Fight, Row } from "../state";
import { DEMO_PLAYER } from "../../seed/world";

export type Rules = typeof CombatRules;
export type Args = Record<string, any>; // eslint-disable-line @typescript-eslint/no-explicit-any

/** The manifest the crate reads its declarations from: the 5e pack's own. */
export const MANIFEST = JSON.stringify(system);

let loading: Promise<Rules> | null = null;

/**
 * The crate, loaded the first time a fight needs it and never before: a
 * visitor who never opens the fight never downloads it (spec 079 FR-010).
 */
export function rules(): Promise<Rules> {
  loading ??= import("@thunderforge/combat").then(async (mod) => {
    await mod.default();
    return mod;
  });
  return loading;
}

export type { Fight };

/**
 * The end-to-end suite's way to fix the dice: a seed in this key is used in
 * place of the browser's randomness when a fight is first made.
 */
export const DICE_SEED_KEY = "thunderforge-demo:dice-seed";

function freshSeed(): number {
  try {
    const fixed = window.localStorage.getItem(DICE_SEED_KEY);
    if (fixed !== null && /^\d+$/.test(fixed)) return Number(fixed) >>> 0;
  } catch {
    // No storage: the browser's randomness, then.
  }
  return crypto.getRandomValues(new Uint32Array(1))[0];
}

export function fightOf(state: DemoState): Fight {
  state.fight ??= {
    combats: [],
    combatants: [],
    attacks: [],
    offers: [],
    copies: {},
    seed: freshSeed(),
    draws: 0,
  };
  return state.fight;
}

/**
 * Dice for one throw. Each throw takes the next seed in a sequence the saved
 * world remembers, so a reload goes on from where it was rather than
 * replaying the same rolls (FR-006).
 */
export function dice(r: Rules, fight: Fight): CombatRules.Dice {
  const n = fight.draws++;
  return r.Dice.seeded((fight.seed + Math.imul(n + 1, 0x9e3779b9)) >>> 0);
}

/** The crate answers in JSON; it refuses in a plain sentence. */
export function call<T>(answer: () => string): T {
  try {
    return JSON.parse(answer()) as T;
  } catch (error) {
    throw new GraphQLError(
      typeof error === "string" ? error : (error as Error).message,
    );
  }
}

interface WasmDie {
  sides: { Numeric?: number } | string;
  rolls: number[];
  kept: boolean;
  final_value: number;
}

export interface WasmResolution {
  formula: string;
  dice: WasmDie[];
  kind: { Total?: number } | string;
}

/** `GraphQLRollResolution`, from the crate's `RollResolution`. */
export function resolutionRow(resolution: WasmResolution, value: number): Row {
  return {
    formula: resolution.formula,
    dice: resolution.dice.map((die) => {
      // `types_dice.rs`: a numbered die, or `Fate` and `Coin` by name.
      const numeric =
        typeof die.sides === "object" ? (die.sides.Numeric ?? null) : null;
      return {
        sidesKind:
          numeric === null ? String(die.sides).toUpperCase() : "NUMERIC",
        numericSides: numeric,
        rolls: die.rolls,
        kept: die.kept,
        finalValue: die.final_value,
      };
    }),
    resultKind: "TOTAL",
    resultValue: value,
    outcome: null,
  };
}

/** A flag as the crate spells it (`out_of_reach`) to the schema's enum. */
export const enumOf = (flag: string) => flag.toUpperCase();

/** The token on the board, or the server's refusal. */
export function tokenById(state: DemoState, tokenId: string): Row | undefined {
  return state.tokens.find((t) => t.tokenId === tokenId);
}

export function actorOf(state: DemoState, token: Row | undefined): Row | null {
  if (!token?.actorId) return null;
  return state.actors.find((a) => a.id === token.actorId) ?? null;
}

export function slotsOf(state: DemoState, actorId: unknown): Row {
  return (state.systemData.find((row) => row.actorId === actorId) ?? {}) as Row;
}

/**
 * Whether the viewer may act for a token: the Game Master always, a player
 * for a token they own or whose actor is theirs
 * (`combat/controllers.rs`).
 */
export function controls(state: DemoState, token: Row | undefined): boolean {
  if (viewerIsGm(state)) return true;
  return playerControls(state, token);
}

/** Whether the seeded player controls the token, whoever is looking. */
export function playerControls(state: DemoState, token: Row | undefined) {
  if (!token) return false;
  if (token.ownerUserId === DEMO_PLAYER.id) return true;
  const actor = actorOf(state, token);
  return !!actor && !actor.isNpc && actor.ownedBy === DEMO_PLAYER.id;
}

/** What a token is called on the board: its own label, else its actor's. */
export function tokenLabel(state: DemoState, token: Row | undefined): string {
  const metadata = token?.metadata as Row | null | undefined;
  if (typeof metadata?.label === "string" && metadata.label.trim()) {
    return metadata.label;
  }
  if (typeof token?.name === "string" && token.name.trim()) return token.name;
  const actor = actorOf(state, token);
  return (actor?.label as string | undefined) ?? "Unnamed creature";
}

/**
 * Whether a player may read a token's name: shown to players, and its actor
 * either a player's or revealed (`combat/redaction.rs`).
 */
export function nameReadable(state: DemoState, token: Row | undefined) {
  if (!token || token.nameVisibleToPlayers !== true) return false;
  const actor = actorOf(state, token);
  return !actor || !actor.isNpc || actor.visibleToPlayers === true;
}

/** A turn with nothing spent yet (`Spent::default`). */
export const NOTHING_SPENT = {
  actionSpent: 0,
  bonusActionSpent: 0,
  reactionSpent: 0,
  movementSpent: 0,
  legendaryPerRound: null,
  legendaryRemaining: null,
};

/** Walking speed off the sheet, as the budget reads it (`budget.rs`). */
function speedOf(state: DemoState, actorId: unknown): number {
  const traits = (slotsOf(state, actorId).traitData ?? {}) as Row;
  return typeof traits.speed_walk === "number" ? traits.speed_walk : 30;
}

type BudgetLine = { allowed: number; spent: number; remaining: number };

/** One combatant's budget, resolved by the crate (`budget_of`). */
export function budgetOf(
  r: Rules,
  state: DemoState,
  combatant: Row,
): Record<string, BudgetLine | string | null> {
  return call(() =>
    r.resolveBudget(
      JSON.stringify({
        manifest: system,
        speed: speedOf(state, combatant.actorId),
        spent: combatant.spent ?? NOTHING_SPENT,
      }),
    ),
  );
}
