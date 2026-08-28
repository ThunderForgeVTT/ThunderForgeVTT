import { postGraphQL } from "@/api/graphqlClient";
import type { InventoryEntryRecord } from "@/types/inventory";

const INVENTORY_ENTRY_FIELDS = `
  id
  actorId
  itemId
  itemName
  quantity
`;

type ActorInventoryQuery = {
  actorInventory: InventoryEntryRecord[];
};

/** Requires at least Viewer on the actor (FR-013). */
export function getActorInventory(
  actorId: string,
): Promise<InventoryEntryRecord[]> {
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

export function removeInventoryEntry(
  inventoryEntryId: string,
): Promise<boolean> {
  return postGraphQL<RemoveInventoryEntryMutation>(
    `
      mutation RemoveInventoryEntry($inventoryEntryId: UUID!) {
        removeInventoryEntry(inventoryEntryId: $inventoryEntryId)
      }
    `,
    { inventoryEntryId },
  ).then((data) => data.removeInventoryEntry);
}
