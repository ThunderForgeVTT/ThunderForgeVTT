/**
 * worldMembersCollection.ts
 * Plain types and pure helpers for world members (campaign roster).
 *
 * RxDB was hard cut from this layer: members are now fetched directly via
 * GraphQL (see `hooks/useWorldMembers.ts`, `api/worldMembers.ts`) rather
 * than cached/queried through a local RxDB collection. This module keeps
 * only the document shape and pure role-hierarchy helpers.
 *
 * The ranking itself lives in `@/types/world`, beside the one role type, and
 * every helper here asks it rather than keeping a table of its own.
 */

import {
  WORLD_ROLES,
  roleRank,
  runsTheWorld,
  type WorldMemberRole,
} from "@/types/world";

/**
 * Type definition for a world membership record, as returned by the
 * server's `worldMembers` GraphQL query (mapped from camelCase to this
 * snake_case shape for backward-compatible field names across consumers).
 */
export interface WorldMemberDoc {
  id: string;
  world_id: string;
  user_id: string;
  role: WorldMemberRole;
  joined_at: string;
  created_at: string;
  updated_at: string;

  // Client-side metadata
  is_current_user?: boolean;
  display_name?: string | null;
  /** Spec 023 (FR-004): the character this member has claimed, if any. */
  claimed_actor?: { id: string; label: string } | null;
}

/**
 * Whether a caller may change or remove a member holding `targetRole`.
 *
 * The server's rule, mirrored: an Owner manages anyone, a Game Master manages
 * whoever ranks below them (a Trusted Player included, so the trust can be
 * taken back), and nobody else manages anyone. The server decides; this only
 * decides which controls are worth rendering.
 */
export function canManageRole(
  callerRole: WorldMemberRole,
  targetRole: WorldMemberRole,
): boolean {
  if (callerRole === "Owner") return true;
  return (
    runsTheWorld(callerRole) && roleRank(targetRole) < roleRank(callerRole)
  );
}

/**
 * The roles a caller may hand out: none unless they run the world, and never
 * one above their own (ADR-099 — only an Owner or Game Master makes somebody
 * a Trusted Player). Highest first, the order a picker reads best in.
 */
export function assignableRoles(
  callerRole: WorldMemberRole,
): WorldMemberRole[] {
  if (!runsTheWorld(callerRole)) return [];
  return [...WORLD_ROLES]
    .reverse()
    .filter((role) => roleRank(role) <= roleRank(callerRole));
}

/**
 * Role hierarchy: determine who can invite.
 * Only those who run the world generate invites.
 */
export function canGenerateInvites(role: WorldMemberRole): boolean {
  return runsTheWorld(role);
}

/**
 * Sort members by role for display, highest first.
 */
export function sortMembersByRole(members: WorldMemberDoc[]): WorldMemberDoc[] {
  return [...members].sort((a, b) => roleRank(b.role) - roleRank(a.role));
}

/**
 * Filter members by role.
 */
export function filterMembersByRole(
  members: WorldMemberDoc[],
  role: WorldMemberRole,
): WorldMemberDoc[] {
  return members.filter((m) => m.role === role);
}

/**
 * Find a specific member by user_id in a world.
 */
export function findMember(
  members: WorldMemberDoc[],
  userId: string,
): WorldMemberDoc | undefined {
  return members.find((m) => m.user_id === userId);
}

/**
 * Check if user is a member of a world with a given role or higher.
 */
export function isMemberWithRole(
  members: WorldMemberDoc[],
  userId: string,
  role: WorldMemberRole,
): boolean {
  const member = findMember(members, userId);
  if (!member) return false;
  return roleRank(member.role) >= roleRank(role);
}
