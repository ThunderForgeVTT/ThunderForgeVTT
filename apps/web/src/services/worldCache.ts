/**
 * Sign-out for the client world cache (spec 028, FR-016a/FR-016b, T037/T038).
 *
 * The cache holds world content on disk, encrypted under a key bound to the
 * signed-in session. Sign-out has to make that content inert *immediately*,
 * and immediacy is why the key is what gets destroyed rather than the bytes:
 * a multi-gigabyte store cannot be wiped before the tab closes, and an
 * interrupted wipe leaves readable bytes behind. Destroying the key is one
 * IndexedDB delete and finishes in milliseconds; everything that remains on
 * disk afterwards is ciphertext nobody can open.
 *
 * Reclaiming the disk space is a separate, slower, entirely optional job. It
 * only ever deletes. If it fails, is interrupted, or never runs, the content
 * stays unreadable — reclamation frees space, it is never what makes the data
 * safe.
 *
 * # Two paths, because the engine may not be loaded
 *
 * The engine owns the cache, so the engine's `forget_world_cache` is the
 * complete operation: it drops the live `CryptoKey` this tab is holding in
 * memory, deletes the stored key, switches the asset read path off, and starts
 * the background reclamation. But signing out from the app header on a page
 * that never mounted the engine is the *common* case, and the key survives
 * page loads — so waiting for a multi-megabyte wasm module to download before
 * a sign-out could discard a key would be both slow and, if the user closed
 * the tab first, simply wrong.
 *
 * So the key is discarded here first, directly, with no wasm involved, and the
 * engine entry point is then invoked as a best-effort extra for the case where
 * it is already loaded. The engine call is idempotent — by the time it runs the
 * key record is usually already gone — and its real value is the in-memory key
 * and the reclamation, neither of which this file can reach.
 *
 * Nothing here ever throws. Sign-out is not the cache's business to interfere
 * with.
 */

/**
 * The database name, and the one store this file touches. Mirrors
 * `DB_NAME`/`STORE_KEYS` in `crates/thunderforge-cache-browser/src/lib.rs`.
 *
 * Deliberately NOT mirroring `DB_VERSION` or the full store list. Pinning a
 * version here would mean that the day someone bumps `DB_VERSION` in Rust,
 * this file could no longer open the database at all — IndexedDB refuses to
 * open at a version below the stored one — and sign-out would silently stop
 * discarding keys. A security-relevant failure caused by a schema change
 * nobody would connect to it.
 *
 * Instead we never create the database: if it does not exist there is no key
 * to discard, so there is nothing to do. That also avoids the trap the
 * version-pinning was written to dodge — opening version-less on a
 * non-existent database creates an empty one at version 1 with no object
 * stores, after which the Rust side's own `open` at version 1 sees its
 * version as current, never fires `upgradeneeded`, and permanently finds no
 * stores.
 */
const DB_NAME = "thunderforge-cache";
const STORE_KEYS = "keys";

/**
 * The cross-tab sign-out signal (FR-021b).
 *
 * Deleting the stored key above is enough for a tab that has yet to read it.
 * It is not enough for a tab with the engine mounted: that tab is holding a
 * live `CryptoKey` in wasm memory and will keep decrypting cached blobs until
 * something makes it stop. Nothing in IndexedDB can make it stop, because it
 * never looks there again. **A key discarded from storage while another tab
 * holds it is not discarded**, so the discard is announced as well as done.
 *
 * Two carriers, mirroring `crates/thunderforge-cache-browser/src/signal.rs`,
 * which is what receives them:
 *
 * - `BroadcastChannel` is the right primitive and reaches workers, but is not
 *   everywhere.
 * - A `localStorage` write fires a `storage` event in every *other* window of
 *   the profile, which browsers have done for well over a decade. It does not
 *   reach workers and does not fire in this window, so it supplements rather
 *   than replaces the channel — but it means the degraded path for FR-021b is
 *   a second mechanism rather than none (FR-021d).
 *
 * Both carry the same payload, and the receiver is idempotent, so a tab
 * getting both is not a problem worth suppressing.
 */
const SIGNAL_CHANNEL = "thunderforge-cache";
const SIGNAL_STORAGE_KEY = "thunderforge-cache:signal";
const SIGNAL_SIGNED_OUT = "signed-out";

/**
 * Tell every other tab of this profile that the session ended.
 *
 * Best effort and synchronous: sign-out must not wait on this, and must not
 * be failed by it. A browser with neither carrier leaves other tabs on
 * today's behaviour, which is what FR-021d asks for.
 *
 * The nonce exists solely so consecutive `localStorage` writes differ — a
 * `storage` event fires on change, so a second sign-out writing the identical
 * value would notify nobody.
 */
function broadcastSignOut(): void {
  const payload = JSON.stringify({
    kind: SIGNAL_SIGNED_OUT,
    nonce: String(Date.now()),
  });

  try {
    if (typeof BroadcastChannel === "function") {
      const channel = new BroadcastChannel(SIGNAL_CHANNEL);
      channel.postMessage(payload);
      channel.close();
    }
  } catch {
    // No channel, or posting failed. The storage carrier below is still
    // worth trying, and neither is worth failing a sign-out over.
  }

  try {
    localStorage.setItem(SIGNAL_STORAGE_KEY, payload);
  } catch {
    // Absent, or site data is blocked. Nothing to do.
  }
}

/** The engine's wasm module, as far as this file needs to know it. */
type EngineCacheModule = {
  forget_world_cache?: (userId: string) => Promise<string>;
};

/** Whether the cache database exists at all. */
async function cacheDbExists(): Promise<boolean> {
  // `databases()` is not universally available (Firefox only gained it in
  // 126). Where it is missing we fall through and attempt the open, which is
  // the pre-existing behaviour and no worse than not trying.
  if (typeof indexedDB.databases !== "function") {
    return true;
  }
  try {
    const dbs = await indexedDB.databases();
    return dbs.some((d) => d.name === DB_NAME);
  } catch {
    return true;
  }
}

/**
 * Open the existing database at whatever version it is already at.
 *
 * Version-less on purpose — see the constants above. `onupgradeneeded` firing
 * here would mean the database did not exist, which we checked for; if it
 * somehow does fire, we abort rather than create a store-less database that
 * would break the Rust side's own open forever.
 */
function openDb(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(DB_NAME);
    request.onupgradeneeded = () => {
      request.transaction?.abort();
      reject(new Error("cache database did not exist"));
    };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
    // Another tab is mid-upgrade. Not our problem to resolve, and not worth
    // hanging sign-out over.
    request.onblocked = () => reject(new Error("blocked"));
  });
}

/**
 * Delete every stored session key, making everything this browser profile has
 * cached unreadable (FR-016a).
 *
 * Every key, not just the departing user's, and that is intentional twice
 * over. It avoids re-deriving the user's opaque scope here — the derivation
 * lives in Rust, and a copy of it that silently drifted would leave the key it
 * was supposed to destroy sitting on disk. And the threat this encryption
 * exists to counter is precisely a second person on a shared computer, so
 * clearing the lot on sign-out is the behaviour that matches the reason for
 * the feature. The cost to any other account cached here is a cold cache on
 * their next visit, which the spec already requires be indistinguishable from
 * a normal one (FR-016c).
 */
async function discardSessionKeys(): Promise<boolean> {
  if (typeof indexedDB === "undefined") {
    return false;
  }
  // Nothing cached means no key to discard. Returning true here is the honest
  // answer: there is no readable content left, which is what the caller asked
  // about.
  if (!(await cacheDbExists())) {
    return true;
  }
  let db: IDBDatabase | null = null;
  try {
    db = await openDb();
    const database = db;
    await new Promise<void>((resolve, reject) => {
      // A database that predates the keys store has no key to discard.
      if (!database.objectStoreNames.contains(STORE_KEYS)) {
        resolve();
        return;
      }
      const tx = database.transaction(STORE_KEYS, "readwrite");
      tx.objectStore(STORE_KEYS).clear();
      tx.oncomplete = () => resolve();
      tx.onerror = () => reject(tx.error);
      tx.onabort = () => reject(tx.error);
    });
    return true;
  } catch {
    return false;
  } finally {
    db?.close();
  }
}

/**
 * Ask the engine to finish the job, if and only if it is already loaded.
 *
 * The dynamic import resolves from the module registry when the engine has
 * been mounted this session, and otherwise pulls in only wasm-bindgen's glue —
 * never the wasm binary itself, which is fetched by the engine's own
 * initialisation. Calling into an uninitialised module throws, and that throw
 * is the signal that there was no in-memory key to drop and no engine-side
 * reclamation to start. Either way the stored key is already gone.
 */
async function askEngineToForget(userId: string): Promise<void> {
  const module =
    (await import("@thunderforge/engine/engine")) as EngineCacheModule;
  await module.forget_world_cache?.(userId);
}

/**
 * Discard the local world cache's session key on sign-out.
 *
 * Resolves once the stored key is gone — the point from which the cached bytes
 * are inert — and never rejects. Reclaiming the space those bytes occupy is
 * left running in the background and is not awaited.
 */
export async function discardWorldCache(
  userId: string | null | undefined,
): Promise<void> {
  await discardSessionKeys();

  // After the stored key is gone, so a tab that reacts by re-reading the
  // store finds nothing rather than the record we are about to delete.
  // Before the engine call, because the engine call is best-effort and may
  // not resolve at all, whereas the other tabs' in-memory keys are the point
  // of FR-021b.
  broadcastSignOut();

  if (!userId) {
    // No id, no scope, so there is nothing the engine could reclaim. The key
    // is already gone, which is the part that mattered.
    return;
  }
  // Deliberately not awaited: the in-memory drop is instant and the
  // reclamation behind it is slow by nature.
  void askEngineToForget(userId).catch(() => {
    // The engine was never loaded, or has no cache entry point. Both are the
    // ordinary case on most sign-outs and neither leaves a readable cache.
  });
}
