/**
 * worldInvitesCollection.ts
 * Plain types and pure helpers for world invites (campaign invite codes).
 *
 * RxDB was hard cut from this layer: invites are now fetched directly via
 * GraphQL (see `hooks/useWorldInvites.ts`, `api/world.ts`) rather than
 * cached/queried through a local RxDB collection. This module keeps only
 * the document shape and the pure derived-data helper, still consumed by
 * `engine/world/sync/schemas.ts`.
 */

/**
 * Type definition for a world invite record, as returned by the server's
 * `worldInvites` GraphQL query (mapped from camelCase to this snake_case
 * shape for backward-compatible field names across consumers).
 */
export interface WorldInviteDoc {
  id: string;
  world_id: string;
  invite_code: string;
  max_uses: number;
  used_count: number;
  expires_at?: string | null;
  created_by: string;
  created_at: string;
  updated_at: string;

  // Client-side computed
  status?: string;
  is_valid?: boolean;
}

/**
 * Compute derived data for an invite (called on client to avoid network overhead).
 * 
 * Derives:
 * - status: "2/5 uses" format for display
 * - is_valid: true if not expired and usage < max_uses
 */
export function computeInviteDerivedData(invite: WorldInviteDoc): {
  status: string;
  is_valid: boolean;
} {
  const status = `${invite.used_count}/${invite.max_uses} uses`;

  let is_valid = invite.used_count < invite.max_uses;
  if (invite.expires_at) {
    const expiresAt = new Date(invite.expires_at).getTime();
    const now = new Date().getTime();
    is_valid = is_valid && expiresAt > now;
  }

  return { status, is_valid };
}
