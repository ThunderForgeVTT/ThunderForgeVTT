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
  /** What this token represents: `character`, `npc`, `vehicle` or `object`. */
  tokenType: TokenType;
  /**
   * Spec 046 (ADR-102): `true` when the token is its actor, so its hit points
   * are the actor's; `false` for an unlinked copy holding its own.
   */
  linked: boolean;
  /**
   * Playtest 2026-09-10 P7: the name drawn above the token — its own label,
   * else its character's. Absent when a Game Master has hidden it from this
   * viewer: the server does not send it.
   */
  name?: string | null;
  /** Whether players may read the name. A Game Master always can. */
  nameVisibleToPlayers?: boolean;
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
  /** Omitted means the actor's kind, or `character` for a token with none. */
  tokenType?: TokenType;
  /**
   * Omitted means the server decides from the actor: a character or unique
   * NPC is linked, any other NPC is an unlinked copy.
   */
  linked?: boolean;
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
}
