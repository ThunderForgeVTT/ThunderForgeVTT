import { createRxDatabase, addRxPlugin } from "rxdb/plugins/core";
import { getRxStorageLocalstorage } from "rxdb/plugins/storage-localstorage";
import { wrappedValidateAjvStorage } from "rxdb/plugins/validate-ajv";
import { RxDBDevModePlugin } from "rxdb/plugins/dev-mode";
import { worldSnapshotsSchema, worldTokensSchema } from "./schemas";

let devPluginRegistered = false;
let worldDbPromise: Promise<WorldDatabase> | null = null;

export type WorldCollections = {
  world_tokens: {
    find: (query?: unknown) => {
      exec: () => Promise<Array<Record<string, unknown>>>;
    };
    upsert: (doc: Record<string, unknown>) => Promise<unknown>;
    findOne: (id: string) => {
      exec: () => Promise<{ remove: () => Promise<unknown> } | null>;
    };
  };
  world_snapshots: {
    upsert: (doc: Record<string, unknown>) => Promise<unknown>;
  };
};

export type WorldDatabase = {
  collections: WorldCollections;
};

function shouldValidateInProd(): boolean {
  return import.meta.env.VITE_RXDB_VALIDATE_PROD === "true";
}

function shouldForceLocalStorage(): boolean {
  return import.meta.env.VITE_RXDB_STORAGE === "localstorage";
}

async function createBaseStorage() {
  if (!shouldForceLocalStorage()) {
    try {
      const dexieModule = await import("rxdb/plugins/storage-dexie");
      if (typeof dexieModule.getRxStorageDexie === "function") {
        return dexieModule.getRxStorageDexie();
      }
    } catch {
      // Fall back to localStorage when IndexedDB-backed storage is unavailable.
    }
  }

  return getRxStorageLocalstorage();
}

async function createStorage() {
  const baseStorage = await createBaseStorage();

  if (import.meta.env.DEV || shouldValidateInProd()) {
    return wrappedValidateAjvStorage({ storage: baseStorage });
  }

  return baseStorage;
}

export async function getWorldDatabase(): Promise<WorldDatabase> {
  if (!worldDbPromise) {
    if (import.meta.env.DEV && !devPluginRegistered) {
      addRxPlugin(RxDBDevModePlugin);
      devPluginRegistered = true;
    }

    worldDbPromise = (async () => {
      const db = await createRxDatabase({
        name: "thunderforge-world-sync",
        storage: await createStorage(),
      });

      await db.addCollections({
        world_tokens: { schema: worldTokensSchema },
        world_snapshots: { schema: worldSnapshotsSchema },
      });

      return db as unknown as WorldDatabase;
    })();
  }

  return worldDbPromise;
}
