/**
 * useWorldDatabase.ts
 * React hook for accessing the RxDB world database
 *
 * Handles the async initialization of the database and provides
 * it to child components via hook.
 *
 * Usage:
 *   const db = useWorldDatabase();
 *   if (db) {
 *     const tokens = await db.collections.world_tokens.find().exec();
 *   }
 */

import { useEffect, useState } from 'react';
import { getWorldDatabase } from '@/engine/world/sync/database';
import type { WorldDatabase } from '@/engine/world/sync/database';

let cachedDb: WorldDatabase | null = null;

export function useWorldDatabase(): WorldDatabase | null {
  const [db, setDb] = useState<WorldDatabase | null>(cachedDb);

  useEffect(() => {
    let mounted = true;

    if (cachedDb) {
      setDb(cachedDb);
      return;
    }

    (async () => {
      try {
        const database = await getWorldDatabase();
        cachedDb = database;
        if (mounted) {
          setDb(database);
        }
      } catch (error) {
        console.error('Failed to initialize world database:', error);
        if (mounted) {
          setDb(null);
        }
      }
    })();

    return () => {
      mounted = false;
    };
  }, []);

  return db;
}
