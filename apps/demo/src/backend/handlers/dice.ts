/**
 * Dice, as the server rolls them (spec 014).
 *
 * `rollDice`, `validateDiceFormula` and `worldRollRecords` mirror
 * `crates/thunderforge-server/src/graphql/mutations_roll.rs` and
 * `graphql/queries/roll.rs`. The formula is parsed and resolved by
 * `crates/thunderforge-dice` itself, built for the page (`@thunderforge/dice`,
 * `scripts/shared.mjs` `buildDice`), so the demo accepts, refuses and resolves
 * exactly what the server would. Nothing here knows what a die is.
 *
 * The entropy is the browser's: four fresh words from `crypto.getRandomValues`
 * per roll, where the server seeds a `StdRng` from the OS. A test fixes the
 * seed with `seedDice`.
 */
import init, { initSync, roll, validateFormula } from "@thunderforge/dice";
import { GraphQLError } from "graphql";
import { viewerIsGm, viewerUser } from "../actors";
import { demoState, markChanged, type Row } from "../state";

type Args = Record<string, any>; // eslint-disable-line @typescript-eslint/no-explicit-any

let loaded: Promise<unknown> | null = null;

/** The dice module, compiled once per page. */
function diceReady(): Promise<unknown> {
  loaded ??= init();
  return loaded;
}

/** For a test, which has the bytes and no page to fetch them from. */
export function loadDiceForTest(bytes: BufferSource): void {
  initSync({ module: bytes });
  loaded = Promise.resolve();
}

let fixedSeed: number[] | null = null;
let rollsSinceSeed = 0;

/** Every roll after this is decided by `seed`; `null` gives the page's own. */
export function seedDice(seed: number[] | null): void {
  fixedSeed = seed;
  rollsSinceSeed = 0;
}

function nextSeed(): Uint32Array {
  if (fixedSeed) {
    const words = new Uint32Array(4);
    fixedSeed.forEach((word, i) => (words[i % 4] ^= word));
    words[3] ^= ++rollsSinceSeed;
    return words;
  }
  return crypto.getRandomValues(new Uint32Array(4));
}

/** `thunderforge_dice::RollResolution`, as serde writes it. */
interface Resolution {
  formula: string;
  dice: Array<{
    sides: { Numeric: number } | "Fate" | "Coin";
    rolls: number[];
    kept: boolean;
    final_value: number;
  }>;
  kind: { Total: number } | { SuccessCount: number };
}

/** `GraphQLRollResolution::from` (`graphql/types_dice.rs`). */
export function resolutionRow(detail: Resolution, outcome: Row | null): Row {
  const total = "Total" in detail.kind;
  return {
    formula: detail.formula,
    dice: detail.dice.map((die) => ({
      sidesKind:
        typeof die.sides === "object" ? "NUMERIC" : die.sides.toUpperCase(),
      numericSides: typeof die.sides === "object" ? die.sides.Numeric : null,
      rolls: die.rolls,
      kept: die.kept,
      finalValue: die.final_value,
    })),
    resultKind: total ? "TOTAL" : "SUCCESS_COUNT",
    resultValue:
      "Total" in detail.kind ? detail.kind.Total : detail.kind.SuccessCount,
    outcome,
  };
}

/** The most roll records a world keeps here; the most the server answers. */
const KEPT = 500;

/**
 * `roll_dice_impl`: resolve, then record. A formula that fails to parse or
 * resolve rolls nothing and records nothing (FR-011), refused in the server's
 * words.
 */
export async function resolveAndRecord(
  formula: string,
  bindings: Record<string, number>,
  outcome: Row | null = null,
): Promise<Row> {
  await diceReady();
  let detail: Resolution;
  try {
    detail = JSON.parse(
      roll(formula, JSON.stringify(bindings), nextSeed()),
    ) as Resolution;
  } catch (error) {
    throw new GraphQLError(`Roll rejected: ${(error as Error).message}`);
  }
  const state = demoState();
  const rolls = (state.rolls ??= []);
  rolls.push({
    id: crypto.randomUUID(),
    worldId: state.world.id,
    triggeredBy: viewerUser(state).id,
    detail,
    outcome,
    createdAt: new Date().toISOString(),
  });
  rolls.splice(0, Math.max(0, rolls.length - KEPT));
  markChanged();
  return resolutionRow(detail, outcome);
}

export const diceQueries = {
  validateDiceFormula: async ({ formula }: Args) => {
    await diceReady();
    return validateFormula(String(formula));
  },
  /** DM-only, newest first, 50 unless asked, never more than 500. */
  worldRollRecords: ({ limit }: Args) => {
    const state = demoState();
    if (!viewerIsGm(state)) {
      throw new GraphQLError("Only the DM (Owner or GM) may view roll history");
    }
    const take = Math.min(Math.max(limit ?? 50, 1), KEPT);
    return [...(state.rolls ?? [])]
      .reverse()
      .slice(0, take)
      .map((record) => ({
        ...record,
        resolution: resolutionRow(
          record.detail as Resolution,
          (record.outcome as Row | null) ?? null,
        ),
      }));
  },
};

export const diceMutations = {
  rollDice: ({ input }: Args) =>
    resolveAndRecord(
      String(input.formula),
      Object.fromEntries(
        ((input.bindings ?? []) as Array<{ name: string; value: number }>).map(
          (b) => [b.name, b.value],
        ),
      ),
    ),
};
