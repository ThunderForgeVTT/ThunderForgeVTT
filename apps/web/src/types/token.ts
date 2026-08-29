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
  ownerUserId: string | null;
  isPrimary: boolean;
  photoUrl: string | null;
  health: number | null;
  maxHealth: number | null;
  /** What this token represents: `character`, `npc`, `vehicle` or `object`. */
  tokenType: TokenType;
}

/**
 * What a token represents on the board.
 *
 * Mirrors `thunderforge_canvas_core::token_kind::TokenKind`, which is also
 * where each kind's colour is decided — the server rejects anything outside
 * this set, so a value arriving here is always one of these four.
 */
export type TokenType = "character" | "npc" | "vehicle" | "object";

/** Every kind, with the label a person sees. Order is the order shown. */
export const TOKEN_TYPES: { value: TokenType; label: string }[] = [
  { value: "character", label: "Character" },
  { value: "npc", label: "NPC" },
  { value: "vehicle", label: "Vehicle" },
  { value: "object", label: "Object" },
];

export interface CreateTokenInput {
  sceneId: string;
  actorId?: string;
  x: number;
  y: number;
  rotation?: number;
  scale?: number;
  metadata?: Record<string, unknown>;
  ownerUserId?: string;
  isPrimary?: boolean;
  photoUrl?: string;
  health?: number;
  maxHealth?: number;
  /** Omitted means `character`, matching the column default. */
  tokenType?: TokenType;
}

export interface UpdateTokenInput {
  actorId?: string;
  x?: number;
  y?: number;
  rotation?: number;
  scale?: number;
  metadata?: Record<string, unknown>;
  ownerUserId?: string;
  isPrimary?: boolean;
  /**
   * Omit to leave the token's art alone; send `null` to remove it. The
   * server reads this as a `MaybeUndefined`, so the two are genuinely
   * different requests rather than both meaning "unchanged".
   */
  photoUrl?: string | null;
  health?: number;
  maxHealth?: number;
}
