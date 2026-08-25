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
import type { WorldAccessLinkState } from "@/api/world";

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

  /**
   * Spec 027 (FR-010): supplied by the server, not recomputed here.
   *
   * The client cannot see revocation by inspecting counts and dates, so a
   * locally-derived verdict rendered a revoked link as perfectly healthy.
   */
  state: WorldAccessLinkState;
  /** Uses left, or `null` when uncapped. Server-supplied. */
  remaining_uses?: number | null;
  /** The link this one replaced, when created by rotation. */
  rotated_from?: string | null;

  // Client-side computed, for display only
  status?: string;
  is_valid?: boolean;
}

/**
 * Human-readable label for a link's state.
 *
 * Deliberately says nothing about the use cap being enforcement — rotation
 * resets the count, so a DM can rotate indefinitely. ADR-050 records the cap
 * as a convenience control, and GM-facing copy must not imply otherwise.
 */
export function inviteStateLabel(state: WorldAccessLinkState): string {
  switch (state) {
    case "ACTIVE":
      return "Active";
    case "EXPIRED":
      return "Expired";
    case "EXHAUSTED":
      return "All uses claimed";
    case "REVOKED":
      return "Revoked";
    default:
      // A state the client does not recognise must not render as working.
      return "Unavailable";
  }
}

/**
 * Compute display-only derived data for an invite.
 *
 * Spec 027: `is_valid` now comes straight from the server's `state` rather
 * than being re-derived from counts and dates. The previous local derivation
 * could not see revocation at all, so a revoked link reported itself valid.
 *
 * This remains **display only**. Enforcement happens server-side inside the
 * atomic consume; nothing here gates access, and a stale value is expected —
 * `useWorldInvites` has no live push transport.
 */
export function computeInviteDerivedData(invite: WorldInviteDoc): {
  status: string;
  is_valid: boolean;
} {
  const remaining =
    invite.remaining_uses ?? (invite.max_uses > 0 ? invite.max_uses - invite.used_count : null);

  const status =
    invite.state === "ACTIVE"
      ? remaining === null
        ? "Active · unlimited uses"
        : `Active · ${remaining} of ${invite.max_uses} uses left`
      : inviteStateLabel(invite.state);

  return { status, is_valid: invite.state === "ACTIVE" };
}
