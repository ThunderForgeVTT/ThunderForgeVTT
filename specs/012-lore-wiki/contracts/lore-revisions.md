# Contract: Lore Entry Revision History (new)

## Shape

```graphql
type GraphQLLoreRevision {
  id: ID!
  loreEntryId: ID!
  contentMarkdown: String!
  renderedHtml: String!            # re-rendered on read for this specific historical revision
  authorId: ID!
  restoredFromRevisionId: ID
  createdAt: String!
}

loreEntryRevisions(loreEntryId: ID!): [GraphQLLoreRevision!]!   # chronological, newest first
restoreLoreRevision(revisionId: ID!): GraphQLLoreEntry!
```

## Behavior

- `loreEntryRevisions`: returns every revision for the entry, newest first, each with author/timestamp (FR-017). No pagination cap specified by the spec — implementation may add one at scale, but that's a tasks.md-level detail, not a contract change.
- `restoreLoreRevision`: looks up the target revision, appends a **new** `world_lore_revisions` row whose `content_markdown` matches the target and whose `restored_from_revision_id` points at it, updates the entry's `content`/`current_revision_id`/`slug`-unaffected fields accordingly, re-extracts `[[...]]` links against the restored content (research.md §2). No existing revision row is ever deleted or mutated (FR-018).

## Authorization

- `loreEntryRevisions`: caller MUST have at least `VIEWER` access to the entry (FR-017 — same as viewing the entry itself).
- `restoreLoreRevision`: caller's `myPermissionLevel` on the revision's parent entry MUST be `EDITOR` or `OWNER` (FR-018).

## Non-goals

- No diff/comparison view between two revisions — the spec only requires viewing a single past revision's full content and restoring it (FR-017/018), not a diff UI.
