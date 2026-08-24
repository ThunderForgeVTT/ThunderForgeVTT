/**
 * worldTokensSceneCollection.ts
 * Plain TS types for scene-scoped tokens (the modern `tokens` table,
 * native canvas authoring).
 *
 * Named distinctly from `worldTokensCollection.ts`, which holds the
 * legacy world-scoped `world_tokens` table's types.
 *
 * RxDB was hard-cut from this collection: the sync path
 * (engine/world/sync/tokens.ts) already had a complete, working live-sync
 * mechanism through the world store / GraphQL mutation bridge that never
 * depended on RxDB — the RxDB schema and replication helpers that used to
 * live here were a redundant, unread local-cache side-write. See
 * engine/world/sync/walls.ts's module doc comment (tokens mirrors it) for
 * the full architecture. Field names match the GraphQL
 * `Token`/`GraphQLToken` type on the wire (camelCase).
 */

/**
 * Type definition for a scene-scoped token record (mirrors the GraphQL
 * `GraphQLToken` type, src/server/src/graphql.rs).
 */
export interface WorldSceneTokenDoc {
  tokenId: string;
  sceneId: string;
  actorId?: string | null;
  x: number;
  y: number;
  rotation: number;
  scale: number;
  metadata?: Record<string, unknown> | null;
  createdAt: string;
  updatedAt: string;
  ownerUserId?: string | null;
  isPrimary?: boolean;
  photoUrl?: string | null;
  health?: number | null;
  maxHealth?: number | null;
}
