export type InventoryEntryRecord = {
  id: string;
  actorId: string;
  itemId: string | null;
  itemName: string;
  quantity: number;
};
