/**
 * worldTokensSceneCollection.ts
 * RxDB collection schema and replication setup for scene-scoped tokens
 * (the modern `tokens` table, native canvas authoring).
 *
 * Named distinctly from `worldTokensCollection.ts`, which already exists
 * for the legacy world-scoped `world_tokens` table/collection — this is
 * additive, not a replacement. Mirrors `worldWallsCollection.ts`'s shape
 * (offline-first caching, event-driven replication from the world_events
 * NOTIFY backplane, optimistic updates). Field names match the GraphQL
 * `Token`/`GraphQLToken` type on the wire (camelCase).
 */

import { RxCollection, RxJsonSchema } from 'rxdb';

/**
 * JSON Schema for the world_scene_tokens RxDB collection.
 * Must match the server's GraphQLToken type (src/server/src/graphql.rs).
 */
export const worldSceneTokensSchema: RxJsonSchema<any> = {
  title: 'World Scene Tokens',
  description: 'Tokens authored on world scenes (scene-scoped, replaces per-world token fixtures)',
  version: 0,
  keyCompression: false,
  primaryKey: 'tokenId',
  type: 'object',
  properties: {
    tokenId: {
      type: 'string',
      description: 'Unique token ID (UUID)',
      maxLength: 36,
    },
    sceneId: {
      type: 'string',
      description: 'Scene this token belongs to',
      maxLength: 36,
      index: true,
    },
    actorId: {
      type: 'string',
      description: 'Actor this token represents, if any',
      maxLength: 36,
      nullable: true,
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
    rotation: {
      type: 'number',
      description: 'Rotation in degrees',
      default: 0,
    },
    scale: {
      type: 'number',
      description: 'Uniform scale factor',
      default: 1,
    },
    metadata: {
      type: 'object',
      description: 'Arbitrary GM-authored metadata for this token',
      nullable: true,
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
    ownerUserId: {
      type: 'string',
      description: 'Spec 004: the player who controls this token (primary or GM-granted)',
      maxLength: 36,
      nullable: true,
    },
    isPrimary: {
      type: 'boolean',
      description: 'Spec 004: true for exactly one token per (sceneId, ownerUserId)',
      default: false,
    },
    photoUrl: {
      type: 'string',
      description: 'Spec 004: player-/GM-editable avatar override; falls back to Dicebear when null',
      nullable: true,
    },
    health: {
      type: 'number',
      description: 'Spec 004 (ported from legacy world_tokens): current HP',
      nullable: true,
    },
    maxHealth: {
      type: 'number',
      description: 'Spec 004 (ported from legacy world_tokens): max HP',
      nullable: true,
    },

    // Client-side extensions (not sent to server)
    _optimistic: {
      type: 'boolean',
      description: 'Whether this token represents an optimistic update',
      default: false,
    },
  },
  required: [
    'tokenId',
    'sceneId',
    'x',
    'y',
    'rotation',
    'scale',
    'createdAt',
    'updatedAt',
  ],
  indexes: [['sceneId', 'updatedAt']],
};

/**
 * Type definition for WorldSceneToken documents in RxDB.
 */
export interface WorldSceneTokenDoc {
  tokenId: string;
  sceneId: string;
  actorId?: string | null;
  x: number;
  y: number;
  rotation: number;
  scale: number;
  metadata?: Record<string, unknown> | null;
  createdAt: string;
  updatedAt: string;
  ownerUserId?: string | null;
  isPrimary?: boolean;
  photoUrl?: string | null;
  health?: number | null;
  maxHealth?: number | null;

  // Client extensions
  _optimistic?: boolean;
}

/**
 * Set up replication for the world_scene_tokens collection.
 *
 * Mirrors setupWorldWallsReplication's event-driven design: rather than a
 * dedicated subscription, scene tokens reuse the generic
 * `worldEventsCreated(worldId)` NOTIFY stream shared by every canvas-
 * authoring entity (src/server/src/world_events.rs). Token events use
 * `event_code = 14` and carry `{ action, token_id, scene_id }` — not the
 * full token payload — so on receipt this refetches the affected scene's
 * tokens via GraphQL (see api/tokens.ts#getTokens) rather than trying to
 * reconstruct a token from the notify payload alone.
 */
export async function setupWorldSceneTokensReplication(
  collection: RxCollection<WorldSceneTokenDoc>,
  graphqlSubscription: AsyncIterable<any>,
  sceneId: string,
  fetchTokens: (sceneId: string) => Promise<WorldSceneTokenDoc[]>
): Promise<() => void> {
  const abortController = new AbortController();
  const TOKEN_EVENT_CODE = 14;

  (async () => {
    try {
      for await (const event of graphqlSubscription) {
        if (abortController.signal.aborted) break;

        const eventCode = event.event_code ?? event.eventCode;
        const tokenEvent = event.token_event ?? event.tokenEvent;

        if (eventCode !== TOKEN_EVENT_CODE) {
          continue;
        }

        const eventSceneId = tokenEvent?.scene_id ?? tokenEvent?.sceneId;
        if (eventSceneId && eventSceneId !== sceneId) {
          continue;
        }

        const action = tokenEvent?.action;
        const tokenId = tokenEvent?.token_id ?? tokenEvent?.tokenId;

        if (action === 'deleted' && tokenId) {
          const doc = await collection.findOne(tokenId).exec();
          if (doc) {
            await doc.remove();
          }
          continue;
        }

        // created/updated: re-fetch the scene's tokens and upsert.
        const tokens = await fetchTokens(sceneId);
        for (const token of tokens) {
          await collection.upsert({ ...token, _optimistic: false });
        }
      }
    } catch (error) {
      console.error('World scene tokens replication error:', error);
    }
  })();

  return () => {
    abortController.abort();
  };
}

/**
 * Prepare a token document for optimistic update, before the mutation
 * response is known.
 */
export async function applyOptimisticSceneTokenUpdate(
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
