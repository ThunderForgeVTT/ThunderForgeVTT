/**
 * worldLightsCollection.ts
 * Plain TS types for scene light sources (native canvas authoring,
 * specs/001-bevy-canvas-authoring, User Story 3).
 *
 * RxDB was hard-cut from this collection: the sync path
 * (engine/world/sync/lights.ts) already had a complete, working live-sync
 * mechanism through the world store / GraphQL mutation bridge that never
 * depended on RxDB — the RxDB schema and replication helpers that used to
 * live here were a redundant, unread local-cache side-write. See
 * engine/world/sync/walls.ts's module doc comment (lights mirrors it) for
 * the full architecture. Field names match the GraphQL `LightSource` type
 * on the wire (camelCase) rather than the Diesel model's snake_case.
 */

/**
 * Type definition for a light source record (mirrors the GraphQL
 * `LightSource` type, src/server/src/graphql.rs).
 */
export interface WorldLightDoc {
  lightId: string;
  sceneId: string;
  x: number;
  y: number;
  radius: number;
  intensity: number;
  color?: string | null;
  attachedTokenId?: string | null;
  castsShadows: boolean;
  metadata?: Record<string, unknown> | null;
  createdBy: string;
  updatedBy: string;
  createdAt: string;
  updatedAt: string;
}
