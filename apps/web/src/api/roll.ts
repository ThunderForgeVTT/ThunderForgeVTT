// Spec 014: GraphQL calls for the dice rolling engine
// (contracts/graphql-roll.md). `rollDice` is the sole way to produce an
// authoritative result — this module never computes or guesses one
// itself.

import { postGraphQL } from "@/api/graphqlClient";
import type {
  PlaceholderBinding,
  RollRecordRecord,
  RollResolutionRecord,
} from "@/types/roll";

const ROLL_RESOLUTION_FIELDS = `
  formula
  dice {
    sidesKind
    numericSides
    rolls
    kept
    finalValue
  }
  resultKind
  resultValue
`;

type RollDiceMutation = {
  rollDice: RollResolutionRecord;
};

/**
 * The sole way to produce an authoritative roll. Always re-resolves
 * server-side regardless of anything this client sends — there is no
 * field here that could express a pre-computed result (FR-001/FR-002).
 */
export function rollDice(
  worldId: string,
  formula: string,
  bindings?: PlaceholderBinding[],
): Promise<RollResolutionRecord> {
  return postGraphQL<RollDiceMutation>(
    `
      mutation RollDice($input: RollDiceInput!) {
        rollDice(input: $input) {
          ${ROLL_RESOLUTION_FIELDS}
        }
      }
    `,
    { input: { worldId, formula, bindings } },
  ).then((data) => data.rollDice);
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
