/**
 * Dice, as the server rolls them (spec 014).
 *
 * `rollDice`, `revealRoll`, `validateDiceFormula`, `worldRollRecords`,
 * `worldRoll` and `worldRolls` mirror
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
import { DEMO_PLAYER, DEMO_USER } from "../../seed/world";
import { viewerIsGm, viewerUser } from "../actors";
import { EVENT, record } from "../events";
import { demoState, markChanged, type DemoState, type Row } from "../state";

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
    /** Why each value after the first was rolled; absent before spec 083. */
    steps?: Array<"Reroll" | "Explode">;
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
      steps: (die.steps ?? []).map((step) =>
        step === "Explode" ? "EXPLODE" : "REROLL",
      ),
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

/** `MAX_ROLL_LABEL`. */
const MAX_LABEL = 80;
/** The feed's page, unless asked; never more than `FEED_MOST`. */
const FEED_PAGE = 50;
const FEED_MOST = 100;

/** Spec 081 `rolls/visibility.rs`, as stored. */
export type Visibility = "everyone" | "gm_eyes" | "gm_only";

const VISIBILITY_ENUM: Record<Visibility, string> = {
  everyone: "EVERYONE",
  gm_eyes: "GM_EYES",
  gm_only: "GM_ONLY",
};

/** A roll recorded before spec 081 has no visibility: it was public. */
function visibilityOf(record: Row): Visibility {
  const stored = record.visibility as string | undefined;
  if (stored === undefined || stored === "everyone") return "everyone";
  // Anything unrecognised reads as the most hidden, as `Visibility::parse`.
  return stored === "gm_eyes" ? "gm_eyes" : "gm_only";
}

function visibilityFromInput(value: unknown): Visibility {
  if (value === "GM_EYES") return "gm_eyes";
  if (value === "GM_ONLY") return "gm_only";
  return "everyone";
}

/** `may_roll`: neither side may borrow the other's kind of hidden roll. */
function mayRoll(visibility: Visibility, isGm: boolean): void {
  if (visibility === "gm_eyes" && isGm) {
    throw new GraphQLError("The GM rolls GM only, not for the GM's eyes");
  }
  if (visibility === "gm_only" && !isGm) {
    throw new GraphQLError("Only the GM can roll for their eyes only");
  }
}

/** `roll_label`: trimmed, empty is none, at most 80 characters. */
function labelFrom(value: unknown): string | null {
  const label = typeof value === "string" ? value.trim() : "";
  if (label === "") return null;
  if ([...label].length > MAX_LABEL) {
    throw new GraphQLError("A roll's label is at most 80 characters");
  }
  return label;
}

interface RecordOptions {
  /** The numbers put in for the formula's placeholders (spec 083). */
  bindings?: Record<string, number>;
  visibility?: Visibility;
  label?: string | null;
  outcome?: Row | null;
}

/**
 * One roll into the world's history, and the table told of it by id and
 * visibility only (spec 081 FR-001, FR-002) — as `roll_and_settle` and the
 * attack's `roll_and_record` both write it. `detail` is the crate's
 * `RollResolution`, as serde writes it.
 */
export function recordRoll(
  detail: unknown,
  {
    bindings = {},
    visibility = "everyone",
    label = null,
    outcome = null,
  }: RecordOptions = {},
): string {
  const state = demoState();
  const rolls = (state.rolls ??= []);
  const id = crypto.randomUUID();
  rolls.push({
    id,
    worldId: state.world.id,
    triggeredBy: viewerUser(state).id,
    detail,
    bindings,
    outcome,
    visibility,
    label: label === null ? null : [...label].slice(0, MAX_LABEL).join(""),
    createdAt: new Date().toISOString(),
    revealedAt: null,
    revealedBy: null,
  });
  rolls.splice(0, Math.max(0, rolls.length - KEPT));
  record(EVENT.rollMade, { rollId: id, visibility });
  markChanged();
  return id;
}

/**
 * `roll_dice_impl`: resolve, then record. A formula that fails to parse or
 * resolve rolls nothing and records nothing (FR-011), refused in the server's
 * words.
 */
export async function resolveAndRecord(
  formula: string,
  bindings: Record<string, number>,
  outcome: Row | null = null,
  options: Omit<RecordOptions, "outcome" | "bindings"> = {},
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
  recordRoll(detail, { ...options, bindings, outcome });
  return resolutionRow(detail, outcome);
}

const usernameOf = (id: unknown): string =>
  id === DEMO_PLAYER.id
    ? DEMO_PLAYER.username
    : id === DEMO_USER.id
      ? DEMO_USER.username
      : "";

/** `row_bindings`: a record's bindings, sorted; none before spec 083. */
function bindingsOf(record: Row): Row[] {
  const stored = (record.bindings ?? {}) as Record<string, number>;
  return Object.entries(stored)
    .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))
    .map(([placeholder, value]) => ({ placeholder, value }));
}

/**
 * `entry_for`: one roll as the viewer may see it, or nothing. The roller,
 * the GM and a revealed roll see it whole; another player sees a GM's eyes
 * roll masked and a GM only roll not at all (FR-003).
 */
function entryFor(record: Row, state: DemoState): Row | null {
  const visibility = visibilityOf(record);
  const whole =
    record.revealedAt != null ||
    visibility === "everyone" ||
    record.triggeredBy === viewerUser(state).id ||
    viewerIsGm(state);
  if (whole) {
    return {
      __typename: "WorldRoll",
      id: record.id,
      rollerId: record.triggeredBy,
      rollerName: usernameOf(record.triggeredBy),
      label: record.label ?? null,
      formula: (record.detail as Resolution).formula,
      bindings: bindingsOf(record),
      resolution: resolutionRow(
        record.detail as Resolution,
        (record.outcome as Row | null) ?? null,
      ),
      visibility: VISIBILITY_ENUM[visibility],
      createdAt: record.createdAt,
      revealedAt: record.revealedAt ?? null,
      revealedByName:
        record.revealedBy == null ? null : usernameOf(record.revealedBy),
    };
  }
  if (visibility === "gm_eyes") {
    return {
      __typename: "MaskedRoll",
      id: record.id,
      rollerName: usernameOf(record.triggeredBy),
      createdAt: record.createdAt,
      visibility: VISIBILITY_ENUM[visibility],
    };
  }
  return null;
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
  /** A roll hidden from the viewer answers as one that does not exist. */
  worldRoll: ({ rollId }: Args) => {
    const state = demoState();
    const found = (state.rolls ?? []).find((record) => record.id === rollId);
    return found ? entryFor(found, state) : null;
  },
  /** Newest first, before `before` (exclusive); hidden rolls left out. */
  worldRolls: ({ before, limit }: Args) => {
    const state = demoState();
    let cutoff: number | null = null;
    if (before != null) {
      cutoff = Date.parse(String(before));
      if (Number.isNaN(cutoff))
        throw new GraphQLError("`before` is not a time");
    }
    const take = Math.min(Math.max(limit ?? FEED_PAGE, 1), FEED_MOST);
    return [...(state.rolls ?? [])]
      .reverse()
      .filter(
        (record) =>
          cutoff === null || Date.parse(String(record.createdAt)) < cutoff,
      )
      .map((record) => entryFor(record, state))
      .filter((entry): entry is Row => entry !== null)
      .slice(0, take);
  },
};

export const diceMutations = {
  rollDice: ({ input }: Args) => {
    const visibility = visibilityFromInput(input.visibility);
    mayRoll(visibility, viewerIsGm(demoState()));
    const label = labelFrom(input.label);
    return resolveAndRecord(
      String(input.formula),
      Object.fromEntries(
        ((input.bindings ?? []) as Array<{ name: string; value: number }>).map(
          (b) => [b.name, b.value],
        ),
      ),
      null,
      { visibility, label },
    );
  },
  /**
   * `reveal_roll_impl`: who and when, the dice untouched, and the table told
   * to fetch it again. A roll already public answers as it is.
   */
  revealRoll: ({ rollId }: Args) => {
    const state = demoState();
    if (!viewerIsGm(state)) {
      throw new GraphQLError("Only the GM can reveal a roll");
    }
    const found = (state.rolls ?? []).find((record) => record.id === rollId);
    if (!found) throw new GraphQLError("Roll not found");
    const visibility = visibilityOf(found);
    if (visibility !== "everyone" && found.revealedAt == null) {
      found.revealedAt = new Date().toISOString();
      found.revealedBy = viewerUser(state).id;
      record(EVENT.rollRevealed, { rollId: found.id, visibility });
      markChanged();
    }
    return entryFor(found, state);
  },
};
