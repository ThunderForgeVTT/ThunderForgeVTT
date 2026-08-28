/**
 * worldShapesCollection.ts
 * Plain TS types for scene shapes (native canvas authoring: freehand
 * strokes, rectangles, ellipses, lines/arrows, text labels drawn directly
 * on the game canvas — specs/001-bevy-canvas-authoring).
 *
 * RxDB was hard-cut from this collection: the sync path
 * (engine/world/sync/shapes.ts) already had a complete, working live-sync
 * mechanism through the world store / GraphQL mutation bridge that never
 * depended on RxDB — the RxDB schema and replication helpers that used to
 * live here were a redundant, unread local-cache side-write. See
 * engine/world/sync/walls.ts's module doc comment (shapes mirrors it) for
 * the full architecture. Field names match the GraphQL `Shape` type on the
 * wire (camelCase) rather than the Diesel model's snake_case.
 */

export type ShapeKind = "STROKE" | "RECT" | "ELLIPSE" | "LINE" | "TEXT";

/**
 * Type definition for a shape record (mirrors the GraphQL `Shape` type,
 * src/server/src/graphql.rs).
 */
export interface WorldShapeDoc {
  shapeId: string;
  sceneId: string;
  kind: ShapeKind;
  geometry: Record<string, unknown>;
  text?: string | null;
  style?: Record<string, unknown> | null;
  visibleToPlayers: boolean;
  metadata?: Record<string, unknown> | null;
  createdBy: string;
  updatedBy: string;
  createdAt: string;
  updatedAt: string;
}
