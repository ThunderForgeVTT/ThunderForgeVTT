# Contract: Lore Entry Creation, Editing, Deletion (new)

## Shape

```graphql
type GraphQLLoreEntry {
  id: ID!
  worldId: ID!
  title: String!
  slug: String!
  content: String!                          # current Markdown source
  renderedHtml: String!                     # server-rendered, sanitized GFM output (research.md §1)
  currentRevisionId: ID
  myPermissionLevel: ActorPermissionLevel!  # reuses the existing enum from spec 010 (VIEWER/EDITOR/OWNER)
  createdBy: ID!
  createdAt: String!
  updatedAt: String!
}

input CreateLoreEntryInput {
  worldId: ID!
  title: String!
  content: String                           # optional — defaults to empty body
}

input UpdateLoreEntryInput {
  loreEntryId: ID!
  title: String                             # if provided and different, slug is regenerated (FR-014)
  content: String                            # if provided, appends a new revision (FR-016)
  expectedCurrentRevisionId: ID              # REQUIRED whenever `content` is provided (FR-019) — the revision the author was editing against
}

createLoreEntry(input: CreateLoreEntryInput!): GraphQLLoreEntry!
updateLoreEntry(input: UpdateLoreEntryInput!): GraphQLLoreEntry!
deleteLoreEntry(loreEntryId: ID!): Boolean!

worldLoreEntries(worldId: ID!): [GraphQLLoreEntry!]!
loreEntry(worldId: ID!, slug: String!): GraphQLLoreEntry     # null if not found or slug stale (FR-014)
```

## Behavior

- `createLoreEntry`: inserts a `world_lore_entries` row, generates its initial `slug` from `title` (data-model.md, disambiguated on collision per FR-013), creates the first `world_lore_revisions` row if `content` is non-empty, sets `created_by` to the caller. No `world_lore_permissions` row is created for the creator (the DM already has implicit Owner, same precedent as `createActor`).
- `updateLoreEntry`: partial update — only provided fields change. A `title` change regenerates `slug` (with disambiguation) but keeps `id` stable; a `content` change appends a new `world_lore_revisions` row, re-extracts `[[...]]` links (replacing this entry's `world_lore_links` source rows, research.md §2), and re-renders `renderedHtml`. Rejects (per FR-010a) if the new `content` exceeds 25 MB, before any row is written. When `content` is provided, `expectedCurrentRevisionId` MUST also be provided and MUST match the entry's actual `current_revision_id` at the moment of the write (checked inside the same transaction that inserts the new revision) — a mismatch means someone else saved first, and the mutation is rejected outright with a conflict error rather than overwriting their change (FR-019, per Clarifications). The rejected caller must re-fetch the entry's current content and revision id before retrying.
- `deleteLoreEntry`: deletes the entry and cascades its permissions/links/images/revisions (data-model.md). Does NOT check for or block on other entries' `world_lore_links` rows pointing at it (FR-020) — those rows' `target_lore_entry_id` becomes dangling and are rendered as broken links going forward (the row itself may be left as `unresolved` lazily on next read, or proactively updated in the same delete transaction — an implementation choice for tasks.md).
- `worldLoreEntries`: returns every entry in the world to every member (listing is not permission-gated, matching the `worldActors` precedent) — `myPermissionLevel` on each tells the client what UI to show.
- `loreEntry(worldId, slug)`: the canonical detail-page lookup, resolved by `(world_id, slug)` — returns `null` (not an error) for a stale/nonexistent slug so the frontend can render a graceful not-found state (FR-014's "fail gracefully" choice, data-model.md).

## Authorization

- `createLoreEntry`: caller MUST be DM (Owner or GM role) of `input.worldId` (FR-002), via `is_dm_of_world` (generalized/reused from `auth/actor_permissions.rs`).
- `updateLoreEntry`: caller's `myPermissionLevel` for `input.loreEntryId` MUST be `EDITOR` or `OWNER` (FR-003 ownership-block enforcement), via `require_lore_permission(..., minimum: Editor)`.
- `deleteLoreEntry`: caller's `myPermissionLevel` for `loreEntryId` MUST be `OWNER` (FR-021 — entry-level Owner, not DM-only, per spec Clarifications), via `require_lore_permission(..., minimum: Owner)`.
- `worldLoreEntries` / `loreEntry`: caller MUST be a member (or admin) of the world at all; `loreEntry` additionally returns `null` (not the entry) if the caller's `myPermissionLevel` would be below `Viewer` — but since Viewer is the universal default (FR-003), this only excludes non-members entirely (FR-015).

## Non-goals

- No GraphQL field for listing "all lore entries across all worlds" — this feature is strictly world-scoped (spec Assumptions).
- No bulk-delete or bulk-create mutation.
