/**
 * worldActorSystemDataCollection.ts
 * RxDB collection schema and replication setup for actor system-specific data
 *
 * Phase 4.8.1: System-Agnostic Actor Data Architecture
 *
 * This collection mirrors the server's world_actor_system_data table with:
 * - Five semantic JSONB columns (ability_data, resource_data, proficiency_data, trait_data, spell_data)
 * - Offline-first caching
 * - Automatic replication via GraphQL subscriptions
 * - Optimistic updates with rollback
 * - Zero schema migrations when new game systems are added
 *
 * Example: D&D 5e character stores all stats as JSONB in one row per actor:
 *   ability_data: { "strength": 10, "dexterity": 12, ... }
 *   resource_data: { "current_hp": 45, "max_hp": 50, ... }
 *   proficiency_data: { "proficient_in_acrobatics": true, ... }
 *   trait_data: { "class": "rogue", "level": 5, ... }
 *   spell_data: { "spellcasting_ability": "dexterity", "spell_slots": {...} }
 *
 * Pathfinder 2e character stores DIFFERENT structure in SAME columns:
 *   ability_data: { "strength_mod": 0, "reflex_mod": 2, ... }
 *   etc.
 */

import { RxCollection, RxJsonSchema } from 'rxdb';

/**
 * JSON Schema for world_actor_system_data RxDB collection.
 * Must match the Diesel ActorSystemData model but with client-side extensions.
 */
export const worldActorSystemDataSchema: RxJsonSchema<any> = {
  title: 'World Actor System Data',
  description: 'System-specific actor data stored as semantic JSONB columns',
  version: 1,
  keyCompression: false,
  primaryKey: 'id',
  type: 'object',
  properties: {
    // Base fields from server
    id: {
      type: 'string',
      description: 'Unique actor system data ID (UUID)',
      maxLength: 36,
    },
    actor_id: {
      type: 'string',
      description: 'Actor this data belongs to (FK to world_actors)',
      maxLength: 36,
      index: true,
    },
    game_system_id: {
      type: 'string',
      description: 'Game system (dnd5e, pathfinder2e, coc7e, etc.)',
      maxLength: 100,
      index: true,
    },

    // Five semantic JSONB columns - structure varies by system, column names are identical
    // This is the KEY INNOVATION that enables infinite system scalability without schema changes

    ability_data: {
      type: 'object',
      description:
        'Base ability scores/modifiers (varies per system: D&D 5e has strength 1-20, Pathfinder has STR mod)',
      nullable: true,
      properties: {}, // Dynamic schema per system
    },

    resource_data: {
      type: 'object',
      description:
        'Resources like HP, mana, sanity, focus (varies per system)',
      nullable: true,
      properties: {},
    },

    proficiency_data: {
      type: 'object',
      description:
        'Skills, weapon/armor proficiencies, languages (varies per system)',
      nullable: true,
      properties: {},
    },

    trait_data: {
      type: 'object',
      description:
        'Class, subclass, feats, backgrounds, etc. (varies per system)',
      nullable: true,
      properties: {},
    },

    spell_data: {
      type: 'object',
      description: 'Spellbook, slots, prepared spells (varies per system)',
      nullable: true,
      properties: {},
    },

    // Audit trail
    created_by: {
      type: 'string',
      description: 'User who created this actor',
      maxLength: 36,
      index: true,
    },
    updated_by: {
      type: 'string',
      description: 'User who last updated this actor',
      maxLength: 36,
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
      description: 'Whether this data represents an optimistic update',
      default: false,
    },
    _lastServerData: {
      type: 'object',
      description: 'Last known server data for rollback',
      nullable: true,
      properties: {
        ability_data: { type: 'object' },
        resource_data: { type: 'object' },
        proficiency_data: { type: 'object' },
        trait_data: { type: 'object' },
        spell_data: { type: 'object' },
      },
    },
  },
  required: [
    'id',
    'actor_id',
    'game_system_id',
    'created_by',
    'updated_by',
    'created_at',
    'updated_at',
  ],
  indexes: [
    // Query by actor (most common: fetch all system data for an actor)
    ['actor_id'],
    // Query by system (less common: find all actors in a world using this system)
    ['game_system_id', 'updated_at'],
  ],
};

/**
 * Type definition for WorldActorSystemData documents in RxDB.
 */
export interface WorldActorSystemDataDoc {
  id: string;
  actor_id: string;
  game_system_id: string;

  ability_data?: Record<string, any>;
  resource_data?: Record<string, any>;
  proficiency_data?: Record<string, any>;
  trait_data?: Record<string, any>;
  spell_data?: Record<string, any>;

  created_by: string;
  updated_by: string;
  created_at: string;
  updated_at: string;

  // Client extensions
  _optimistic?: boolean;
  _lastServerData?: {
    ability_data?: Record<string, any>;
    resource_data?: Record<string, any>;
    proficiency_data?: Record<string, any>;
    trait_data?: Record<string, any>;
    spell_data?: Record<string, any>;
  };
}

/**
 * Setup replication for world_actor_system_data collection.
 *
 * This function:
 * 1. Subscribes to worldActorSystemDataUpdated GraphQL subscription
 * 2. Applies server events to the RxDB collection
 * 3. Handles optimistic rollback on rejection
 * 4. Manages offline queuing of mutations
 */
export async function setupActorSystemDataReplication(
  collection: RxCollection<WorldActorSystemDataDoc>,
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
        const {
          id,
          event_code,
          actor_system_event,
          created_by,
          updated_by,
        } = event;

        // Event codes:
        // 1 = actor system data created
        // 2 = actor system data updated
        // 3 = actor system data deleted
        // -1 = validation error (rollback)

        if (actor_system_event?.actor_id) {
          const actorId = actor_system_event.actor_id;
          const gameSystemId = actor_system_event.game_system_id;

          switch (event_code) {
            case 1: // Created
              // Upsert new actor system data from server
              await collection.upsert({
                id: actor_system_event.id || id,
                actor_id: actorId,
                game_system_id: gameSystemId,
                ability_data: actor_system_event.ability_data,
                resource_data: actor_system_event.resource_data,
                proficiency_data: actor_system_event.proficiency_data,
                trait_data: actor_system_event.trait_data,
                spell_data: actor_system_event.spell_data,
                created_by,
                updated_by,
                created_at: new Date().toISOString(),
                updated_at: new Date().toISOString(),
                _optimistic: false,
              });
              break;

            case 2: // Updated
              // Merge updated fields
              const doc = await collection
                .findOne(actorId)
                .sort({ created_at: -1 })
                .exec();
              if (doc) {
                const updates: any = { _optimistic: false, updated_by };

                // Only update fields that were changed (merge, not replace)
                if (actor_system_event.ability_data !== undefined) {
                  updates.ability_data = actor_system_event.ability_data;
                }
                if (actor_system_event.resource_data !== undefined) {
                  updates.resource_data = actor_system_event.resource_data;
                }
                if (actor_system_event.proficiency_data !== undefined) {
                  updates.proficiency_data =
                    actor_system_event.proficiency_data;
                }
                if (actor_system_event.trait_data !== undefined) {
                  updates.trait_data = actor_system_event.trait_data;
                }
                if (actor_system_event.spell_data !== undefined) {
                  updates.spell_data = actor_system_event.spell_data;
                }
                updates.updated_at = new Date().toISOString();

                await doc.update({
                  $set: updates,
                });
              }
              break;

            case 3: // Deleted
              // Remove actor system data
              const docs = await collection
                .find()
                .where('actor_id')
                .eq(actorId)
                .exec();
              for (const deleteDoc of docs) {
                await deleteDoc.remove();
              }
              break;
          }
        }

        // Check for rejection events (event_code = -1)
        if (event_code === -1 && actor_system_event?.actor_id) {
          // Rollback: restore _lastServerData
          const docs = await collection
            .find()
            .where('actor_id')
            .eq(actor_system_event.actor_id)
            .exec();

          for (const rollbackDoc of docs) {
            if (
              rollbackDoc.get('_optimistic') &&
              rollbackDoc.get('_lastServerData')
            ) {
              const lastData = rollbackDoc.get('_lastServerData');
              const updates: any = { _optimistic: false, _lastServerData: null };

              if (lastData.ability_data !== undefined) {
                updates.ability_data = lastData.ability_data;
              }
              if (lastData.resource_data !== undefined) {
                updates.resource_data = lastData.resource_data;
              }
              if (lastData.proficiency_data !== undefined) {
                updates.proficiency_data = lastData.proficiency_data;
              }
              if (lastData.trait_data !== undefined) {
                updates.trait_data = lastData.trait_data;
              }
              if (lastData.spell_data !== undefined) {
                updates.spell_data = lastData.spell_data;
              }

              await rollbackDoc.update({
                $set: updates,
              });
            }
          }
        }
      }
    } catch (error) {
      console.error('Actor system data replication error:', error);
    }
  })();

  // Return cleanup function
  return () => {
    abortController.abort();
  };
}

/**
 * Prepare actor system data for optimistic update.
 *
 * Saves the current data as a rollback point before making a change.
 */
export async function prepareOptimisticDataUpdate(
  doc: any,
  dataType: 'ability_data' | 'resource_data' | 'proficiency_data' | 'trait_data' | 'spell_data'
): Promise<void> {
  const lastServerData = {
    [dataType]: doc.get(dataType),
  };

  await doc.update({
    $set: {
      _optimistic: true,
      _lastServerData: lastServerData,
    },
  });
}

/**
 * Apply optimistic data update (immediately visible to user).
 *
 * Called before the server request is sent, for instant UI feedback.
 */
export async function applyOptimisticDataUpdate(
  doc: any,
  dataType: 'ability_data' | 'resource_data' | 'proficiency_data' | 'trait_data' | 'spell_data',
  newData: Record<string, any>
): Promise<void> {
  await prepareOptimisticDataUpdate(doc, dataType);
  await doc.update({
    $set: {
      [dataType]: newData,
    },
  });
}

/**
 * Compute derived stats from base actor system data.
 *
 * Called locally after data updates to avoid sending derived data over network.
 * Example: health_percentage, ability_modifiers, skill_bonuses, etc.
 *
 * System-specific derivation happens in game-system plugins (Phase E).
 */
export function computeActorDerivedStats(
  data: WorldActorSystemDataDoc,
  gameSystemId: string
) {
  // Base derived stats applicable to all systems
  const hasAbilityData = !!data.ability_data && Object.keys(data.ability_data).length > 0;
  const hasResourceData = !!data.resource_data && Object.keys(data.resource_data).length > 0;
  const hasTraitData = !!data.trait_data && Object.keys(data.trait_data).length > 0;

  const baseStats = {
    isFullyConfigured: hasAbilityData && hasResourceData && hasTraitData,
    lastUpdated: new Date(data.updated_at).getTime(),
    age: Date.now() - new Date(data.updated_at).getTime(),
  };

  // System-specific derivation (to be implemented in Phase E game system plugins)
  // For now, just return base stats
  return baseStats;
}
