import { withCsrf } from "@/api/auth";
import type { WorldActorRecord } from "@/types/actor";

type GraphQLError = {
  message?: string;
};

type GraphQLResponse<TData> = {
  data?: TData;
  errors?: GraphQLError[];
};

const GRAPHQL_ENDPOINT = "/api/graphql";

const WORLD_ACTOR_FIELDS = `
  id
  worldId
  sceneId
  actorType
  gameSystemId
  label
  isPublic
  isNpc
  createdBy
  ownedBy
  createdAt
  updatedAt
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
    body: JSON.stringify({
      query,
      variables,
    }),
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

type WorldActorsQuery = {
  worldActors: WorldActorRecord[];
};

/**
 * Fetch every actor (NPCs and player characters, distinguished by `isNpc`)
 * in a world, across every scene — used by the GM staging page and the
 * full-screen sidebar's NPC/combat roster (spec 009).
 */
export function getWorldActors(worldId: string): Promise<WorldActorRecord[]> {
  return postGraphQL<WorldActorsQuery>(
    `
      query WorldActors($worldId: UUID!) {
        worldActors(worldId: $worldId) {
          ${WORLD_ACTOR_FIELDS}
        }
      }
    `,
    { worldId },
  ).then((data) => data.worldActors);
}
