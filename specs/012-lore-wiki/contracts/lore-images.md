# Contract: Lore Image Upload (new)

## Shape

```graphql
type GraphQLLoreImageAsset {
  id: ID!
  loreEntryId: ID!
  url: String!         # full-size processed rendition
  thumbnailUrl: String!
  byteSize: Int!
  createdAt: String!
}

# multipart upload, mirrors the existing uploadCanvasImage mutation's shape (spec 002)
uploadLoreImage(loreEntryId: ID!, file: Upload!): GraphQLLoreImageAsset!
```

## Behavior

- `uploadLoreImage`: accepts raw image bytes via the existing multipart GraphQL upload path (Axum `multipart` feature, already enabled per `src/server/Cargo.toml`). Server enforces the 25 MB cap (FR-010) *before* decode, then runs the extended `transcode.rs` pipeline (research.md §5) to produce a normalized full-size WebP and a thumbnail WebP, writes both to RustFS via the existing per-object STS-scoped `write_object` path (unchanged, ADR-039), and only then inserts the `world_lore_image_assets` row. Returns the two durable URLs for the client to insert as `![](url)` at the cursor (FR-008).
- On any failure (oversized, unsupported format, transcode error, or a partial RustFS write), no `world_lore_image_assets` row is created and the mutation returns a GraphQL error — the entry's content is never left referencing a missing asset (edge case, spec 012).

## Authorization

- Caller's `myPermissionLevel` for `loreEntryId` MUST be `EDITOR` or `OWNER` (same edit-gate as `updateLoreEntry`) — a Viewer cannot upload images to an entry they can't edit.

## Non-goals

- No standalone "media library" browsing all of a world's lore images outside the entry they were uploaded into.
- No client-side crop/filter editing — only server-side normalization (resize/format), per research.md §5.
