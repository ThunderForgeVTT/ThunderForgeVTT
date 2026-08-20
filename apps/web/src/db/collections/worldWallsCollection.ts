/**
 * worldWallsCollection.ts
 * RxDB collection schema and replication setup for scene walls.
 *
 * Mirrors worldTokensCollection.ts's shape (offline-first caching,
 * event-driven replication from the world_events NOTIFY backplane,
 * optimistic updates) but for the walls introduced by native canvas
 * authoring (specs/001-bevy-canvas-authoring). Field names match the
 * GraphQL `Wall` type on the wire (camelCase) rather than the Diesel
 * model's snake_case, since walls are the newer, camelCase-first
 * collection convention used by engine/world/sync/schemas.ts.
 */

import { RxCollection, RxJsonSchema } from 'rxdb';

export type WallDoorState = 'none' | 'open' | 'closed';

/**
 * JSON Schema for the world_walls RxDB collection.
 * Must match the server's GraphQLWall type (src/server/src/graphql.rs).
 */
export const worldWallsSchema: RxJsonSchema<any> = {
  title: 'World Walls',
  description: 'Wall segments authored on world scenes',
  version: 0,
  keyCompression: false,
  primaryKey: 'wallId',
  type: 'object',
  properties: {
    wallId: {
      type: 'string',
      description: 'Unique wall ID (UUID)',
      maxLength: 36,
    },
    sceneId: {
      type: 'string',
      description: 'Scene this wall belongs to',
      maxLength: 36,
      index: true,
    },
    x1: {
      type: 'number',
      description: 'Start point X coordinate',
      minimum: -Infinity,
      maximum: Infinity,
    },
    y1: {
      type: 'number',
      description: 'Start point Y coordinate',
      minimum: -Infinity,
      maximum: Infinity,
    },
    x2: {
      type: 'number',
      description: 'End point X coordinate',
      minimum: -Infinity,
      maximum: Infinity,
    },
    y2: {
      type: 'number',
      description: 'End point Y coordinate',
      minimum: -Infinity,
      maximum: Infinity,
    },
    blocksVision: {
      type: 'boolean',
      description: 'Whether this wall blocks line-of-sight',
      default: true,
    },
    blocksMovement: {
      type: 'boolean',
      description: 'Whether this wall blocks token movement',
      default: false,
    },
    doorState: {
      type: 'string',
      description: 'Door state: none, open, or closed',
      enum: ['none', 'open', 'closed'],
      default: 'none',
    },
    metadata: {
      type: 'object',
      description: 'Arbitrary GM-authored metadata for this wall',
      nullable: true,
    },
    createdBy: {
      type: 'string',
      description: 'User who created this wall',
      maxLength: 36,
    },
    updatedBy: {
      type: 'string',
      description: 'User who last updated this wall',
      maxLength: 36,
    },
    createdAt: {
      type: 'string',
      format: 'date-time',
      description: 'ISO 8601 creation timestamp',
      maxLength: 64,
    },
    updatedAt: {
      type: 'string',
      format: 'date-time',
      description: 'ISO 8601 last update timestamp',
      maxLength: 64,
    },

    // Client-side extensions (not sent to server)
    _optimistic: {
      type: 'boolean',
      description: 'Whether this wall represents an optimistic update',
      default: false,
    },
  },
  required: [
    'wallId',
    'sceneId',
    'x1',
    'y1',
    'x2',
    'y2',
    'blocksVision',
    'blocksMovement',
    'doorState',
    'createdBy',
    'updatedBy',
    'createdAt',
    'updatedAt',
  ],
  indexes: [['sceneId', 'updatedAt']],
};

/**
 * Type definition for WorldWall documents in RxDB.
 */
export interface WorldWallDoc {
  wallId: string;
  sceneId: string;
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  blocksVision: boolean;
  blocksMovement: boolean;
  doorState: WallDoorState;
  metadata?: Record<string, unknown> | null;
  createdBy: string;
  updatedBy: string;
  createdAt: string;
  updatedAt: string;

  // Client extensions
  _optimistic?: boolean;
}

/**
 * Set up replication for the world_walls collection.
 *
 * Mirrors setupWorldTokensReplication's event-driven design: rather than
 * a dedicated `worldWallEventCreated` subscription, walls reuse the
 * generic `worldEventsCreated(worldId)` NOTIFY stream shared by every
 * canvas-authoring entity (src/server/src/world_events.rs). Wall events
 * use `event_code = 10` and carry `{ action, wall_id, scene_id }` — not
 * the full wall payload — so on receipt this refetches the affected
 * scene's walls via GraphQL (see api/walls.ts#getWalls) rather than
 * trying to reconstruct a wall from the notify payload alone.
 */
export async function setupWorldWallsReplication(
  collection: RxCollection<WorldWallDoc>,
  graphqlSubscription: AsyncIterable<any>,
  sceneId: string,
  fetchWalls: (sceneId: string) => Promise<WorldWallDoc[]>
): Promise<() => void> {
  const abortController = new AbortController();
  const WALL_EVENT_CODE = 10;

  (async () => {
    try {
      for await (const event of graphqlSubscription) {
        if (abortController.signal.aborted) break;

        const eventCode = event.event_code ?? event.eventCode;
        const wallEvent = event.token_event ?? event.tokenEvent;

        if (eventCode !== WALL_EVENT_CODE) {
          continue;
        }

        const eventSceneId = wallEvent?.scene_id ?? wallEvent?.sceneId;
        if (eventSceneId && eventSceneId !== sceneId) {
          continue;
        }

        const action = wallEvent?.action;
        const wallId = wallEvent?.wall_id ?? wallEvent?.wallId;

        if (action === 'deleted' && wallId) {
          const doc = await collection.findOne(wallId).exec();
          if (doc) {
            await doc.remove();
          }
          continue;
        }

        // created/updated: re-fetch the scene's walls and upsert.
        const walls = await fetchWalls(sceneId);
        for (const wall of walls) {
          await collection.upsert({ ...wall, _optimistic: false });
        }
      }
    } catch (error) {
      console.error('World walls replication error:', error);
    }
  })();

  return () => {
    abortController.abort();
  };
}

/**
 * Prepare a wall document for optimistic update, before the mutation
 * response is known.
 */
export async function applyOptimisticWallUpdate(
  doc: any,
  changes: Record<string, unknown>
): Promise<void> {
  await doc.update({
    $set: {
      ...changes,
      _optimistic: true,
    },
  });
}
