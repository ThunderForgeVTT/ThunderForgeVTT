import { GraphQLRequestError, postGraphQL } from "@/api/graphqlClient";
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

type PickUpPlacedItemMutation = {
  pickUpPlacedItem: InventoryEntryRecord;
};

/**
 * The extension code the server sets when an item was already taken.
 *
 * Spec 031 FR-016. Worth telling apart from every other failure: losing a race
 * is an ordinary thing that happens at a busy table, and reporting it as an
 * error the player did something wrong would be both wrong and discouraging.
 */
export const ALREADY_TAKEN = "ALREADY_TAKEN";

/**
 * Take a placed item off the map and into a character's inventory.
 *
 * Server-authoritative and all-or-nothing: the token is deleted and the entry
 * created in one transaction, and exactly one of two simultaneous callers wins
 * (spec 031 FR-015, FR-016). Chrome never removes the token itself — it waits
 * for the sync that follows the server's answer, which is what makes a refusal
 * cost nothing to recover from (FR-017).
 */
export function pickUpPlacedItem(
  tokenId: string,
  actorId: string,
): Promise<InventoryEntryRecord> {
  return postGraphQL<PickUpPlacedItemMutation>(
    `
      mutation PickUpPlacedItem($input: PickUpPlacedItemInput!) {
        pickUpPlacedItem(input: $input) {
          ${INVENTORY_ENTRY_FIELDS}
        }
      }
    `,
    { input: { tokenId, actorId } },
  ).then((data) => data.pickUpPlacedItem);
}

/**
 * Whether a failed pickup was somebody else being quicker.
 *
 * Matched on the server's `extensions.code`, not its message — the wording is
 * for a person and may change, the code is the contract.
 */
export function isAlreadyTaken(error: unknown): boolean {
  return error instanceof GraphQLRequestError && error.hasCode(ALREADY_TAKEN);
}
