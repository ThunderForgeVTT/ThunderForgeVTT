import { withCsrf } from "@/api/auth";

export interface WorldMemberRecord {
  id: string;
  userId: string;
  role: string;
  joinedAt: string;
}

type GraphQLError = {
  message?: string;
};

type GraphQLResponse<TData> = {
  data?: TData;
  errors?: GraphQLError[];
};

const GRAPHQL_ENDPOINT = "/api/graphql";

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

type WorldMembersQuery = {
  worldMembers: WorldMemberRecord[];
};

/**
 * Fetches a world's membership roster directly via GraphQL. NOTE: this
 * deliberately does not use the `useWorldMembers`/RxDB path — that
 * replication is wired through `services/replication/inviteReplicationHandler.ts`,
 * whose `setupInviteReplication`/`fetchInitialData` are never actually
 * invoked anywhere in the app (and, independently, its GraphQL queries
 * use snake_case field/argument names against a schema that is entirely
 * camelCase, so they would fail even if wired up) — so that collection is
 * always empty today. This function mirrors `CampaignSettingsPanel.tsx`'s
 * own direct-query workaround for the same reason.
 */
export function getWorldMembers(worldId: string): Promise<WorldMemberRecord[]> {
  return postGraphQL<WorldMembersQuery>(
    `
      query WorldMembers($worldId: ID!) {
        worldMembers(worldId: $worldId) {
          id
          userId
          role
          joinedAt
        }
      }
    `,
    { worldId },
  ).then((data) => data.worldMembers);
}
