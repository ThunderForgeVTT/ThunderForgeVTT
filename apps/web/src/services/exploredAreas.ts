/**
 * What a player's own token has explored, kept in their own browser.
 *
 * Spec 045 US7, owner decision 3: exploration is the player's, it lives on
 * their machine, and clearing their storage loses it — which is theirs to
 * lose. Nothing here is ever sent to the server.
 *
 * # Why a database of its own
 *
 * The data model says "the IndexedDB the world cache already uses", and this
 * deliberately does not. That database's schema is opened and versioned by
 * `crates/thunderforge-cache-browser` from Rust; adding an object store to it
 * means a version bump that both sides have to agree on, and a disagreement
 * does not fail loudly — it makes the world cache unopenable, which costs a
 * player far more than their fog. A sibling database is a few lines and
 * cannot break the cache.
 *
 * # Nothing here throws
 *
 * Storage can be unavailable — a private window, a browser set to block site
 * data, a quota that is full. A player whose fog cannot be saved should still
 * be able to play, so every entry point resolves: reading gives `null`,
 * writing gives `false`, and the caller carries on.
 */

const DB_NAME = "thunderforge-exploration";
const DB_VERSION = 1;
const STORE = "areas";

/** One scene's remembered cells, as stored. */
export interface StoredExploration {
  /** `[q, r]` pairs, exactly as the engine reports them. */
  cells: [number, number][];
  /**
   * The epoch this was accumulated under.
   *
   * Compared against what the server says on arrival: if the server's number
   * is greater, a Game Master reset the fog while this browser was away, and
   * what is stored here is stale (FR-078).
   */
  epoch: number;
  updatedAt: number;
}

/**
 * A player's fog for one scene is keyed by all three.
 *
 * The user because two people share a machine; the world and scene because a
 * player explores each separately. Without the user in the key, signing in as
 * somebody else would show you their map.
 */
function keyFor(userId: string, worldId: string, sceneId: string): string {
  return `${userId}:${worldId}:${sceneId}`;
}

function open(): Promise<IDBDatabase | null> {
  return new Promise((resolve) => {
    if (typeof indexedDB === "undefined") {
      resolve(null);
      return;
    }
    let request: IDBOpenDBRequest;
    try {
      request = indexedDB.open(DB_NAME, DB_VERSION);
    } catch {
      resolve(null);
      return;
    }
    request.onupgradeneeded = () => {
      const db = request.result;
      if (!db.objectStoreNames.contains(STORE)) {
        db.createObjectStore(STORE);
      }
    };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => resolve(null);
    // A blocked open means another tab holds an older version. Resolving null
    // rather than waiting: the fog is not worth hanging a scene load on.
    request.onblocked = () => resolve(null);
  });
}

/** What this player remembers of one scene, or `null`. */
export async function readExploration(
  userId: string,
  worldId: string,
  sceneId: string,
): Promise<StoredExploration | null> {
  const db = await open();
  if (!db) return null;
  return new Promise((resolve) => {
    try {
      const tx = db.transaction(STORE, "readonly");
      const request = tx
        .objectStore(STORE)
        .get(keyFor(userId, worldId, sceneId));
      request.onsuccess = () => {
        const value = request.result as StoredExploration | undefined;
        resolve(value && Array.isArray(value.cells) ? value : null);
      };
      request.onerror = () => resolve(null);
    } catch {
      resolve(null);
    } finally {
      db.close();
    }
  });
}

/**
 * Save what this player remembers.
 *
 * The epoch is stored alongside, because a memory without the number it was
 * accumulated under cannot be told from one a Game Master has since reset.
 */
export async function writeExploration(
  userId: string,
  worldId: string,
  sceneId: string,
  cells: [number, number][],
  epoch: number,
): Promise<boolean> {
  const db = await open();
  if (!db) return false;
  return new Promise((resolve) => {
    try {
      const tx = db.transaction(STORE, "readwrite");
      tx.objectStore(STORE).put(
        { cells, epoch, updatedAt: Date.now() } satisfies StoredExploration,
        keyFor(userId, worldId, sceneId),
      );
      tx.oncomplete = () => resolve(true);
      tx.onerror = () => resolve(false);
      // A quota that is full is the commonest failure here, and it is not an
      // error a player needs to see: their fog stops growing, and nothing
      // else about the session changes.
      tx.onabort = () => resolve(false);
    } catch {
      resolve(false);
    } finally {
      db.close();
    }
  });
}

/** Forget one scene's fog — a reset, or a player clearing it themselves. */
export async function forgetExploration(
  userId: string,
  worldId: string,
  sceneId: string,
): Promise<boolean> {
  const db = await open();
  if (!db) return false;
  return new Promise((resolve) => {
    try {
      const tx = db.transaction(STORE, "readwrite");
      tx.objectStore(STORE).delete(keyFor(userId, worldId, sceneId));
      tx.oncomplete = () => resolve(true);
      tx.onerror = () => resolve(false);
    } catch {
      resolve(false);
    } finally {
      db.close();
    }
  });
}

/** What one player's fog is costing, for the storage panel (FR-076). */
export interface ExplorationUsage {
  scenes: number;
  cells: number;
  /** A rough byte figure: two numbers a cell, stored as JSON. */
  bytes: number;
}

/**
 * Everything this player has explored, across every world.
 *
 * Reported so a player can see what they hold and clear it — the same screen
 * that already accounts for the world cache. Approximate by design: the exact
 * on-disk size of an IndexedDB record is not observable, and a figure a
 * player can act on is worth more than one nobody can produce.
 */
export async function readExplorationUsage(
  userId: string,
): Promise<ExplorationUsage> {
  const empty: ExplorationUsage = { scenes: 0, cells: 0, bytes: 0 };
  const db = await open();
  if (!db) return empty;
  return new Promise((resolve) => {
    try {
      const tx = db.transaction(STORE, "readonly");
      const store = tx.objectStore(STORE);
      const keys = store.getAllKeys();
      const values = store.getAll();
      tx.oncomplete = () => {
        const mine = (keys.result as string[])
          .map((key, index) => ({
            key,
            value: (values.result as StoredExploration[])[index],
          }))
          .filter(({ key }) => key.startsWith(`${userId}:`));
        const cells = mine.reduce(
          (total, { value }) => total + (value?.cells?.length ?? 0),
          0,
        );
        resolve({
          scenes: mine.length,
          cells,
          // Two small integers and their punctuation, in JSON.
          bytes: cells * 12,
        });
      };
      tx.onerror = () => resolve(empty);
    } catch {
      resolve(empty);
    } finally {
      db.close();
    }
  });
}

/** Forget every scene's fog for one player. */
export async function forgetAllExploration(userId: string): Promise<number> {
  const db = await open();
  if (!db) return 0;
  return new Promise((resolve) => {
    try {
      const tx = db.transaction(STORE, "readwrite");
      const store = tx.objectStore(STORE);
      const keys = store.getAllKeys();
      let removed = 0;
      keys.onsuccess = () => {
        for (const key of keys.result as string[]) {
          // Only this user's. A shared machine must not have one person's
          // "clear my fog" take everybody's.
          if (key.startsWith(`${userId}:`)) {
            store.delete(key);
            removed += 1;
          }
        }
      };
      tx.oncomplete = () => resolve(removed);
      tx.onerror = () => resolve(0);
    } catch {
      resolve(0);
    } finally {
      db.close();
    }
  });
}
