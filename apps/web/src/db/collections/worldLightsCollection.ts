/**
 * worldLightsCollection.ts
 * RxDB collection schema and replication setup for scene light sources.
 *
 * Mirrors worldWallsCollection.ts's shape (offline-first caching,
 * event-driven replication from the world_events NOTIFY backplane,
 * optimistic updates) but for the light sources introduced by native
 * canvas authoring (specs/001-bevy-canvas-authoring, User Story 3). Field
 * names match the GraphQL `LightSource` type on the wire (camelCase)
 * rather than the Diesel model's snake_case, consistent with the walls
 * collection's convention.
 */

import { RxCollection, RxJsonSchema } from 'rxdb';

/**
 * JSON Schema for the world_lights RxDB collection.
 * Must match the server's GraphQLLightSource type (src/server/src/graphql.rs).
 */
export const worldLightsSchema: RxJsonSchema<any> = {
  title: 'World Lights',
  description: 'Light sources authored on world scenes',
  version: 0,
  keyCompression: false,
  primaryKey: 'lightId',
  type: 'object',
  properties: {
    lightId: {
      type: 'string',
      description: 'Unique light source ID (UUID)',
      maxLength: 36,
    },
    sceneId: {
      type: 'string',
      description: 'Scene this light source belongs to',
      maxLength: 36,
      index: true,
    },
    x: {
      type: 'number',
      description: 'Light position X coordinate',
      minimum: -Infinity,
      maximum: Infinity,
    },
    y: {
      type: 'number',
      description: 'Light position Y coordinate',
      minimum: -Infinity,
      maximum: Infinity,
    },
    radius: {
      type: 'number',
      description: 'Light radius',
      minimum: 0,
      maximum: Infinity,
    },
    intensity: {
      type: 'number',
      description: 'Light intensity',
      minimum: 0,
      maximum: Infinity,
    },
    color: {
      type: 'string',
      description: 'Light color (e.g. hex string)',
      nullable: true,
    },
    attachedTokenId: {
      type: 'string',
      description: 'Token this light is attached to, if any',
      maxLength: 36,
      nullable: true,
    },
    castsShadows: {
      type: 'boolean',
      description: 'Whether this light casts shadows',
      default: true,
    },
    metadata: {
      type: 'object',
      description: 'Arbitrary GM-authored metadata for this light',
      nullable: true,
    },
    createdBy: {
      type: 'string',
      description: 'User who created this light source',
      maxLength: 36,
    },
    updatedBy: {
      type: 'string',
      description: 'User who last updated this light source',
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
      description: 'Whether this light source represents an optimistic update',
      default: false,
    },
  },
  required: [
    'lightId',
    'sceneId',
    'x',
    'y',
    'radius',
    'intensity',
    'castsShadows',
    'createdBy',
    'updatedBy',
    'createdAt',
    'updatedAt',
  ],
  indexes: [['sceneId', 'updatedAt']],
};

/**
 * Type definition for WorldLight documents in RxDB.
 */
export interface WorldLightDoc {
  lightId: string;
  sceneId: string;
  x: number;
  y: number;
  radius: number;
  intensity: number;
  color?: string | null;
  attachedTokenId?: string | null;
  castsShadows: boolean;
  metadata?: Record<string, unknown> | null;
  createdBy: string;
  updatedBy: string;
  createdAt: string;
  updatedAt: string;

  // Client extensions
  _optimistic?: boolean;
}

/**
 * Set up replication for the world_lights collection.
 *
 * Mirrors setupWorldWallsReplication's event-driven design: light events
 * reuse the generic `worldEventsCreated(worldId)` NOTIFY stream shared by
 * every canvas-authoring entity (src/server/src/world_events.rs). Light
 * events use `event_code = 11` and carry `{ action, light_id, scene_id }`
 * — not the full light payload — so on receipt this refetches the
 * affected scene's lights via GraphQL (see api/lights.ts#getLights)
 * rather than trying to reconstruct a light from the notify payload
 * alone.
 */
export async function setupWorldLightsReplication(
  collection: RxCollection<WorldLightDoc>,
  graphqlSubscription: AsyncIterable<any>,
  sceneId: string,
  fetchLights: (sceneId: string) => Promise<WorldLightDoc[]>
): Promise<() => void> {
  const abortController = new AbortController();
  const LIGHT_EVENT_CODE = 11;

  (async () => {
    try {
      for await (const event of graphqlSubscription) {
        if (abortController.signal.aborted) break;

        const eventCode = event.event_code ?? event.eventCode;
        const lightEvent = event.token_event ?? event.tokenEvent;

        if (eventCode !== LIGHT_EVENT_CODE) {
          continue;
        }

        const eventSceneId = lightEvent?.scene_id ?? lightEvent?.sceneId;
        if (eventSceneId && eventSceneId !== sceneId) {
          continue;
        }

        const action = lightEvent?.action;
        const lightId = lightEvent?.light_id ?? lightEvent?.lightId;

        if (action === 'deleted' && lightId) {
          const doc = await collection.findOne(lightId).exec();
          if (doc) {
            await doc.remove();
          }
          continue;
        }

        // created/updated: re-fetch the scene's lights and upsert.
        const lights = await fetchLights(sceneId);
        for (const light of lights) {
          await collection.upsert({ ...light, _optimistic: false });
        }
      }
    } catch (error) {
      console.error('World lights replication error:', error);
    }
  })();

  return () => {
    abortController.abort();
  };
}

/**
 * Prepare a light source document for optimistic update, before the
 * mutation response is known.
 */
export async function applyOptimisticLightUpdate(
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
