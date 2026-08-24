import { withCsrf } from "@/api/auth";

/**
 * Read counterpart to `updateActorSystemData` (spec 018 fix): the
 * `actorSystemData(actorId)` GraphQL query — see
 * src/server/src/graphql/queries/actor.rs's `actor_system_data_impl` for
 * why this was missing and what it's a fix for (no query path ever existed
 * to read an actor's ability/resource/proficiency/trait_data back, only a
 * mutation to write it; the client-side RxDB collection that was supposed
 * to mirror it has no replication registered — a separate, larger,
 * still-open gap this query sidesteps for direct on-demand reads).
 */

type GraphQLError = {
  message?: string;
};

type GraphQLResponse<TData> = {
  data?: TData;
  errors?: GraphQLError[];
};

const GRAPHQL_ENDPOINT = "/api/graphql";

export interface ActorSystemDataRecord {
  id: string;
  actorId: string;
  gameSystemId: string;
  abilityData: Record<string, unknown> | null;
  resourceData: Record<string, unknown> | null;
  proficiencyData: Record<string, unknown> | null;
  traitData: Record<string, unknown> | null;
  spellData: Record<string, unknown> | null;
  createdAt: string;
  updatedAt: string;
}

const ACTOR_SYSTEM_DATA_QUERY = `
  query ActorSystemData($actorId: UUID!) {
    actorSystemData(actorId: $actorId) {
      id
      actorId
      gameSystemId
      abilityData
      resourceData
      proficiencyData
      traitData
      spellData
      createdAt
      updatedAt
    }
  }
`;

async function postGraphQL<TData>(
  query: string,
  variables?: Record<string, unknown>,
): Promise<TData> {
  const response = await fetch(GRAPHQL_ENDPOINT, {
    method: "POST",
    credentials: "same-origin",
    headers: withCsrf({
      "Content-Type": "application/json",
    }),
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

/** Fetches an actor's system data directly (bypassing RxDB entirely — see
 * file header). Returns `null` if the actor has no system data row yet. */
export async function fetchActorSystemData(
  actorId: string,
): Promise<ActorSystemDataRecord | null> {
  const data = await postGraphQL<{ actorSystemData: ActorSystemDataRecord | null }>(
    ACTOR_SYSTEM_DATA_QUERY,
    { actorId },
  );
  return data.actorSystemData;
}

export type ActorSystemDataType =
  | "ability_data"
  | "resource_data"
  | "proficiency_data"
  | "trait_data"
  | "spell_data";

const UPDATE_ACTOR_SYSTEM_DATA_MUTATION = `
  mutation UpdateActorSystemData($input: GraphQLUpdateActorSystemDataInput!) {
    updateActorSystemData(input: $input) {
      id
      actorId
      gameSystemId
      abilityData
      resourceData
      proficiencyData
      traitData
      spellData
      createdAt
      updatedAt
    }
  }
`;

/** Writes one JSONB column of an actor's system data (RxDB hard-cut: this is
 * now the only write path — no local optimistic collection is involved). */
export async function updateActorSystemData(
  actorId: string,
  gameSystemId: string,
  dataType: ActorSystemDataType,
  data: Record<string, unknown>,
): Promise<ActorSystemDataRecord> {
  const result = await postGraphQL<{ updateActorSystemData: ActorSystemDataRecord }>(
    UPDATE_ACTOR_SYSTEM_DATA_MUTATION,
    { input: { actorId, gameSystemId, dataType, data } },
  );
  return result.updateActorSystemData;
}
