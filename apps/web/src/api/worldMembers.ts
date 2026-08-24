import { withCsrf } from "@/api/auth";

export interface WorldMemberRecord {
  id: string;
  userId: string;
  role: string;
  joinedAt: string;
  worldId?: string;
  createdAt?: string;
  updatedAt?: string;
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
 * Fetches a world's membership roster directly via GraphQL. RxDB has been
 * hard cut from this layer entirely (it was fatally broken app-wide — RxDB
 * 17.2.0 rejects the inline `index: true` schema shorthand used throughout,
 * error SC26 — and there was never a live GraphQL subscription transport
 * wired up client-side to make its "reactive" queries actually reactive
 * across tabs/sessions anyway). `useWorldMembers` now calls this function
 * directly. This mirrors `CampaignSettingsPanel.tsx`'s own direct-query
 * pattern for the same data.
 */
export function getWorldMembers(worldId: string): Promise<WorldMemberRecord[]> {
  return postGraphQL<WorldMembersQuery>(
    `
      query WorldMembers($worldId: ID!) {
        worldMembers(worldId: $worldId) {
          id
          worldId
          userId
          role
          joinedAt
          createdAt
          updatedAt
        }
      }
    `,
    { worldId },
  ).then((data) => data.worldMembers);
}
