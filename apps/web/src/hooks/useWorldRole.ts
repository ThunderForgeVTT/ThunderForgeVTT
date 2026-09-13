/**
 * useWorldRole.ts
 * Derives the current user's role within a world, for gating GM-only
 * controls (spec 009, research.md §3).
 *
 * Reuses `useWorldMembers` (already fetched for the player roster) rather
 * than a second round-trip. Falls back to "Owner" when no `world_members`
 * row exists yet for the caller but they created the world — mirroring the
 * server's own `require_world_member` fallback (see
 * `graphql/queries/invite.rs`'s comment on that helper) — so a world's
 * creator is never locked out of GM controls just because their own
 * membership row hasn't been backfilled.
 */

import { useMemo } from "react";
import { useAuth } from "@/hooks/useAuth";
import { useWorldMembers } from "@/hooks/useWorldMembers";
import {
  managesContent,
  runsTheWorld,
  type WorldMemberRole,
  type WorldRecord,
} from "@/types/world";

export type WorldRole = WorldMemberRole | null;

export interface UseWorldRoleResult {
  role: WorldRole;
  /** Owner or Game Master — the shorthand most callers actually need. False
   * for a Trusted Player, who is shown what a Player is shown (ADR-099). */
  isGm: boolean;
  /** Owner, Game Master or Trusted Player: who may arrange the world's book
   * material. Deliberately a second flag rather than a wider `isGm`, so no
   * existing Game Master control opens to a Trusted Player by accident. */
  managesContent: boolean;
  loading: boolean;
}

export function useWorldRole(
  worldId: string,
  world: WorldRecord | null,
): UseWorldRoleResult {
  const { user } = useAuth();
  const { members, loading } = useWorldMembers(worldId);

  const role = useMemo<WorldRole>(() => {
    if (!user) {
      return null;
    }

    const membership = members.find((member) => member.user_id === user.id);
    if (membership) {
      return membership.role;
    }

    if (world && world.createdBy === user.id) {
      return "Owner";
    }

    return null;
  }, [members, user, world]);

  return {
    role,
    isGm: runsTheWorld(role),
    managesContent: managesContent(role),
    loading,
  };
}
