# Contract: Items as a Lore In-Text Link Target (extends spec 012)

This contract describes the delta to spec 012's lore in-text-link resolution (`[[Title]]` → lore entry / actor), not a new standalone subsystem. It assumes spec 012's `world_lore_links` table and link-resolution pass (`src/server/src/markdown/links.rs`) as the baseline (see `specs/012-lore-wiki/contracts/lore-crud.md` and `specs/012-lore-wiki/data-model.md` §`world_lore_links`).

## Shape (delta to spec 012)

```graphql
enum LoreLinkTargetKind {
  LORE_ENTRY
  ACTOR
  ITEM        # NEW
  UNRESOLVED
}

type GraphQLLoreLink {
  id: ID!
  rawTitle: String!
  targetKind: LoreLinkTargetKind!
  targetLoreEntry: GraphQLLoreEntry     # set iff targetKind = LORE_ENTRY
  targetActor: GraphQLActor             # set iff targetKind = ACTOR
  targetItem: GraphQLItem               # NEW — set iff targetKind = ITEM
}

# Item detail gains a backlink field (mirrors GraphQLActor's existing "linked from" field)
extend type GraphQLItem {
  linkedFromLore: [GraphQLLoreEntry!]!
}
```

Data model delta: `world_lore_links` gains a nullable `target_item_id UUID FK → world_items.id ON DELETE SET NULL` column alongside its existing `target_lore_entry_id`/`target_actor_id`, and its `target_kind` enum gains an `item` variant. The existing "exactly one of the target FK columns is non-null at insert time, matching `target_kind`" check constraint (spec 012 data-model.md) extends to cover the new column.

## Behavior

- At lore-entry save time, the existing `[[Title]]` extraction/resolution pass (spec 012 research.md §2) additionally resolves each raw title against `world_items.name` (case-insensitive, scoped to the current world) alongside its existing lore-entry/actor resolution. Resolution stays "first match wins is not allowed" — if a title matches more than one kind of target (or more than one Item, since Item names may collide per FR-019), the authoring UI's autocomplete presents every match as a distinct, disambiguated choice (FR-016 of this spec, extending spec 012 FR-007a) rather than the server silently picking one.
- `GraphQLItem.linkedFromLore`: a query, not a stored column — `SELECT source_lore_entry_id FROM world_lore_links WHERE target_item_id = :itemId`, mirroring how `GraphQLActor`'s existing "linked from" field is computed (spec 012 data-model.md's note on `world_lore_links`).
- Deleting an Item nulls `target_item_id` on any referencing `world_lore_links` rows (via `ON DELETE SET NULL`, not `RESTRICT`) so the delete is never blocked (FR-017); any read/render path that already treats a null-FK'd row as "unresolved/broken" (spec 012's existing handling) applies unchanged to items — no new broken-link code path is needed, only the new nullable column and enum variant.

## Authorization

- No new authorization surface — resolving/rendering an Item link is already gated by the existing FR-018 "at least Viewer access" check when the reader views the Item's own detail page; the lore entry itself renders the link as a working link only if the *reader* has at least Viewer access to the target Item (mirrors spec 012's existing "resolves for users who can see the target, shows as inaccessible for users who cannot" rule, applied to the new target kind).

## Non-goals

- No change to spec 012's revision-history, image-upload, or Markdown-rendering behavior — this contract only extends the link-resolution target set.
- No requirement that spec 012 be implemented before this feature ships (plan.md's Structure Decision) — `world_lore_links.target_item_id` and the `item` enum variant are additive and can land whenever spec 012's schema exists, without blocking Item/inventory/share-link delivery.
