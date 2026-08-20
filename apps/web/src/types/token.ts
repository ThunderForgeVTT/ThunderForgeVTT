// Wire types for the scene-scoped `tokens` table (native canvas authoring),
// mirroring types/wall.ts's shape. Distinct from the legacy world-scoped
// `WorldToken` GraphQL type (types.ts's `WorldToken`/`GraphQLWorldToken`) —
// this is the modern per-scene token persisted via TokenMutation
// (src/server/src/graphql/mutations_tokens.rs).

export interface TokenRecord {
  tokenId: string;
  sceneId: string;
  actorId: string | null;
  x: number;
  y: number;
  rotation: number;
  scale: number;
  metadata: Record<string, unknown> | null;
  createdAt: string;
  updatedAt: string;
}

export interface CreateTokenInput {
  sceneId: string;
  actorId?: string;
  x: number;
  y: number;
  rotation?: number;
  scale?: number;
  metadata?: Record<string, unknown>;
}

export interface UpdateTokenInput {
  actorId?: string;
  x?: number;
  y?: number;
  rotation?: number;
  scale?: number;
  metadata?: Record<string, unknown>;
}
