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
import init, {
  initSync,
  lowestDie,
  replayRoll,
  roll,
  validateFormula,
} from "@thunderforge/dice";
import { GraphQLError } from "graphql";

import { facetRows, rerollOffers, rerollPlan, type RollKind } from "./facets";
import { DEMO_PLAYER, DEMO_USER } from "../../seed/world";
import { mayActFor, systemDataOf, viewerIsGm, viewerUser } from "../actors";
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
  /** Spec 084: whose roll it was, what for, and what shaped it. */
  meta?: RollMeta;
}

/** Spec 084: `RollMeta`, as `RollMeta::write_to` writes it on the record. */
export interface RollMeta {
  actorId?: string | null;
  rollKind?: RollKind | null;
  checkId?: string | null;
  facets?: string[];
  /** The roll this one rerolled, and what was spent to do it. */
  rerollOf?: string | null;
  rerollSpent?: string | null;
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
    meta = {},
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
    actorId: meta.actorId ?? null,
    rollKind: meta.rollKind ?? null,
    checkId: meta.checkId ?? null,
    facets: meta.facets ?? [],
    rerollOf: meta.rerollOf ?? null,
    rerollSpent: meta.rerollSpent ?? null,
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
/** `reroll::REROLL_WINDOW`: how long a roll's maker may reroll it. */
const REROLL_WINDOW_MS = 120_000;

const ONLY_MAKER = "Only the person who made a roll may reroll it.";
const NO_LONGER_ACTS = "You can no longer act for this character.";
const NOT_A_D20_TEST = "Only a d20 test can be rerolled.";
const ALREADY_REROLLED = "This roll has already been rerolled.";
const TOO_LATE = "It is too late to reroll this roll.";

const isD20Test = (record: Row) =>
  record.rollKind === "check" || record.rollKind === "to_hit";

/** `rerolled_by`: the roll that replaced this one, if any. */
function rerolledBy(record: Row, state: DemoState): Row | null {
  return (state.rolls ?? []).find((r) => r.rerollOf === record.id) ?? null;
}

/** `chain_of`: the roll and every roll it rerolled, newest first. */
function chainOf(record: Row, state: DemoState): Row[] {
  const chain = [record];
  let at = record;
  while (at.rerollOf != null) {
    const before = (state.rolls ?? []).find((r) => r.id === at.rerollOf);
    if (!before) break;
    chain.push(before);
    at = before;
  }
  return chain;
}

/** `whole_chain`: oldest first, the rolls rerolled and the rerolls after. */
function wholeChain(record: Row, state: DemoState): Row[] {
  const chain = chainOf(record, state).reverse();
  let next = rerolledBy(record, state);
  while (next) {
    chain.push(next);
    next = rerolledBy(next, state);
  }
  return chain;
}

/** `spent_in`: every facet spent along the chain. */
function spentIn(record: Row, state: DemoState): string[] {
  return chainOf(record, state)
    .map((r) => r.rerollSpent as string | null)
    .filter((id): id is string => id != null);
}

/** `reroll_until`: when the window shuts, for a d20 test not yet rerolled. */
function rerollUntil(record: Row, state: DemoState): number | null {
  if (record.actorId == null || !isD20Test(record)) return null;
  if (rerolledBy(record, state)) return null;
  return Date.parse(String(record.createdAt)) + REROLL_WINDOW_MS;
}

/** `may_reroll`, refused in the server's order; the facets come after. */
function mayReroll(record: Row | undefined, state: DemoState): Row {
  if (
    !record ||
    record.actorId == null ||
    record.triggeredBy !== viewerUser(state).id
  ) {
    throw new GraphQLError(ONLY_MAKER);
  }
  const actor = state.actors.find((a) => a.id === record.actorId);
  if (!actor || !mayActFor(state, actor)) {
    throw new GraphQLError(NO_LONGER_ACTS);
  }
  // A to_hit roll waits for the attack reroll (spec 084 T057).
  if (record.rollKind !== "check") throw new GraphQLError(NOT_A_D20_TEST);
  if (rerolledBy(record, state)) throw new GraphQLError(ALREADY_REROLLED);
  if (Date.now() >= (rerollUntil(record, state) ?? 0)) {
    throw new GraphQLError(TOO_LATE);
  }
  return actor;
}

/** `offers_from_db`: what the viewer may spend on this roll now. */
function offersFor(record: Row, state: DemoState): string[] {
  let actor: Row;
  try {
    actor = mayReroll(record, state);
  } catch {
    return [];
  }
  return rerollOffers(
    systemDataOf(state, actor.id as string)?.traitData,
    rerollTarget(record),
    spentIn(record, state),
  );
}

/** What the pack is asked to plan a reroll against. */
function rerollTarget(record: Row) {
  return {
    kind: record.rollKind,
    formula: (record.detail as Resolution).formula,
    facets: (record.facets as string[] | undefined) ?? [],
  };
}

function entryFor(record: Row, state: DemoState): Row | null {
  const visibility = visibilityOf(record);
  const whole =
    record.revealedAt != null ||
    visibility === "everyone" ||
    record.triggeredBy === viewerUser(state).id ||
    viewerIsGm(state);
  if (whole) {
    const until = rerollUntil(record, state);
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
      facets: facetRows((record.facets as string[] | undefined) ?? []),
      rerollOf: record.rerollOf ?? null,
      rerolledBy: rerolledBy(record, state)?.id ?? null,
      spent:
        record.rerollSpent == null
          ? null
          : facetRows([record.rerollSpent as string])[0],
      rerollOffers: facetRows(offersFor(record, state)),
      rerollUntil: until === null ? null : new Date(until).toISOString(),
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
    // Spec 084: a reroll and the roll it replaced are revealed together.
    for (const link of wholeChain(found, state)) {
      const visibility = visibilityOf(link);
      if (visibility !== "everyone" && link.revealedAt == null) {
        link.revealedAt = new Date().toISOString();
        link.revealedBy = viewerUser(state).id;
        record(EVENT.rollRevealed, { rollId: link.id, visibility });
        markChanged();
      }
    }
    return entryFor(found, state);
  },
  /**
   * Spec 084 `reroll_roll_impl`: the maker spends a facet to roll their own
   * d20 test again: the lowest d20 once more (Heroic Inspiration), or the
   * whole roll through a reshaped formula that keeps its dice (a Luck Point);
   * the new roll keeps the old one's visibility, label and actor.
   */
  rerollRoll: async ({ rollId, spend }: Args) => {
    await diceReady();
    const state = demoState();
    const found = (state.rolls ?? []).find((r) => r.id === rollId);
    const actor = mayReroll(found, state);
    const original = found!;
    const sheet = systemDataOf(state, actor.id as string);
    const plan = rerollPlan(
      sheet?.traitData,
      String(spend),
      String(actor.label),
      spentIn(original, state),
      rerollTarget(original),
    );
    if (sheet) sheet.traitData = plan.traitData;
    record(EVENT.actorSheet, {
      action: "changed",
      actorId: actor.id,
      dataType: "trait_data",
    });
    const detail = original.detail as Resolution;
    const bindings = (original.bindings ?? {}) as Record<string, number>;
    const { edit } = plan;
    const replayed = JSON.parse(
      replayRoll(
        detail.formula,
        JSON.stringify(bindings),
        JSON.stringify(detail),
        edit.kind === "reshape" ? edit.formula : undefined,
        edit.kind === "lowest"
          ? lowestDie(JSON.stringify(detail), edit.sides)
          : undefined,
        nextSeed(),
      ),
    ) as Resolution;
    const id = recordRoll(replayed, {
      bindings,
      visibility: visibilityOf(original),
      label: (original.label as string | null) ?? null,
      outcome: (original.outcome as Row | null) ?? null,
      meta: {
        actorId: actor.id as string,
        rollKind: original.rollKind as RollKind,
        checkId: (original.checkId as string | null) ?? null,
        facets: [
          ...((original.facets as string[] | undefined) ?? []),
          String(spend),
        ],
        rerollOf: original.id as string,
        rerollSpent: String(spend),
      },
    });
    return entryFor(state.rolls!.find((r) => r.id === id)!, state);
  },
};
