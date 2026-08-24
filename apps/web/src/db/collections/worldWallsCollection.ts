/**
 * worldWallsCollection.ts
 * Plain TS types for scene walls (native canvas authoring,
 * specs/001-bevy-canvas-authoring).
 *
 * RxDB was hard-cut from this collection: the sync path
 * (engine/world/sync/walls.ts) already had a complete, working live-sync
 * mechanism through the world store / GraphQL mutation bridge that never
 * depended on RxDB — the RxDB schema and replication helpers that used to
 * live here were a redundant, unread local-cache side-write. See
 * engine/world/sync/walls.ts's module doc comment for the full
 * architecture. Field names match the GraphQL `Wall` type on the wire
 * (camelCase) rather than the Diesel model's snake_case.
 */

export type WallDoorState = 'none' | 'open' | 'closed';

/**
 * Type definition for a wall record (mirrors the GraphQL `Wall` type,
 * src/server/src/graphql.rs).
 */
export interface WorldWallDoc {
  wallId: string;
  sceneId: string;
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  blocksVision: boolean;
  blocksMovement: boolean;
  doorState: WallDoorState;
  metadata?: Record<string, unknown> | null;
  createdBy: string;
  updatedBy: string;
  createdAt: string;
  updatedAt: string;
}
