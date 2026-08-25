import { withCsrf } from "@/api/auth";
import type { ActorAbilityEntryRecord } from "@/types/actorAbility";

/**
 * Spec 025 (T053): an actor's known abilities.
 *
 * Argument shapes match the resolvers in `mutations_actor_abilities.rs`:
 *   * `input:` object  → attachAbilityToActor
 *   * flat scalar args → actorAbilities, detachAbilityFromActor
 */

type GraphQLResponse<TData> = {
  data?: TData;
  errors?: { message?: string }[];
};

const GRAPHQL_ENDPOINT = "/api/graphql";

const ENTRY_FIELDS = `
  id
  actorId
  abilityId
  abilityName
  classification
  gmOnly
`;

async function postGraphQL<TData>(
  query: string,
  variables?: Record<string, unknown>,
): Promise<TData> {
  const response = await fetch(GRAPHQL_ENDPOINT, {
    method: "POST",
    credentials: "same-origin",
    headers: withCsrf({ "Content-Type": "application/json" }),
    body: JSON.stringify({ query, variables }),
  });

  const payload = (await response.json()) as GraphQLResponse<TData>;
  if (!response.ok) {
    throw new Error(payload.errors?.[0]?.message || "GraphQL request failed");
  }
  if (payload.errors?.length) {
    throw new Error(payload.errors[0]?.message || "GraphQL request failed");
  }
  if (!payload.data) {
    throw new Error("GraphQL response did not include data");
  }
  return payload.data;
}

/** Requires Viewer on the ACTOR. GM-only abilities are omitted server-side
 * for non-DMs — silently, with no inferable trace (FR-023, FR-024b). */
export function getActorAbilities(actorId: string): Promise<ActorAbilityEntryRecord[]> {
  return postGraphQL<{ actorAbilities: ActorAbilityEntryRecord[] }>(
    `
      query ActorAbilities($actorId: UUID!) {
        actorAbilities(actorId: $actorId) {
          ${ENTRY_FIELDS}
        }
      }
    `,
    { actorId },
  ).then((data) => data.actorAbilities);
}

/**
 * FR-022: permission is checked against the ACTOR, not the ability. Attaching
 * an already-known ability is a no-op that returns the existing entry, not an
 * error (FR-021).
 */
export function attachAbilityToActor(
  actorId: string,
  abilityId: string,
): Promise<ActorAbilityEntryRecord> {
  return postGraphQL<{ attachAbilityToActor: ActorAbilityEntryRecord }>(
    `
      mutation AttachAbilityToActor($input: AttachAbilityToActorInput!) {
        attachAbilityToActor(input: $input) {
          ${ENTRY_FIELDS}
        }
      }
    `,
    { input: { actorId, abilityId } },
  ).then((data) => data.attachAbilityToActor);
}

/** Removes the entry only — the ability itself is untouched (FR-023). */
export function detachAbilityFromActor(entryId: string): Promise<boolean> {
  return postGraphQL<{ detachAbilityFromActor: boolean }>(
    `
      mutation DetachAbilityFromActor($entryId: UUID!) {
        detachAbilityFromActor(entryId: $entryId)
      }
    `,
    { entryId },
  ).then((data) => data.detachAbilityFromActor);
}
