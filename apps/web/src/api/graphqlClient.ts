import { withCsrf } from "@/api/auth";

/**
 * Spec 025 (T097): the one GraphQL POST helper.
 *
 * Before this, `postGraphQL` was privately re-declared in **23** files under
 * `src/api/`, in 7 subtly different variants — differing in error precedence,
 * whether a missing `data` was treated as an error, and message fallbacks.
 * That divergence is the actual hazard, more than the repetition.
 *
 * ⚠️ **Only spec 025's own three modules use this so far**
 * (`abilities.ts`, `abilityShares.ts`, `actorAbilities.ts`). Migrating the
 * other 20 touches every API surface in the app, and `apps/web` has almost no
 * test coverage to catch a behavioural difference between variants — so that
 * is deliberately left as its own change with its own verification, rather
 * than tacked onto the end of a feature. New API modules should use this.
 */

const GRAPHQL_ENDPOINT = "/api/graphql";

type GraphQLResponse<TData> = {
  data?: TData;
  errors?: { message?: string }[];
};

export async function postGraphQL<TData>(
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

  // A GraphQL error message beats the bare HTTP status: the server returns a
  // useful message in `errors` even on a non-2xx, and surfacing "GraphQL
  // request failed" over it loses the only actionable detail.
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
