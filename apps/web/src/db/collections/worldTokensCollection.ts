/**
 * worldTokensCollection.ts
 * RxDB collection schema and replication setup for world tokens
 *
 * This collection mirrors the server's world_tokens table with:
 * - Offline-first caching
 * - Automatic replication via GraphQL subscriptions
 * - Optimistic updates with rollback
 * - Derived data computation on the client
 */

import { RxCollection, RxJsonSchema } from 'rxdb';

/**
 * JSON Schema for world_tokens RxDB collection.
 * Must match the Diesel WorldToken model but with client-side extensions.
 */
export const worldTokensSchema: RxJsonSchema<any> = {
  title: 'World Tokens',
  description: 'Tokens placed on world scenes',
  version: 1,
  keyCompression: false,
  primaryKey: 'id',
  type: 'object',
  properties: {
    // Base fields from server
    id: {
      type: 'string',
      description: 'Unique token ID (UUID)',
      maxLength: 36,
    },
    world_id: {
      type: 'string',
      description: 'World this token belongs to',
      maxLength: 36,
      index: true, // Indexed for queries
    },
    x: {
      type: 'number',
      description: 'X coordinate',
      minimum: -Infinity,
      maximum: Infinity,
    },
    y: {
      type: 'number',
      description: 'Y coordinate',
      minimum: -Infinity,
      maximum: Infinity,
    },
    z: {
      type: 'number',
      description: 'Z coordinate (layer depth)',
      minimum: -Infinity,
      maximum: Infinity,
    },
    label: {
      type: 'string',
      description: 'Display label for token',
      nullable: true,
      maxLength: 255,
    },
    health: {
      type: 'number',
      description: 'Current health points',
      nullable: true,
      integer: true,
      minimum: 0,
    },
    max_health: {
      type: 'number',
      description: 'Maximum health points',
      nullable: true,
      integer: true,
      minimum: 1,
    },
    created_by: {
      type: 'string',
      description: 'User who created this token',
      maxLength: 36,
      index: true,
    },
    updated_by: {
      type: 'string',
      description: 'User who last updated this token',
      maxLength: 36,
    },
    schema_version: {
      type: 'integer',
      description: 'Schema version for migration tracking',
      minimum: 1,
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

    // Client-side extensions (not sent to server)
    _optimistic: {
      type: 'boolean',
      description: 'Whether this token represents an optimistic update',
      default: false,
    },
    _lastServerPosition: {
      type: 'object',
      description: 'Last known server position for rollback',
      nullable: true,
      properties: {
        x: { type: 'number' },
        y: { type: 'number' },
        z: { type: 'number' },
      },
    },
  },
  required: [
    'id',
    'world_id',
    'x',
    'y',
    'z',
    'created_by',
    'updated_by',
    'schema_version',
    'created_at',
    'updated_at',
  ],
  indexes: [
    // Composite index for efficient queries by world
    ['world_id', 'created_at'],
  ],
};

/**
 * Type definition for WorldToken documents in RxDB.
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

  // Client extensions
  _optimistic?: boolean;
  _lastServerPosition?: {
    x: number;
    y: number;
    z: number;
  };
}

/**
 * Setup replication for world_tokens collection.
 *
 * This function:
 * 1. Subscribes to worldEventCreated GraphQL subscription
 * 2. Applies server events to the RxDB collection
 * 3. Handles optimistic rollback on rejection
 * 4. Manages offline queuing of mutations
 */
export async function setupWorldTokensReplication(
  collection: RxCollection<WorldTokenDoc>,
  graphqlSubscription: AsyncIterable<any>,
  worldId: string
): Promise<() => void> {
  /**
   * Subscribe to server events and apply them locally.
   */
  const abortController = new AbortController();

  (async () => {
    try {
      for await (const event of graphqlSubscription) {
        if (abortController.signal.aborted) break;

        // Parse the world event
        const { id, event_code, token_event, created_by, updated_by } = event;

        // Event codes:
        // 1 = token created
        // 2 = token moved
        // 3 = token deleted
        // 4 = token stats updated

        if (token_event?.token_id) {
          const tokenId = token_event.token_id;

          switch (event_code) {
            case 1: // Created
              // Upsert new token from server
              await collection.upsert({
                id: tokenId,
                world_id: worldId,
                x: token_event.x ?? 0,
                y: token_event.y ?? 0,
                z: token_event.z ?? 0,
                label: token_event.label,
                health: token_event.health,
                max_health: token_event.max_health,
                created_by,
                updated_by,
                schema_version: token_event.schema_version ?? 1,
                created_at: new Date().toISOString(),
                updated_at: new Date().toISOString(),
                _optimistic: false,
              });
              break;

            case 2: // Moved
              // Update position
              const doc = await collection.findOne(tokenId).exec();
              if (doc) {
                await doc.update({
                  $set: {
                    x: token_event.x ?? doc.get('x'),
                    y: token_event.y ?? doc.get('y'),
                    z: token_event.z ?? doc.get('z'),
                    updated_by,
                    updated_at: new Date().toISOString(),
                    _optimistic: false,
                  },
                });
              }
              break;

            case 3: // Deleted
              // Remove token
              const deleteDoc = await collection.findOne(tokenId).exec();
              if (deleteDoc) {
                await deleteDoc.remove();
              }
              break;

            case 4: // Stats updated
              // Update health/label
              const updateDoc = await collection.findOne(tokenId).exec();
              if (updateDoc) {
                await updateDoc.update({
                  $set: {
                    health: token_event.health ?? updateDoc.get('health'),
                    max_health:
                      token_event.max_health ?? updateDoc.get('max_health'),
                    label: token_event.label ?? updateDoc.get('label'),
                    updated_by,
                    updated_at: new Date().toISOString(),
                    _optimistic: false,
                  },
                });
              }
              break;
          }
        }

        // Check for rejection events (event_code = -1)
        if (event_code === -1 && token_event?.token_id) {
          // Rollback: restore _lastServerPosition
          const rollbackDoc = await collection
            .findOne(token_event.token_id)
            .exec();
          if (
            rollbackDoc &&
            rollbackDoc.get('_optimistic') &&
            rollbackDoc.get('_lastServerPosition')
          ) {
            const lastPos = rollbackDoc.get('_lastServerPosition');
            await rollbackDoc.update({
              $set: {
                x: lastPos.x,
                y: lastPos.y,
                z: lastPos.z,
                _optimistic: false,
                _lastServerPosition: null,
              },
            });
          }
        }
      }
    } catch (error) {
      console.error('World tokens replication error:', error);
    }
  })();

  // Return cleanup function
  return () => {
    abortController.abort();
  };
}

/**
 * Compute derived stats from base token data.
 *
 * Called locally after token updates to avoid sending derived data over network.
 * Example: health_percentage, effective_attack, etc.
 */
export function computeTokenDerivedStats(token: WorldTokenDoc) {
  const health = token.health ?? 0;
  const maxHealth = token.max_health ?? 1;
  const healthPercentage =
    maxHealth > 0 ? (health / maxHealth) * 100 : 0;

  return {
    healthPercentage: Math.max(0, Math.min(100, healthPercentage)),
    isDead: health <= 0,
    isFullHealth: health >= maxHealth,
  };
}

/**
 * Prepare token for optimistic update.
 *
 * Saves the current position as a rollback point before making a change.
 */
export async function prepareOptimisticUpdate(
  doc: any
): Promise<void> {
  const lastServerPosition = {
    x: doc.get('x'),
    y: doc.get('y'),
    z: doc.get('z'),
  };

  await doc.update({
    $set: {
      _optimistic: true,
      _lastServerPosition: lastServerPosition,
    },
  });
}

/**
 * Apply optimistic position update (immediately visible to user).
 *
 * Called before the server request is sent, for instant UI feedback.
 */
export async function applyOptimisticMove(
  doc: any,
  x: number,
  y: number,
  z: number
): Promise<void> {
  await prepareOptimisticUpdate(doc);
  await doc.update({
    $set: {
      x,
      y,
      z,
    },
  });
}
