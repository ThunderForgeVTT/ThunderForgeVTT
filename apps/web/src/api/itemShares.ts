import { withCsrf } from "@/api/auth";
import type { DmWorldSummary, ItemShareLinkRecord, SharedItemPreview } from "@/types/itemShare";
import type { WorldItemRecord } from "@/types/item";

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

type CreateItemShareLinkMutation = {
  createItemShareLink: ItemShareLinkRecord;
};

/** Requires effective Owner on the item (FR-022). */
export function createItemShareLink(itemId: string): Promise<ItemShareLinkRecord> {
  return postGraphQL<CreateItemShareLinkMutation>(
    `
      mutation CreateItemShareLink($itemId: UUID!) {
        createItemShareLink(itemId: $itemId) {
          id
          itemId
          shareCode
          revoked
          createdAt
        }
      }
    `,
    { itemId },
  ).then((data) => data.createItemShareLink);
}

type RevokeItemShareLinkMutation = {
  revokeItemShareLink: boolean;
};

/** The link's creator or the world's DM (FR-027). */
export function revokeItemShareLink(shareId: string): Promise<boolean> {
  return postGraphQL<RevokeItemShareLinkMutation>(
    `
      mutation RevokeItemShareLink($shareId: UUID!) {
        revokeItemShareLink(shareId: $shareId)
      }
    `,
    { shareId },
  ).then((data) => data.revokeItemShareLink);
}

type SharedItemQuery = {
  sharedItem: SharedItemPreview;
};

/** Authenticated-only, world-identity-scrubbed (mirrors sharedActor). */
export function getSharedItem(shareCode: string): Promise<SharedItemPreview> {
  return postGraphQL<SharedItemQuery>(
    `
      query SharedItem($shareCode: String!) {
        sharedItem(shareCode: $shareCode) {
          name
          description
          iconAssetId
          effects {
            id
            itemId
            effectType
            formula
            target
            triggerKind
            sortOrder
          }
        }
      }
    `,
    { shareCode },
  ).then((data) => data.sharedItem);
}

type MyDmWorldsQuery = {
  myDmWorlds: DmWorldSummary[];
};

/** Reuses the existing world-type-agnostic myDmWorlds query as-is
 * (research.md §5) — no item-specific variant needed. */
export function getMyDmWorlds(): Promise<DmWorldSummary[]> {
  return postGraphQL<MyDmWorldsQuery>(
    `
      query MyDmWorlds {
        myDmWorlds {
          id
          name
        }
      }
    `,
  ).then((data) => data.myDmWorlds);
}

type CopySharedItemMutation = {
  copySharedItemToWorld: WorldItemRecord;
};

/** Re-verified server-side regardless of what myDmWorlds returned earlier. */
export function copySharedItemToWorld(
  shareCode: string,
  destinationWorldId: string,
): Promise<WorldItemRecord> {
  return postGraphQL<CopySharedItemMutation>(
    `
      mutation CopySharedItemToWorld($input: CopySharedItemInput!) {
        copySharedItemToWorld(input: $input) {
          id
          worldId
          name
          description
          iconAssetId
          effects {
            id
            itemId
            effectType
            formula
            target
            triggerKind
            sortOrder
          }
          myPermissionLevel
          createdAt
          updatedAt
        }
      }
    `,
    { input: { shareCode, destinationWorldId } },
  ).then((data) => data.copySharedItemToWorld);
}
