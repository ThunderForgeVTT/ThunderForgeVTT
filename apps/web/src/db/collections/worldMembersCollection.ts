/**
 * worldMembersCollection.ts
 * RxDB collection schema for world members (campaign roster)
 *
 * This collection mirrors the server's world_members table with:
 * - Offline-first caching of campaign rosters
 * - Automatic replication via GraphQL subscriptions
 * - Real-time updates when members join or roles change
 */

import { RxJsonSchema } from 'rxdb';

/**
 * JSON Schema for world_members RxDB collection.
 * Must match the Diesel WorldMember model.
 */
export const worldMembersSchema: RxJsonSchema<any> = {
  title: 'World Members',
  description: 'Players and GMs who are members of a campaign',
  version: 0,
  keyCompression: false,
  primaryKey: 'id',
  type: 'object',
  properties: {
    // Base fields from server (world_members table)
    id: {
      type: 'string',
      description: 'Unique membership record ID (UUID)',
      maxLength: 36,
    },
    world_id: {
      type: 'string',
      description: 'World/campaign this member belongs to',
      maxLength: 36,
      index: true,
    },
    user_id: {
      type: 'string',
      description: 'User ID of the member (UUID)',
      maxLength: 36,
      index: true,
    },
    role: {
      type: 'string',
      description: 'Member role: Owner, GM, or Player',
      enum: ['Owner', 'GM', 'Player'],
    },
    joined_at: {
      type: 'string',
      format: 'date-time',
      description: 'ISO 8601 timestamp when member joined',
    },
    created_at: {
      type: 'string',
      format: 'date-time',
      description: 'ISO 8601 record creation timestamp',
    },
    updated_at: {
      type: 'string',
      format: 'date-time',
      description: 'ISO 8601 last update timestamp',
    },

    // Client-side metadata (not sent to server)
    is_current_user: {
      type: 'boolean',
      description: 'Derived: whether this is the logged-in user',
      default: false,
    },
    display_name: {
      type: ['string', 'null'],
      description: 'Derived: cached user display name (for sorting/filtering)',
    },
  },
  required: [
    'id',
    'world_id',
    'user_id',
    'role',
    'joined_at',
    'created_at',
    'updated_at',
  ],
  indexes: [
    // Efficient queries: all members of a world, sorted by join time
    ['world_id', 'joined_at'],
    // Lookup member by world and user (to check membership)
    ['world_id', 'user_id'],
    // Query members by role (to find all GMs, etc.)
    ['world_id', 'role'],
  ],
};

/**
 * Type definition for WorldMember documents in RxDB.
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
