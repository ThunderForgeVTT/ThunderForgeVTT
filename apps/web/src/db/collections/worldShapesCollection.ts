/**
 * worldShapesCollection.ts
 * RxDB collection schema and replication setup for scene shapes (native
 * canvas authoring: freehand strokes, rectangles, ellipses, lines/arrows,
 * text labels drawn directly on the game canvas).
 *
 * Mirrors worldWallsCollection.ts's shape (offline-first caching,
 * event-driven replication from the world_events NOTIFY backplane,
 * optimistic updates) but for shapes (specs/001-bevy-canvas-authoring).
 * Field names match the GraphQL `Shape` type on the wire (camelCase)
 * rather than the Diesel model's snake_case, per the walls collection's
 * convention.
 */

import { RxCollection, RxJsonSchema } from 'rxdb';

export type ShapeKind = 'STROKE' | 'RECT' | 'ELLIPSE' | 'LINE' | 'TEXT';

/**
 * JSON Schema for the world_shapes RxDB collection.
 * Must match the server's GraphQLShape type (src/server/src/graphql.rs).
 */
export const worldShapesSchema: RxJsonSchema<any> = {
  title: 'World Shapes',
  description: 'Freehand/drawn shapes authored on world scenes',
  version: 0,
  keyCompression: false,
  primaryKey: 'shapeId',
  type: 'object',
  properties: {
    shapeId: {
      type: 'string',
      description: 'Unique shape ID (UUID)',
      maxLength: 36,
    },
    sceneId: {
      type: 'string',
      description: 'Scene this shape belongs to',
      maxLength: 36,
      index: true,
    },
    kind: {
      type: 'string',
      description: 'Kind of shape: STROKE, RECT, ELLIPSE, LINE, or TEXT',
      enum: ['STROKE', 'RECT', 'ELLIPSE', 'LINE', 'TEXT'],
    },
    geometry: {
      type: 'object',
      description:
        'Opaque JSON geometry blob (points, bounds, etc.) — stored as-is, interpreted by the engine',
      additionalProperties: true,
    },
    text: {
      type: 'string',
      description: 'Text content, for TEXT-kind shapes',
      nullable: true,
    },
    style: {
      type: 'object',
      description: 'Opaque JSON style blob (color, stroke width, etc.)',
      nullable: true,
      additionalProperties: true,
    },
    visibleToPlayers: {
      type: 'boolean',
      description: 'Whether this shape is visible to non-GM participants',
      default: false,
    },
    metadata: {
      type: 'object',
      description: 'Arbitrary GM-authored metadata for this shape',
      nullable: true,
    },
    createdBy: {
      type: 'string',
      description: 'User who created this shape',
      maxLength: 36,
    },
    updatedBy: {
      type: 'string',
      description: 'User who last updated this shape',
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
      description: 'Whether this shape represents an optimistic update',
      default: false,
    },
  },
  required: [
    'shapeId',
    'sceneId',
    'kind',
    'geometry',
    'visibleToPlayers',
    'createdBy',
    'updatedBy',
    'createdAt',
    'updatedAt',
  ],
  indexes: [['sceneId', 'updatedAt']],
};

/**
 * Type definition for WorldShape documents in RxDB.
 */
export interface WorldShapeDoc {
  shapeId: string;
  sceneId: string;
  kind: ShapeKind;
  geometry: Record<string, unknown>;
  text?: string | null;
  style?: Record<string, unknown> | null;
  visibleToPlayers: boolean;
  metadata?: Record<string, unknown> | null;
  createdBy: string;
  updatedBy: string;
  createdAt: string;
  updatedAt: string;

  // Client extensions
  _optimistic?: boolean;
}

/**
 * Set up replication for the world_shapes collection.
 *
 * Mirrors setupWorldWallsReplication's event-driven design: rather than
 * a dedicated `worldShapeEventCreated` subscription, shapes reuse the
 * generic `worldEventsCreated(worldId)` NOTIFY stream shared by every
 * canvas-authoring entity (src/server/src/world_events.rs). Shape events
 * use `event_code = 12` and carry `{ action, shape_id, scene_id }` — not
 * the full shape payload — so on receipt this refetches the affected
 * scene's shapes via GraphQL (see api/shapes.ts#getShapes) rather than
 * trying to reconstruct a shape from the notify payload alone.
 */
export async function setupWorldShapesReplication(
  collection: RxCollection<WorldShapeDoc>,
  graphqlSubscription: AsyncIterable<any>,
  sceneId: string,
  fetchShapes: (sceneId: string) => Promise<WorldShapeDoc[]>
): Promise<() => void> {
  const abortController = new AbortController();
  const SHAPE_EVENT_CODE = 12;

  (async () => {
    try {
      for await (const event of graphqlSubscription) {
        if (abortController.signal.aborted) break;

        const eventCode = event.event_code ?? event.eventCode;
        const shapeEvent = event.token_event ?? event.tokenEvent;

        if (eventCode !== SHAPE_EVENT_CODE) {
          continue;
        }

        const eventSceneId = shapeEvent?.scene_id ?? shapeEvent?.sceneId;
        if (eventSceneId && eventSceneId !== sceneId) {
          continue;
        }

        const action = shapeEvent?.action;
        const shapeId = shapeEvent?.shape_id ?? shapeEvent?.shapeId;

        if (action === 'deleted' && shapeId) {
          const doc = await collection.findOne(shapeId).exec();
          if (doc) {
            await doc.remove();
          }
          continue;
        }

        // created/updated: re-fetch the scene's shapes and upsert.
        const shapes = await fetchShapes(sceneId);
        for (const shape of shapes) {
          await collection.upsert({ ...shape, _optimistic: false });
        }
      }
    } catch (error) {
      console.error('World shapes replication error:', error);
    }
  })();

  return () => {
    abortController.abort();
  };
}

/**
 * Prepare a shape document for optimistic update, before the mutation
 * response is known.
 */
export async function applyOptimisticShapeUpdate(
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
