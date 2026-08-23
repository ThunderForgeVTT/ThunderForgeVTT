import { withCsrf } from "@/api/auth";
import type { InventoryEntryRecord } from "@/types/inventory";

type GraphQLError = {
  message?: string;
};

type GraphQLResponse<TData> = {
  data?: TData;
  errors?: GraphQLError[];
};

const GRAPHQL_ENDPOINT = "/api/graphql";

const INVENTORY_ENTRY_FIELDS = `
  id
  actorId
  itemId
  itemName
  quantity
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

type ActorInventoryQuery = {
  actorInventory: InventoryEntryRecord[];
};

/** Requires at least Viewer on the actor (FR-013). */
export function getActorInventory(actorId: string): Promise<InventoryEntryRecord[]> {
  return postGraphQL<ActorInventoryQuery>(
    `
      query ActorInventory($actorId: UUID!) {
        actorInventory(actorId: $actorId) {
          ${INVENTORY_ENTRY_FIELDS}
        }
      }
    `,
    { actorId },
  ).then((data) => data.actorInventory);
}

type AddItemToInventoryMutation = {
  addItemToInventory: InventoryEntryRecord;
};

/** Requires Editor/Owner on the ACTOR, not the item (FR-013/Assumptions).
 * Repeated adds of the same item merge quantity into the existing entry. */
export function addItemToInventory(
  actorId: string,
  itemId: string,
  quantity: number,
): Promise<InventoryEntryRecord> {
  return postGraphQL<AddItemToInventoryMutation>(
    `
      mutation AddItemToInventory($input: AddItemToInventoryInput!) {
        addItemToInventory(input: $input) {
          ${INVENTORY_ENTRY_FIELDS}
        }
      }
    `,
    { input: { actorId, itemId, quantity } },
  ).then((data) => data.addItemToInventory);
}

type AdjustInventoryQuantityMutation = {
  adjustInventoryQuantity: InventoryEntryRecord | null;
};

/** Absolute new value, not a delta. A `null` result means the entry was
 * removed because quantity hit 0 (FR-011). */
export function adjustInventoryQuantity(
  inventoryEntryId: string,
  quantity: number,
): Promise<InventoryEntryRecord | null> {
  return postGraphQL<AdjustInventoryQuantityMutation>(
    `
      mutation AdjustInventoryQuantity($input: AdjustInventoryQuantityInput!) {
        adjustInventoryQuantity(input: $input) {
          ${INVENTORY_ENTRY_FIELDS}
        }
      }
    `,
    { input: { inventoryEntryId, quantity } },
  ).then((data) => data.adjustInventoryQuantity);
}

type RemoveInventoryEntryMutation = {
  removeInventoryEntry: boolean;
};

export function removeInventoryEntry(inventoryEntryId: string): Promise<boolean> {
  return postGraphQL<RemoveInventoryEntryMutation>(
    `
      mutation RemoveInventoryEntry($inventoryEntryId: UUID!) {
        removeInventoryEntry(inventoryEntryId: $inventoryEntryId)
      }
    `,
    { inventoryEntryId },
  ).then((data) => data.removeInventoryEntry);
}
