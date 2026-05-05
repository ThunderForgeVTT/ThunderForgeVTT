/**
 * replication.ts
 * RxDB Replication Plugin for system-agnostic actor data
 *
 * Phase 4.8.1: System-Agnostic Actor Data Architecture - D.2 RxDB Replication
 *
 * This module implements the circular event-driven data flow:
 * 1. GraphQL subscription listens to worldActorSystemDataUpdated
 * 2. Parse incoming events and determine operation (INSERT/UPDATE/DELETE)
 * 3. Apply to RxDB collection via .upsert() or .remove()
 * 4. Handle optimistic rollback on rejection (event_code = -1)
 * 5. Batch updates for performance (250ms debounce)
 *
 * Design: Single replication task per world/subscription
 * Handles all game systems (D&D 5e, Pathfinder, CoC, etc.) with same logic
 */

import { RxCollection } from 'rxdb';
import { WorldActorSystemDataDoc } from '../../../db/collections/worldActorSystemDataCollection';

export interface ActorSystemDataEvent {
  id: string;
  actorId: string;
  gameSystemId: string;
  eventType: 'INSERT' | 'UPDATE' | 'DELETE';
  abilityData?: Record<string, any>;
  resourceData?: Record<string, any>;
  proficiencyData?: Record<string, any>;
  traitData?: Record<string, any>;
  spellData?: Record<string, any>;
  updatedAt: string;
}

export interface ReplicationOptions {
  collection: RxCollection<WorldActorSystemDataDoc>;
  worldId: string;
  gameSystemId: string;
  batchSize?: number; // Default 50 updates per batch
  debounceMs?: number; // Default 250ms debounce
  onError?: (error: Error) => void;
}

/**
 * Start replication of actor system data from GraphQL subscription
 *
 * Returns cleanup function to stop replication
 */
export async function startActorDataReplication(
  options: ReplicationOptions
): Promise<() => void> {
  const {
    collection,
    worldId,
    gameSystemId,
    batchSize = 50,
    debounceMs = 250,
    onError = () => {},
  } = options;

  const abortController = new AbortController();
  let updateBatch: ActorSystemDataEvent[] = [];
  let debounceTimer: ReturnType<typeof setTimeout> | null = null;

  /**
   * Process a batch of updates
   */
  const processBatch = async (events: ActorSystemDataEvent[]) => {
    if (events.length === 0) return;

    const operationsByType = {
      INSERT: events.filter((e) => e.eventType === 'INSERT'),
      UPDATE: events.filter((e) => e.eventType === 'UPDATE'),
      DELETE: events.filter((e) => e.eventType === 'DELETE'),
    };

    try {
      // Process deletes first (clean up state)
      for (const event of operationsByType.DELETE) {
        const doc = await collection.findOne(event.id).exec();
        if (doc) {
          await doc.remove();
        }
      }

      // Process inserts/updates with full data
      for (const event of [...operationsByType.INSERT, ...operationsByType.UPDATE]) {
        await collection.upsert({
          id: event.id,
          actor_id: event.actorId,
          game_system_id: event.gameSystemId,
          ability_data: event.abilityData,
          resource_data: event.resourceData,
          proficiency_data: event.proficiencyData,
          trait_data: event.traitData,
          spell_data: event.spellData,
          updated_at: event.updatedAt,
          _optimistic: false,
        } as any);
      }
    } catch (error) {
      onError(error instanceof Error ? error : new Error(String(error)));
    }
  };

  /**
   * Queue an event for batch processing
   */
  const queueEvent = async (event: ActorSystemDataEvent) => {
    updateBatch.push(event);

    // Clear existing timer
    if (debounceTimer) clearTimeout(debounceTimer);

    // Process immediately if batch is full
    if (updateBatch.length >= batchSize) {
      const batch = updateBatch;
      updateBatch = [];
      await processBatch(batch);
    } else {
      // Otherwise debounce
      debounceTimer = setTimeout(async () => {
        const batch = updateBatch;
        updateBatch = [];
        await processBatch(batch);
      }, debounceMs);
    }
  };

  /**
   * Fetch catch-up events for this world/system
   * Called on subscription start to handle missed events during disconnect
   */
  const fetchMissedEvents = async (): Promise<void> => {
    // PHASE D.3: Query server for events since last_updated_at
    // For now, this is a stub - full implementation pending backend query
    return Promise.resolve();
  };

  /**
   * Handle GraphQL subscription events
   */
  const handleSubscriptionEvent = async (rawEvent: any): Promise<void> => {
    if (abortController.signal.aborted) return;

    try {
      // Convert GraphQL response to internal event format
      const event: ActorSystemDataEvent = {
        id: rawEvent.id,
        actorId: rawEvent.actor_id || rawEvent.actorId,
        gameSystemId: rawEvent.game_system_id || rawEvent.gameSystemId,
        eventType: rawEvent.event_type || rawEvent.eventType || 'UPDATE',
        abilityData: rawEvent.ability_data || rawEvent.abilityData,
        resourceData: rawEvent.resource_data || rawEvent.resourceData,
        proficiencyData:
          rawEvent.proficiency_data || rawEvent.proficiencyData,
        traitData: rawEvent.trait_data || rawEvent.traitData,
        spellData: rawEvent.spell_data || rawEvent.spellData,
        updatedAt: rawEvent.updated_at || rawEvent.updatedAt,
      };

      // Handle rejection events (event_type = REJECT, event_code = -1)
      if (
        event.eventType === 'REJECT' ||
        rawEvent.event_code === -1
      ) {
        // Find optimistic update and rollback
        const doc = await collection.findOne(event.actorId).exec();
        if (doc && doc.get('_optimistic') && doc.get('_lastServerData')) {
          const lastData = doc.get('_lastServerData');
          const updates: any = { _optimistic: false, _lastServerData: null };

          if (lastData.ability_data) updates.ability_data = lastData.ability_data;
          if (lastData.resource_data)
            updates.resource_data = lastData.resource_data;
          if (lastData.proficiency_data)
            updates.proficiency_data = lastData.proficiency_data;
          if (lastData.trait_data) updates.trait_data = lastData.trait_data;
          if (lastData.spell_data) updates.spell_data = lastData.spell_data;

          await doc.update({ $set: updates });
        }
      } else {
        // Queue normal operation
        await queueEvent(event);
      }
    } catch (error) {
      onError(error instanceof Error ? error : new Error(String(error)));
    }
  };

  // Attempt to fetch missed events on startup
  fetchMissedEvents().catch((error) =>
    onError(error instanceof Error ? error : new Error(String(error)))
  );

  // Export for testing: this would normally be wired to GraphQL subscription
  // For now, we expose handleSubscriptionEvent so it can be called from
  // the GraphQL subscription component
  (globalThis as any).__rxdb_actor_data_subscription = handleSubscriptionEvent;

  /**
   * Cleanup function: stop replication
   */
  return () => {
    abortController.abort();
    if (debounceTimer) clearTimeout(debounceTimer);
    // Process any remaining batched events
    if (updateBatch.length > 0) {
      const batch = updateBatch;
      updateBatch = [];
      processBatch(batch).catch(onError);
    }
  };
}

/**
 * Hook for React components to subscribe to actor data changes
 *
 * Usage:
 * ```tsx
 * const actorData$ = useActorSystemDataSubscription(worldId, 'dnd5e', collection);
 * actorData$.subscribe(doc => {
 *   console.log('Actor updated:', doc.actor_id, doc.game_system_id);
 * });
 * ```
 */
export function useActorSystemDataSubscription(
  collection: RxCollection<WorldActorSystemDataDoc>,
  gameSystemId: string
) {
  return collection
    .find()
    .where('game_system_id')
    .eq(gameSystemId)
    .subscribe((docs) => docs);
}

/**
 * Prepare data for optimistic update
 *
 * Saves current state before mutation so we can rollback on rejection
 */
export async function prepareActorDataOptimisticUpdate(
  doc: any,
  changedFields: string[]
): Promise<void> {
  const lastServerData: any = {};

  for (const field of changedFields) {
    lastServerData[field] = doc.get(field);
  }

  await doc.update({
    $set: {
      _optimistic: true,
      _lastServerData: lastServerData,
    },
  });
}

/**
 * Apply optimistic update to actor data
 *
 * Called immediately after mutation is sent to server
 */
export async function applyActorDataOptimisticUpdate(
  doc: any,
  updates: Partial<WorldActorSystemDataDoc>
): Promise<void> {
  const changedFields = Object.keys(updates);
  await prepareActorDataOptimisticUpdate(doc, changedFields);

  await doc.update({
    $set: updates,
  });
}
