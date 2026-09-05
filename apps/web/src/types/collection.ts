export type { DmWorldSummary } from "@/types/actorShare";

/**
 * The five things a collection can hold (spec 026 FR-002).
 *
 * These strings are the wire format — `collection_members.member_type` on the
 * server stores them verbatim, and `collections/membership.rs` matches on them
 * with one arm per type. A sixth type is a server change first; adding one
 * here alone would produce a member the server refuses.
 */
export const COLLECTION_MEMBER_TYPES = [
  "scene",
  "actor",
  "item",
  "lore",
  "ability",
] as const;

export type CollectionMemberType = (typeof COLLECTION_MEMBER_TYPES)[number];

/** Singular, for a member; the plural is only ever shown for a count. */
export const COLLECTION_MEMBER_TYPE_LABELS: Record<
  CollectionMemberType,
  { one: string; many: string }
> = {
  scene: { one: "Scene", many: "Scenes" },
  actor: { one: "Actor", many: "Actors" },
  item: { one: "Item", many: "Items" },
  lore: { one: "Lore entry", many: "Lore entries" },
  ability: { one: "Ability", many: "Abilities" },
};

export function memberTypeLabel(memberType: string, count: number = 1): string {
  const known =
    COLLECTION_MEMBER_TYPE_LABELS[memberType as CollectionMemberType];
  if (known === undefined) {
    // A type the server knows and this build does not. Showing the raw string
    // is honest and still readable; inventing a label would not be either.
    return memberType;
  }
  return count === 1 ? known.one : known.many;
}

export type CollectionRecord = {
  id: string;
  worldId: string;
  name: string;
  description: string | null;
  memberCount: number;
  createdAt: string;
  updatedAt: string;
};

export type CollectionMemberRecord = {
  id: string;
  collectionId: string;
  memberType: string;
  memberId: string;
  sortOrder: number;
};

export type CollectionShareLinkRecord = {
  id: string;
  collectionId: string;
  shareCode: string;
  revoked: boolean;
};

export type SharedCollectionMemberPreview = {
  memberType: string;
  name: string;
};

export type CollectionTypeCount = {
  memberType: string;
  count: number;
};

/**
 * Spec 026 (FR-009a): what a share-link viewer sees, **with or without an
 * account**.
 *
 * Carries no `id`, no `worldId`, no `createdBy` and no member ids — a viewer
 * must not be able to identify the source world, its members, or anything in
 * it they were not shown. Adding any of those here would be a real leak
 * rather than a convenience, and the server has a test that formats this whole
 * preview and greps it for world identifiers.
 */
export type SharedCollectionPreview = {
  name: string;
  description: string | null;
  members: SharedCollectionMemberPreview[];
  /** US4 scenario 1: how many of each kind, before deciding to copy. */
  countsByType: CollectionTypeCount[];
  /**
   * FR-022: **a number, never a name.** Reproducing the title of a taken-down
   * artifact in the sentence explaining that it was taken down would defeat
   * the takedown.
   */
  withheldCount: number;
};

export type CopiedRecord = {
  memberType: string;
  id: string;
  name: string;
};

/**
 * What arrived, and what did not (FR-015).
 *
 * `fidelityNotes` are declared losses — a reference that pointed outside the
 * collection, a member withheld by moderation, tokens that stayed behind. They
 * are shown to the recipient, never swallowed: a copy that quietly differs
 * from what was shared is the failure this field exists to prevent.
 */
export type CopyReceipt = {
  created: CopiedRecord[];
  fidelityNotes: string[];
};
