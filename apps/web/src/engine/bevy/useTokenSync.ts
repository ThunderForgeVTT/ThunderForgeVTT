import { useEffect, useRef } from 'react';
import { useRxDB } from '@/hooks/useRxDB';
import type { Engine } from '@/lib/engine';

interface TokenData {
  id: string;
  world_id: string;
  scene_id: string;
  token_type: string;
  label?: string;
  base_x: number;
  base_y: number;
  size_x: number;
  size_y: number;
  color: {
    r: number;
    g: number;
    b: number;
  };
  is_visible: boolean;
}

/**
 * React hook to sync RxDB world_tokens collection with Bevy engine
 * Watches for RxDB changes and calls engine.update_world_tokens()
 *
 * Architecture:
 * 1. Subscribe to worldTokensCollection in RxDB
 * 2. On each change (insert/update/delete), collect all tokens
 * 3. Convert to TokenData (matches Rust struct)
 * 4. Call WASM export: engine.update_world_tokens(tokens)
 * 5. Bevy system (sync_tokens_from_rxdb) spawns/despawns entities
 *
 * Circle: RxDB → React → WASM → Bevy → render
 */
export function useTokenSync(engine: Engine | null, worldId: string | null) {
  const { db } = useRxDB();
  const subscriptionRef = useRef<{ unsubscribe: () => void } | null>(null);

  useEffect(() => {
    if (!engine || !worldId || !db) return;

    let isActive = true;
    let tokenSubscription: any = null;

    const setupSubscription = async () => {
      try {
        // Watch the worldTokensCollection for any changes
        const subscription = db.collections.worldTokensCollection
          ?.find()
          .where('world_id')
          .equals(worldId)
          .$
          .subscribe({
            next(tokens: any[]) {
              if (!isActive) return;

              // 🎮 Convert RxDB documents to TokenData structs
              const tokenData: TokenData[] = tokens.map((doc: any) => ({
                id: doc.id || doc._id,
                world_id: doc.world_id,
                scene_id: doc.scene_id,
                token_type: doc.token_type || 'character',
                label: doc.label,
                base_x: doc.base_x || doc.x || 0,
                base_y: doc.base_y || doc.y || 0,
                size_x: doc.size_x || 1,
                size_y: doc.size_y || 1,
                color: doc.color || { r: 0.5, g: 0.5, b: 0.5 },
                is_visible: doc.is_visible !== false,
              }));

              // 📤 Send to Bevy (as JSON string)
              try {
                engine.update_world_tokens(JSON.stringify(tokenData));
              } catch (error) {
                console.error('🔴 Failed to sync tokens to Bevy:', error);
              }
            },
            error(error: Error) {
              console.error('❌ RxDB subscription error:', error);
            },
          });

        tokenSubscription = subscription;
      } catch (error) {
        console.error('Failed to setup RxDB subscription:', error);
      }
    };

    setupSubscription();

    return () => {
      isActive = false;
      if (tokenSubscription?.unsubscribe) {
        tokenSubscription.unsubscribe();
      }
    };
  }, [engine, worldId, db]);
}

/**
 * Alternative hook if using GraphQL subscriptions
 * Syncs tokens from GraphQL worldEventCreated subscription
 */
export function useTokenSyncGraphQL(engine: Engine | null, worldId: string | null) {
  // TODO: Implement GraphQL-based sync
  // This would listen to worldEventCreated subscription
  // and update tokens from event payloads
}
