import type { ActorPermissionLevel } from "@/types/actor";

export type LoreEntryRecord = {
  id: string;
  worldId: string;
  title: string;
  slug: string;
  content: string;
  renderedHtml: string;
  currentRevisionId: string | null;
  myPermissionLevel: ActorPermissionLevel;
  createdBy: string;
  createdAt: string;
  updatedAt: string;
};

export type LoreRevisionRecord = {
  id: string;
  loreEntryId: string;
  contentMarkdown: string;
  renderedHtml: string;
  authorId: string;
  restoredFromRevisionId: string | null;
  createdAt: string;
};

export type LorePermissionRecord = {
  loreEntryId: string;
  userId: string;
  level: ActorPermissionLevel;
  updatedAt: string;
};

export type LoreImageAssetRecord = {
  id: string;
  loreEntryId: string;
  url: string;
  thumbnailUrl: string;
  byteSize: number;
  createdAt: string;
};

export type LoreLinkTargetKind = "LORE_ENTRY" | "ACTOR";

export type LoreLinkTargetRecord = {
  id: string;
  title: string;
  kind: LoreLinkTargetKind;
};
