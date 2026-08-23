import type { ItemEffectRecord } from "@/types/item";

export type { DmWorldSummary } from "@/types/actorShare";

export type ItemShareLinkRecord = {
  id: string;
  itemId: string;
  shareCode: string;
  revoked: boolean;
  createdAt: string;
};

export type SharedItemPreview = {
  name: string;
  description: string | null;
  iconAssetId: string | null;
  effects: ItemEffectRecord[];
};
