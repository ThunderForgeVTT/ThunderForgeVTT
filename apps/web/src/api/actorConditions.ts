/**
 * The conditions a world's game system declares, and putting a character
 * under one (spec 067 Story 4).
 *
 * What a character *is* under is not read here: it arrives on the
 * character's tokens (`TokenRecord.conditions`), so it reaches exactly the
 * seats those tokens reach.
 */
import { postGraphQL } from "@/api/graphqlClient";
import type { TokenCondition } from "@/types/token";

/** One condition as its system declares it. */
export interface WorldSystemCondition {
  id: string;
  label: string;
  description: string | null;
  /** The marker's shape, from the host's list. */
  glyph: string;
  /** The marker's colour, as a token. */
  color: string;
}

const WORLD_SYSTEM_CONDITIONS_QUERY = `
  query WorldSystemConditions($worldId: UUID!) {
    worldSystemConditions(worldId: $worldId) {
      id
      label
      description
      glyph
      color
    }
  }
`;

const APPLY_ACTOR_CONDITION_MUTATION = `
  mutation ApplyActorCondition($actorId: UUID!, $conditionId: String!) {
    applyActorCondition(actorId: $actorId, conditionId: $conditionId) {
      id
      glyph
      color
    }
  }
`;

const CLEAR_ACTOR_CONDITION_MUTATION = `
  mutation ClearActorCondition($actorId: UUID!, $conditionId: String!) {
    clearActorCondition(actorId: $actorId, conditionId: $conditionId) {
      id
      glyph
      color
    }
  }
`;

/** Every condition the world's system declares, in its own order. */
export async function getWorldSystemConditions(
  worldId: string,
): Promise<WorldSystemCondition[]> {
  const { worldSystemConditions } = await postGraphQL<{
    worldSystemConditions: WorldSystemCondition[];
  }>(WORLD_SYSTEM_CONDITIONS_QUERY, { worldId });
  return worldSystemConditions;
}

/**
 * Put a character under a condition. Game Masters only. Answers with every
 * condition the character is now under.
 */
export async function applyActorCondition(
  actorId: string,
  conditionId: string,
): Promise<TokenCondition[]> {
  const { applyActorCondition: held } = await postGraphQL<{
    applyActorCondition: TokenCondition[];
  }>(APPLY_ACTOR_CONDITION_MUTATION, { actorId, conditionId });
  return held;
}

/**
 * Lift a condition from a character. Game Masters only. Answers with every
 * condition the character is still under.
 */
export async function clearActorCondition(
  actorId: string,
  conditionId: string,
): Promise<TokenCondition[]> {
  const { clearActorCondition: held } = await postGraphQL<{
    clearActorCondition: TokenCondition[];
  }>(CLEAR_ACTOR_CONDITION_MUTATION, { actorId, conditionId });
  return held;
}
