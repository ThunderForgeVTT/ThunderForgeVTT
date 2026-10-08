import { postGraphQL } from "@/api/graphqlClient";
import type { Advantage } from "@/types/roll";

/**
 * Spec 036 US3b: what this world's system says a character can be asked to
 * roll, and the one call that rolls it.
 *
 * # What the client is deliberately not told
 *
 * The formula, and the bindings. They are in the pack's manifest and are no
 * secret, but publishing them *through this API* would invite a client to
 * compute the roll it is about to ask for and then notice it need not ask.
 * ADR-044 makes the server the only party that may produce a result;
 * `rollCheck` takes a world, an actor and a check id, and there is no argument
 * through which dice, a value or a verdict could be smuggled in.
 *
 * # An empty list is an answer
 *
 * A system that declares no checks offers no button (FR-037). Seven of the
 * eight bundled packs are that case. It is a fact about the ruleset, not a
 * failure to be papered over with an invented d20.
 */

/** Enough to draw a button, and no more. */
export interface SystemCheck {
  id: string;
  label: string;
  /** The set it belongs to — "abilities", "skills" — when it is in one. */
  group: string | null;
}

/**
 * What came back. `resultKind` is the system's, not ours: a d20 total and a
 * Year Zero success count are different kinds of answer, and flattening them
 * into "the number" would put one system's reading of a roll into shared code.
 */
export interface CheckResolution {
  formula: string;
  resultKind: "TOTAL" | "SUCCESS_COUNT";
  resultValue: number;
  dice: { finalValue: number; kept: boolean }[];
}

/**
 * The checks, and whether this system takes Normal, Advantage or
 * Disadvantage on them (spec 084 FR-018) — asked together because the sheet
 * draws both at once.
 */
export interface SystemChecks {
  checks: SystemCheck[];
  rollsWithAdvantage: boolean;
}

const CHECKS_QUERY = `
  query SystemChecks($worldId: UUID!) {
    systemChecks(worldId: $worldId) { id label group }
    rollsWithAdvantage(worldId: $worldId)
  }
`;

export async function getSystemChecks(worldId: string): Promise<SystemChecks> {
  const data = await postGraphQL<{
    systemChecks: SystemCheck[];
    rollsWithAdvantage: boolean;
  }>(CHECKS_QUERY, { worldId });
  return {
    checks: data.systemChecks,
    rollsWithAdvantage: data.rollsWithAdvantage,
  };
}

const ROLLS_WITH_ADVANTAGE = `
  query RollsWithAdvantage($worldId: UUID!) {
    rollsWithAdvantage(worldId: $worldId)
  }
`;

/** Spec 084: whether the attack flow offers the advantage picker. */
export async function getRollsWithAdvantage(worldId: string): Promise<boolean> {
  const data = await postGraphQL<{ rollsWithAdvantage: boolean }>(
    ROLLS_WITH_ADVANTAGE,
    { worldId },
  );
  return data.rollsWithAdvantage;
}

const ROLL_CHECK = `
  mutation RollCheck(
    $worldId: UUID!
    $actorId: UUID!
    $checkId: String!
    $advantage: Advantage
  ) {
    rollCheck(
      worldId: $worldId
      actorId: $actorId
      checkId: $checkId
      advantage: $advantage
    ) {
      formula
      resultKind
      resultValue
      dice { finalValue kept }
    }
  }
`;

export async function rollCheck(input: {
  worldId: string;
  actorId: string;
  checkId: string;
  /** Spec 084: how the check's d20 is rolled; Normal when left out. */
  advantage?: Advantage;
}): Promise<CheckResolution> {
  const data = await postGraphQL<{ rollCheck: CheckResolution }>(ROLL_CHECK, {
    worldId: input.worldId,
    actorId: input.actorId,
    checkId: input.checkId,
    advantage: input.advantage ?? "NORMAL",
  });
  return data.rollCheck;
}
