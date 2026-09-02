import type { ActorPermissionLevel } from "@/types/actor";

export type LoreLinkSourceRecord = {
  id: string;
  title: string;
  slug: string;
};

export type LoreEntryRecord = {
  id: string;
  worldId: string;
  title: string;
  slug: string;
  content: string;
  renderedHtml: string;
  currentRevisionId: string | null;
  myPermissionLevel: ActorPermissionLevel;
  moderated: boolean;
  moderationCaseId: string | null;
  createdBy: string;
  createdAt: string;
  updatedAt: string;
  /** Spec 031 (FR-038): the entry this one files under, `null` at the top
   * level. An id and not a slug: the tree outlives the URL scheme. */
  parentId: string | null;
  /** Spec 031 (FR-038): normalised, alphabetical. Empty on a moderated
   * placeholder, which carries no real content to label. */
  tags: string[];
  /** FR-006: every lore entry whose body currently links to this one. */
  linkedFrom: LoreLinkSourceRecord[];
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

/** Spec 025 (T002): widened to the full set the backend actually returns.
 * `ITEM` has been returned by `lore_link_targets_impl` since spec 013 but was
 * never added here, so item candidates were mislabelled in the `[[`
 * autocomplete; `ABILITY` is added by spec 025. Keep this in sync with
 * `GraphQLLoreLinkTargetKind` (src/server/src/graphql/queries/lore.rs). */
export type LoreLinkTargetKind = "LORE_ENTRY" | "ACTOR" | "ITEM" | "ABILITY";

export type LoreLinkTargetRecord = {
  id: string;
  title: string;
  kind: LoreLinkTargetKind;
};
