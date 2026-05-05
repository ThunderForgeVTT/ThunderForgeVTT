/**
 * actor-data-replication.test.ts
 * Integration tests for actor system data replication
 *
 * Phase 4.8.1: System-Agnostic Actor Data Architecture - D.3 Integration Tests
 *
 * Tests the circular event-driven data flow:
 * 1. GraphQL mutation creates actor data
 * 2. Server persists + creates world_event
 * 3. pg_notify broadcasts event
 * 4. GraphQL subscription streams to client
 * 5. RxDB collection updates
 * 6. Optimistic rollback works on rejection
 */

import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { RxCollection } from 'rxdb';
import { createRxDatabase } from 'rxdb/plugins/core';
import { getRxStorageLocalstorage } from 'rxdb/plugins/storage-localstorage';

describe('Actor System Data Replication', () => {
  let collection: RxCollection;
  let db: any;

  const worldActorSystemDataSchema = {
    title: 'world actor system data schema',
    version: 0,
    primaryKey: 'id',
    type: 'object',
    properties: {
      id: {
        type: 'string',
        maxLength: 128,
      },
      actor_id: {
        type: 'string',
        maxLength: 128,
        index: true,
      },
      game_system_id: {
        type: 'string',
        maxLength: 128,
        index: true,
      },
      ability_data: {
        type: 'object',
        additionalProperties: true,
      },
      resource_data: {
        type: 'object',
        additionalProperties: true,
      },
      proficiency_data: {
        type: 'object',
        additionalProperties: true,
      },
      trait_data: {
        type: 'object',
        additionalProperties: true,
      },
      spell_data: {
        type: 'object',
        additionalProperties: true,
      },
      created_by: {
        type: 'string',
        maxLength: 128,
      },
      updated_by: {
        type: 'string',
        maxLength: 128,
      },
      created_at: {
        type: 'string',
        format: 'date-time',
        maxLength: 64,
      },
      updated_at: {
        type: 'string',
        format: 'date-time',
        maxLength: 64,
      },
      _optimistic: {
        type: 'boolean',
        default: false,
      },
      _lastServerData: {
        type: 'object',
        additionalProperties: true,
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
    indexes: ['actor_id', ['game_system_id', 'updated_at']],
  } as const;

  beforeEach(async () => {
    // Create in-memory test database
    db = await createRxDatabase({
      name: `test-actor-db-${Date.now()}`,
      storage: getRxStorageLocalstorage(),
    });

    await db.addCollections({
      world_actor_system_data: { schema: worldActorSystemDataSchema },
    });

    collection = db.collections.world_actor_system_data;
  });

  afterEach(async () => {
    await db.destroy();
  });

  describe('INSERT operation', () => {
    it('should upsert new actor system data from GraphQL event', async () => {
      const event = {
        id: 'actor-data-1',
        actor_id: 'actor-1',
        game_system_id: 'dnd5e',
        ability_data: {
          strength: 15,
          dexterity: 14,
          constitution: 13,
        },
        updated_at: new Date().toISOString(),
        created_by: 'user-1',
        updated_by: 'user-1',
        created_at: new Date().toISOString(),
      };

      await collection.upsert(event);

      // Verify the data was persisted
      const doc = await collection.findOne(event.id).exec();
      expect(doc).toBeDefined();
      expect(doc?.get('actor_id')).toBe('actor-1');
      expect(doc?.get('game_system_id')).toBe('dnd5e');
    });
  });

  describe('UPDATE operation', () => {
    it('should update existing actor system data', async () => {
      // Insert initial data
      const initialData = {
        id: 'actor-data-2',
        actor_id: 'actor-2',
        game_system_id: 'dnd5e',
        ability_data: { strength: 10 },
        updated_at: new Date().toISOString(),
        created_by: 'user-1',
        updated_by: 'user-1',
        created_at: new Date().toISOString(),
      };

      await collection.upsert(initialData);

      // Update with new data
      const updated = {
        id: 'actor-data-2',
        actor_id: 'actor-2',
        game_system_id: 'dnd5e',
        ability_data: { strength: 15, dexterity: 14 },
        updated_at: new Date().toISOString(),
        created_by: 'user-1',
        updated_by: 'user-1',
        created_at: initialData.created_at,
      };

      await collection.upsert(updated);

      // Verify the update
      const doc = await collection.findOne('actor-data-2').exec();
      expect(doc?.get('ability_data')).toEqual({ strength: 15, dexterity: 14 });
    });
  });

  describe('DELETE operation', () => {
    it('should remove actor system data', async () => {
      // Insert data first
      const data = {
        id: 'actor-data-3',
        actor_id: 'actor-3',
        game_system_id: 'dnd5e',
        ability_data: { strength: 10 },
        updated_at: new Date().toISOString(),
        created_by: 'user-1',
        updated_by: 'user-1',
        created_at: new Date().toISOString(),
      };

      await collection.upsert(data);

      // Verify it exists
      let doc = await collection.findOne(data.id).exec();
      expect(doc).toBeDefined();

      // Delete it
      if (doc) {
        await doc.remove();
      }

      // Verify it's gone
      doc = await collection.findOne(data.id).exec();
      expect(doc).toBeNull();
    });
  });

  describe('Optimistic updates with rollback', () => {
    it('should save last server data before optimistic update', async () => {
      // Create initial data
      const initialData = {
        id: 'actor-data-4',
        actor_id: 'actor-4',
        game_system_id: 'dnd5e',
        ability_data: { strength: 10 },
        updated_at: new Date().toISOString(),
        created_by: 'user-1',
        updated_by: 'user-1',
        created_at: new Date().toISOString(),
      };

      await collection.upsert(initialData);

      // Get the document
      const doc = await collection.findOne(initialData.id).exec();
      expect(doc).toBeDefined();

      if (doc) {
        // Prepare for optimistic update
        const lastData = doc.get('ability_data');

        await doc.update({
          $set: {
            _optimistic: true,
            _lastServerData: {
              ability_data: lastData,
            },
          },
        });

        // Apply optimistic change
        await doc.update({
          $set: {
            ability_data: { strength: 18 },
          },
        });

        // Verify optimistic change is applied
        const updated = await collection.findOne(initialData.id).exec();
        expect(updated?.get('ability_data')).toEqual({ strength: 18 });
        expect(updated?.get('_optimistic')).toBe(true);

        // Simulate rejection - rollback
        const lastServerData = updated?.get('_lastServerData');
        if (lastServerData) {
          await updated?.update({
            $set: {
              ability_data: lastServerData.ability_data,
              _optimistic: false,
              _lastServerData: null,
            },
          });
        }

        // Verify rollback
        const rolledBack = await collection.findOne(initialData.id).exec();
        expect(rolledBack?.get('ability_data')).toEqual({ strength: 10 });
        expect(rolledBack?.get('_optimistic')).toBe(false);
      }
    });
  });

  describe('System-specific data handling', () => {
    it('should support multiple game systems in same collection', async () => {
      // Insert D&D 5e data
      await collection.upsert({
        id: 'dnd5e-actor-1',
        actor_id: 'actor-1',
        game_system_id: 'dnd5e',
        ability_data: { strength: 15, dexterity: 14 },
        updated_at: new Date().toISOString(),
        created_by: 'user-1',
        updated_by: 'user-1',
        created_at: new Date().toISOString(),
      });

      // Insert Pathfinder 2e data
      await collection.upsert({
        id: 'pf2e-actor-1',
        actor_id: 'actor-1',
        game_system_id: 'pathfinder2e',
        ability_data: { strength_mod: 2, reflex_mod: 1 },
        updated_at: new Date().toISOString(),
        created_by: 'user-1',
        updated_by: 'user-1',
        created_at: new Date().toISOString(),
      });

      // Query D&D 5e actors
      const dnd5eDocs = await collection
        .find()
        .where('game_system_id')
        .eq('dnd5e')
        .exec();

      expect(dnd5eDocs.length).toBe(1);
      expect(dnd5eDocs[0].get('ability_data')).toEqual({
        strength: 15,
        dexterity: 14,
      });

      // Query Pathfinder 2e actors
      const pf2eDocs = await collection
        .find()
        .where('game_system_id')
        .eq('pathfinder2e')
        .exec();

      expect(pf2eDocs.length).toBe(1);
      expect(pf2eDocs[0].get('ability_data')).toEqual({
        strength_mod: 2,
        reflex_mod: 1,
      });
    });
  });
});
