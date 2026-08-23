import type { ActorPermissionLevel } from "@/types/actor";

export type { ActorPermissionLevel };

export type ItemEffectType = "HEAL" | "DAMAGE" | "MODIFIER" | "ATTACK_ROLL";
export type ItemEffectTrigger = "ON_USE" | "PASSIVE";

export type ItemEffectRecord = {
  id: string;
  itemId: string;
  effectType: ItemEffectType;
  formula: string;
  target: string;
  triggerKind: ItemEffectTrigger | null;
  sortOrder: number;
};

export type WorldItemRecord = {
  id: string;
  worldId: string;
  name: string;
  description: string | null;
  iconAssetId: string | null;
  effects: ItemEffectRecord[];
  myPermissionLevel: ActorPermissionLevel;
  createdAt: string;
  updatedAt: string;
};

export type ItemPermissionRecord = {
  itemId: string;
  userId: string;
  level: ActorPermissionLevel;
  updatedAt: string;
};
