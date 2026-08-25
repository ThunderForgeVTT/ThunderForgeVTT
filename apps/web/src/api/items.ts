import { postGraphQL } from "@/api/graphqlClient";
import type { ItemEffectType, ItemEffectTrigger, ItemPermissionRecord, WorldItemRecord } from "@/types/item";

const ITEM_EFFECT_FIELDS = `
  id
  itemId
  effectType
  formula
  target
  triggerKind
  sortOrder
`;

const WORLD_ITEM_FIELDS = `
  id
  worldId
  name
  description
  iconAssetId
  effects {
    ${ITEM_EFFECT_FIELDS}
  }
  myPermissionLevel
  moderated
  moderationCaseId
  createdAt
  updatedAt
  linkedFromLore {
    id
    title
    slug
  }
`;

type WorldItemsQuery = {
  worldItems: WorldItemRecord[];
};

/** Every world member sees every item at at least Viewer level by default (FR-008). */
export function getWorldItems(worldId: string, search?: string): Promise<WorldItemRecord[]> {
  return postGraphQL<WorldItemsQuery>(
    `
      query WorldItems($worldId: UUID!, $search: String) {
        worldItems(worldId: $worldId, search: $search) {
          ${WORLD_ITEM_FIELDS}
        }
      }
    `,
    { worldId, search },
  ).then((data) => data.worldItems);
}

type ItemQuery = {
  item: WorldItemRecord;
};

export function getItem(itemId: string): Promise<WorldItemRecord> {
  return postGraphQL<ItemQuery>(
    `
      query Item($itemId: UUID!) {
        item(itemId: $itemId) {
          ${WORLD_ITEM_FIELDS}
        }
      }
    `,
    { itemId },
  ).then((data) => data.item);
}

type SuggestItemNameQuery = {
  suggestItemName: WorldItemRecord[];
};

/** Non-blocking "did you mean?" nudge (FR-020) — never gates createItem. */
export function suggestItemName(worldId: string, name: string): Promise<WorldItemRecord[]> {
  return postGraphQL<SuggestItemNameQuery>(
    `
      query SuggestItemName($worldId: UUID!, $name: String!) {
        suggestItemName(worldId: $worldId, name: $name) {
          ${WORLD_ITEM_FIELDS}
        }
      }
    `,
    { worldId, name },
  ).then((data) => data.suggestItemName);
}

type CreateItemMutation = {
  createItem: WorldItemRecord;
};

/** DM-only (FR-002); description/icon optional (Clarifications). */
export function createItem(input: {
  worldId: string;
  name: string;
  description?: string | null;
}): Promise<WorldItemRecord> {
  return postGraphQL<CreateItemMutation>(
    `
      mutation CreateItem($input: CreateItemInput!) {
        createItem(input: $input) {
          ${WORLD_ITEM_FIELDS}
        }
      }
    `,
    { input },
  ).then((data) => data.createItem);
}

type UpdateItemMutation = {
  updateItem: WorldItemRecord;
};

/** Requires effective Editor or Owner on the item. */
export function updateItem(input: {
  itemId: string;
  name?: string;
  description?: string | null;
}): Promise<WorldItemRecord> {
  return postGraphQL<UpdateItemMutation>(
    `
      mutation UpdateItem($input: UpdateItemInput!) {
        updateItem(input: $input) {
          ${WORLD_ITEM_FIELDS}
        }
      }
    `,
    { input },
  ).then((data) => data.updateItem);
}

type DeleteItemMutation = {
  deleteItem: boolean;
};

/** Requires effective Owner on the item (FR-018); never blocked by
 * outstanding lore links or inventory references (FR-017). */
export function deleteItem(itemId: string): Promise<boolean> {
  return postGraphQL<DeleteItemMutation>(
    `
      mutation DeleteItem($itemId: UUID!) {
        deleteItem(itemId: $itemId)
      }
    `,
    { itemId },
  ).then((data) => data.deleteItem);
}

export type ItemEffectInput = {
  effectType: ItemEffectType;
  formula: string;
  target: string;
  triggerKind?: ItemEffectTrigger | null;
  sortOrder?: number;
};

type AddItemEffectMutation = {
  addItemEffect: WorldItemRecord["effects"][number];
};

/** Requires effective Editor or Owner on the item (FR-005); rejected with
 * a validation error for an empty/malformed formula (FR-006). */
export function addItemEffect(
  itemId: string,
  effect: ItemEffectInput,
): Promise<WorldItemRecord["effects"][number]> {
  return postGraphQL<AddItemEffectMutation>(
    `
      mutation AddItemEffect($itemId: UUID!, $effect: ItemEffectInput!) {
        addItemEffect(itemId: $itemId, effect: $effect) {
          ${ITEM_EFFECT_FIELDS}
        }
      }
    `,
    { itemId, effect },
  ).then((data) => data.addItemEffect);
}

type UpdateItemEffectMutation = {
  updateItemEffect: WorldItemRecord["effects"][number];
};

export function updateItemEffect(
  effectId: string,
  effect: ItemEffectInput,
): Promise<WorldItemRecord["effects"][number]> {
  return postGraphQL<UpdateItemEffectMutation>(
    `
      mutation UpdateItemEffect($effectId: UUID!, $effect: ItemEffectInput!) {
        updateItemEffect(effectId: $effectId, effect: $effect) {
          ${ITEM_EFFECT_FIELDS}
        }
      }
    `,
    { effectId, effect },
  ).then((data) => data.updateItemEffect);
}

type RemoveItemEffectMutation = {
  removeItemEffect: boolean;
};

export function removeItemEffect(effectId: string): Promise<boolean> {
  return postGraphQL<RemoveItemEffectMutation>(
    `
      mutation RemoveItemEffect($effectId: UUID!) {
        removeItemEffect(effectId: $effectId)
      }
    `,
    { effectId },
  ).then((data) => data.removeItemEffect);
}

type ItemPermissionsQuery = {
  itemPermissions: ItemPermissionRecord[];
};

/** DM-only to open/change (mirrors ActorOwnershipBlock — FR-003). */
export function getItemPermissions(itemId: string): Promise<ItemPermissionRecord[]> {
  return postGraphQL<ItemPermissionsQuery>(
    `
      query ItemPermissions($itemId: UUID!) {
        itemPermissions(itemId: $itemId) {
          itemId
          userId
          level
          updatedAt
        }
      }
    `,
    { itemId },
  ).then((data) => data.itemPermissions);
}

type SetItemPermissionMutation = {
  setItemPermission: ItemPermissionRecord;
};

export function setItemPermission(
  itemId: string,
  userId: string,
  level: ItemPermissionRecord["level"],
): Promise<ItemPermissionRecord> {
  return postGraphQL<SetItemPermissionMutation>(
    `
      mutation SetItemPermission($input: SetItemPermissionInput!) {
        setItemPermission(input: $input) {
          itemId
          userId
          level
          updatedAt
        }
      }
    `,
    { input: { itemId, userId, level } },
  ).then((data) => data.setItemPermission);
}

type RemoveItemPermissionMutation = {
  removeItemPermission: boolean;
};

/** Resets a member back to the implicit default Viewer level. */
export function removeItemPermission(itemId: string, userId: string): Promise<boolean> {
  return postGraphQL<RemoveItemPermissionMutation>(
    `
      mutation RemoveItemPermission($itemId: UUID!, $userId: UUID!) {
        removeItemPermission(itemId: $itemId, userId: $userId)
      }
    `,
    { itemId, userId },
  ).then((data) => data.removeItemPermission);
}
