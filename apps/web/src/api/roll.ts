// Spec 014: GraphQL calls for the dice rolling engine
// (contracts/graphql-roll.md). `rollDice` is the sole way to produce an
// authoritative result — this module never computes or guesses one
// itself.

import { postGraphQL } from "@/api/graphqlClient";
import type {
  PlaceholderBinding,
  RollRecordRecord,
  RollResolutionRecord,
  RollVisibility,
  WorldRollEntry,
  WorldRollRecord,
} from "@/types/roll";

const ROLL_RESOLUTION_FIELDS = `
  formula
  dice {
    sidesKind
    numericSides
    rolls
    steps
    kept
    finalValue
  }
  resultKind
  resultValue
`;

type RollDiceMutation = {
  rollDice: RollResolutionRecord;
};

/** What a roll is for and who sees it (spec 081). */
export interface RollOptions {
  bindings?: PlaceholderBinding[];
  /** `EVERYONE` when left out. */
  visibility?: RollVisibility;
  /** What the roll was for — "Stealth", "Longsword". */
  label?: string;
}

/**
 * The sole way to produce an authoritative roll. Always re-resolves
 * server-side regardless of anything this client sends — there is no
 * field here that could express a pre-computed result (FR-001/FR-002).
 *
 * Spec 081: the server announces the roll to the table; every board,
 * this one included, animates it from that announcement.
 */
export function rollDice(
  worldId: string,
  formula: string,
  { bindings, visibility, label }: RollOptions = {},
): Promise<RollResolutionRecord> {
  return postGraphQL<RollDiceMutation>(
    `
      mutation RollDice($input: RollDiceInput!) {
        rollDice(input: $input) {
          ${ROLL_RESOLUTION_FIELDS}
        }
      }
    `,
    { input: { worldId, formula, bindings, visibility, label } },
  ).then((data) => data.rollDice);
}

const WORLD_ROLL_FIELDS = `
  __typename
  id
  rollerId
  rollerName
  label
  formula
  bindings {
    placeholder
    value
  }
  resolution {
    ${ROLL_RESOLUTION_FIELDS}
  }
  visibility
  createdAt
  revealedAt
  revealedByName
  facets {
    id
    label
  }
  rerollOf
  rerolledBy
  spent {
    id
    label
  }
  rerollOffers {
    id
    label
  }
  rerollUntil
`;

const WORLD_ROLL_ENTRY_FIELDS = `
  __typename
  ... on WorldRoll {
    ${WORLD_ROLL_FIELDS}
  }
  ... on MaskedRoll {
    id
    rollerName
    createdAt
    visibility
  }
`;

/**
 * One roll as this viewer may see it: whole, masked, or `null` when it is
 * hidden from them or does not exist — the two answer alike.
 */
export function fetchWorldRoll(
  worldId: string,
  rollId: string,
): Promise<WorldRollEntry | null> {
  return postGraphQL<{ worldRoll: WorldRollEntry | null }>(
    `
      query WorldRoll($worldId: UUID!, $rollId: UUID!) {
        worldRoll(worldId: $worldId, rollId: $rollId) {
          ${WORLD_ROLL_ENTRY_FIELDS}
        }
      }
    `,
    { worldId, rollId },
  ).then((data) => data.worldRoll);
}

/** The table's rolls, newest first, older than `before` when given. */
export function fetchWorldRolls(
  worldId: string,
  options: { before?: string; limit?: number } = {},
): Promise<WorldRollEntry[]> {
  return postGraphQL<{ worldRolls: WorldRollEntry[] }>(
    `
      query WorldRolls($worldId: UUID!, $before: String, $limit: Int) {
        worldRolls(worldId: $worldId, before: $before, limit: $limit) {
          ${WORLD_ROLL_ENTRY_FIELDS}
        }
      }
    `,
    { worldId, before: options.before, limit: options.limit },
  ).then((data) => data.worldRolls);
}

/** GM only: show a hidden roll to the table. Revealing twice is harmless. */
export function revealRoll(
  worldId: string,
  rollId: string,
): Promise<WorldRollRecord> {
  return postGraphQL<{ revealRoll: WorldRollRecord }>(
    `
      mutation RevealRoll($worldId: UUID!, $rollId: UUID!) {
        revealRoll(worldId: $worldId, rollId: $rollId) {
          ${WORLD_ROLL_FIELDS}
        }
      }
    `,
    { worldId, rollId },
  ).then((data) => data.revealRoll);
}

/**
 * Spec 088: for those who run the world, clear the roll feed for everyone.
 * The rolls are kept in the world's record; every feed drops those made at
 * or before the answered time, this one by the event like the rest.
 */
export function clearWorldRolls(worldId: string): Promise<string> {
  return postGraphQL<{ clearWorldRolls: { clearedAt: string } }>(
    `
      mutation ClearWorldRolls($worldId: UUID!) {
        clearWorldRolls(worldId: $worldId) {
          clearedAt
        }
      }
    `,
    { worldId },
  ).then((data) => data.clearWorldRolls.clearedAt);
}

/**
 * Spec 084: spend `spend` (an id from the roll's `rerollOffers`) to roll
 * one of your own d20 tests again. The server answers with the new roll, or
 * refuses in a sentence meant for the person who asked.
 */
export function rerollRoll(
  worldId: string,
  rollId: string,
  spend: string,
): Promise<WorldRollRecord> {
  return postGraphQL<{ rerollRoll: WorldRollRecord }>(
    `
      mutation RerollRoll($worldId: UUID!, $rollId: UUID!, $spend: String!) {
        rerollRoll(worldId: $worldId, rollId: $rollId, spend: $spend) {
          ${WORLD_ROLL_FIELDS}
        }
      }
    `,
    { worldId, rollId, spend },
  ).then((data) => data.rerollRoll);
}

type WorldRollRecordsQuery = {
  worldRollRecords: RollRecordRecord[];
};

/** DM-only (FR-014's stated floor). */
export function getWorldRollRecords(
  worldId: string,
  limit?: number,
): Promise<RollRecordRecord[]> {
  return postGraphQL<WorldRollRecordsQuery>(
    `
      query WorldRollRecords($worldId: UUID!, $limit: Int) {
        worldRollRecords(worldId: $worldId, limit: $limit) {
          id
          worldId
          triggeredBy
          resolution {
            ${ROLL_RESOLUTION_FIELDS}
          }
          createdAt
        }
      }
    `,
    { worldId, limit },
  ).then((data) => data.worldRollRecords);
}

type ValidateDiceFormulaQuery = {
  validateDiceFormula: boolean;
};

/** Pure parse-only check — no evaluation, no RNG, no persistence. */
export function validateDiceFormula(formula: string): Promise<boolean> {
  return postGraphQL<ValidateDiceFormulaQuery>(
    `
      query ValidateDiceFormula($formula: String!) {
        validateDiceFormula(formula: $formula)
      }
    `,
    { formula },
  ).then((data) => data.validateDiceFormula);
}
