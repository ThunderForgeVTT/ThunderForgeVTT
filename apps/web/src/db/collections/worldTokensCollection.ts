/**
 * worldTokensCollection.ts
 * Plain TS types/helpers for the legacy world-scoped `world_tokens` table.
 *
 * RxDB was hard-cut from this collection (and this whole legacy path):
 * scene-scoped tokens are persisted through the modern `tokens` table via
 * engine/world/sync/tokens.ts's store-bridge, which never depended on
 * RxDB. The RxDB schema and replication/optimistic-update helpers that
 * used to live here (and the `engine/world/sync/index.ts#startWorldSync`
 * caller that used them) were dead weight: nothing read the cached docs
 * back, and the durable-mutation transport posted to GraphQL mutations
 * (`syncWorldMutations`/`publishTokenDeltas`) that never existed
 * server-side. `computeTokenDerivedStats` is kept as a plain,
 * RxDB-independent helper in case UI code still wants it.
 */

/**
 * Type definition for a legacy world-scoped token record.
 */
export interface WorldTokenDoc {
  id: string;
  world_id: string;
  x: number;
  y: number;
  z: number;
  label?: string;
  health?: number;
  max_health?: number;
  created_by: string;
  updated_by: string;
  schema_version: number;
  created_at: string;
  updated_at: string;
}

/**
 * Compute derived stats from base token data.
 */
export function computeTokenDerivedStats(token: WorldTokenDoc) {
  const health = token.health ?? 0;
  const maxHealth = token.max_health ?? 1;
  const healthPercentage = maxHealth > 0 ? (health / maxHealth) * 100 : 0;

  return {
    healthPercentage: Math.max(0, Math.min(100, healthPercentage)),
    isDead: health <= 0,
    isFullHealth: health >= maxHealth,
  };
}
