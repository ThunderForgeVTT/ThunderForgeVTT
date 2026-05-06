/**
 * worldInvitesCollection.ts
 * RxDB collection schema for world invites (campaign invite codes)
 *
 * This collection mirrors the server's world_invites table with:
 * - Offline-first caching of invite codes
 * - Automatic replication via GraphQL subscriptions
 * - Real-time updates when invites are created/updated
 */

import { RxJsonSchema } from 'rxdb';

/**
 * JSON Schema for world_invites RxDB collection.
 * Must match the Diesel WorldInvite model.
 */
export const worldInvitesSchema: RxJsonSchema<any> = {
  title: 'World Invites',
  description: 'Campaign invite codes for multiplayer access',
  version: 0,
  keyCompression: false,
  primaryKey: 'id',
  type: 'object',
  properties: {
    // Base fields from server (world_invites table)
    id: {
      type: 'string',
      description: 'Unique invite ID (UUID)',
      maxLength: 36,
    },
    world_id: {
      type: 'string',
      description: 'World/campaign this invite belongs to',
      maxLength: 36,
      index: true,
    },
    invite_code: {
      type: 'string',
      description: '8-character uppercase hex code (e.g., A1B2C3D4)',
      maxLength: 8,
      pattern: '^[A-F0-9]{8}$',
    },
    max_uses: {
      type: 'integer',
      description: 'Maximum number of times this invite can be used',
      minimum: 1,
    },
    used_count: {
      type: 'integer',
      description: 'Number of times this invite has been used',
      minimum: 0,
    },
    expires_at: {
      type: ['string', 'null'],
      format: 'date-time',
      description: 'Optional expiry timestamp (ISO 8601)',
    },
    created_by: {
      type: 'string',
      description: 'User who created this invite (UUID)',
      maxLength: 36,
      index: true,
    },
    created_at: {
      type: 'string',
      format: 'date-time',
      description: 'ISO 8601 creation timestamp',
    },
    updated_at: {
      type: 'string',
      format: 'date-time',
      description: 'ISO 8601 last update timestamp',
    },

    // Client-side derived data (not sent to server)
    status: {
      type: ['string', 'null'],
      description: 'Derived: "used_count/max_uses" format for display',
    },
    is_valid: {
      type: 'boolean',
      description: 'Derived: whether invite is still valid (not expired, usage < max)',
      default: true,
    },
  },
  required: [
    'id',
    'world_id',
    'invite_code',
    'max_uses',
    'used_count',
    'created_by',
    'created_at',
    'updated_at',
  ],
  indexes: [
    // Efficient queries: all invites for a world, sorted by creation
    ['world_id', 'created_at'],
    // Lookup by invite code (for join operations)
    ['invite_code'],
  ],
};

/**
 * Type definition for WorldInvite documents in RxDB.
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
