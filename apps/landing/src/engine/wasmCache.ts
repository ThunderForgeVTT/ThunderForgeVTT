// The engine's wasm, kept between visits.
//
// It is hundreds of megabytes: past what a browser's HTTP cache keeps for
// one response, so a second visit downloaded it all again. Cache Storage
// would hold it, but only on a secure origin, and a phone on the dev
// server's LAN address is not one. IndexedDB is on every origin.
//
// One engine is kept, under its hashed file name: storing a new one drops
// the old. Every failure (no IndexedDB, a private window, a full disk) is
// a miss, and the engine is downloaded as before.

const DB = "thunderforge-engine";
const STORE = "wasm";

function open(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB, 1);
    req.onupgradeneeded = () => req.result.createObjectStore(STORE);
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

function done(tx: IDBTransaction): Promise<void> {
  return new Promise((resolve, reject) => {
    tx.oncomplete = () => resolve();
    tx.onerror = tx.onabort = () => reject(tx.error);
  });
}

/** The stored bytes for `name`, if they are all there. */
export async function cachedWasm(name: string, size: number): Promise<Uint8Array | null> {
  try {
    const db = await open();
    const blob = await new Promise<unknown>((resolve, reject) => {
      const req = db.transaction(STORE).objectStore(STORE).get(name);
      req.onsuccess = () => resolve(req.result);
      req.onerror = () => reject(req.error);
    });
    db.close();
    if (!(blob instanceof Blob) || blob.size !== size) return null;
    return new Uint8Array(await blob.arrayBuffer());
  } catch {
    return null;
  }
}

/** Keeps `bytes` as the one stored engine. Never throws. */
export async function keepWasm(name: string, bytes: Uint8Array): Promise<void> {
  try {
    const db = await open();
    const tx = db.transaction(STORE, "readwrite");
    const store = tx.objectStore(STORE);
    store.clear();
    store.put(new Blob([bytes as Uint8Array<ArrayBuffer>], { type: "application/wasm" }), name);
    await done(tx);
    db.close();
  } catch {
    // A miss next time.
  }
}
