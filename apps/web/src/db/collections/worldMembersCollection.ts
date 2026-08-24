/**
 * worldMembersCollection.ts
 * Plain types and pure helpers for world members (campaign roster).
 *
 * RxDB was hard cut from this layer: members are now fetched directly via
 * GraphQL (see `hooks/useWorldMembers.ts`, `api/worldMembers.ts`) rather
 * than cached/queried through a local RxDB collection. This module keeps
 * only the document shape and pure role-hierarchy helpers, which are still
 * consumed by `engine/world/sync/schemas.ts` and various components.
 */

/**
 * Type definition for a world membership record, as returned by the
 * server's `worldMembers` GraphQL query (mapped from camelCase to this
 * snake_case shape for backward-compatible field names across consumers).
 */
export interface WorldMemberDoc {
  id: string;
  world_id: string;
  user_id: string;
  role: 'Owner' | 'GM' | 'Player';
  joined_at: string;
  created_at: string;
  updated_at: string;

  // Client-side metadata
  is_current_user?: boolean;
  display_name?: string | null;
}

/**
 * Role hierarchy helper: check if a role can manage another role.
 * 
 * Owner can manage anyone.
 * GM can manage Players and GMs, but not Owners.
 * Player cannot manage anyone.
 */
export function canManageRole(
  callerRole: 'Owner' | 'GM' | 'Player',
  targetRole: 'Owner' | 'GM' | 'Player'
): boolean {
  if (callerRole === 'Owner') return true;
  if (callerRole === 'GM') return targetRole !== 'Owner';
  return false;
}

/**
 * Role hierarchy: determine who can invite.
 * Only Owner and GM can generate invites.
 */
export function canGenerateInvites(role: 'Owner' | 'GM' | 'Player'): boolean {
  return role === 'Owner' || role === 'GM';
}

/**
 * Sort members by role hierarchy for display.
 * Owner first, then GM, then Player.
 */
export function sortMembersByRole(
  members: WorldMemberDoc[]
): WorldMemberDoc[] {
  const roleOrder = { Owner: 0, GM: 1, Player: 2 };
  return [...members].sort(
    (a, b) => roleOrder[a.role] - roleOrder[b.role]
  );
}

/**
 * Filter members by role.
 */
export function filterMembersByRole(
  members: WorldMemberDoc[],
  role: 'Owner' | 'GM' | 'Player'
): WorldMemberDoc[] {
  return members.filter((m) => m.role === role);
}

/**
 * Find a specific member by user_id in a world.
 */
export function findMember(
  members: WorldMemberDoc[],
  userId: string
): WorldMemberDoc | undefined {
  return members.find((m) => m.user_id === userId);
}

/**
 * Check if user is a member of a world with a given role or higher.
 */
export function isMemberWithRole(
  members: WorldMemberDoc[],
  userId: string,
  role: 'Owner' | 'GM' | 'Player'
): boolean {
  const member = findMember(members, userId);
  if (!member) return false;

  const roleHierarchy = { Owner: 3, GM: 2, Player: 1 };
  return roleHierarchy[member.role] >= roleHierarchy[role];
}
