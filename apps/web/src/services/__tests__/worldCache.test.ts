import { afterEach, describe, expect, it, vi } from "vitest";

import { discardWorldCache } from "../worldCache";

/**
 * Sign-out for the world cache (spec 028, FR-016a).
 *
 * The point of these is that the discard has actually *run*. The whole
 * confidentiality story of the local cache rests on one IndexedDB delete
 * happening at sign-out, and a wiring that merely compiles buys nothing — a
 * mistyped store name or a transaction that never commits looks identical to a
 * working one until somebody reads a signed-out user's cache.
 *
 * The engine's own `forget_world_cache` (the in-memory key drop and the
 * background reclamation) is not exercisable from node — it lives in wasm —
 * and its absence here is deliberately treated as the ordinary case, because
 * on most sign-outs the engine was never loaded.
 */

/** Just enough IndexedDB to run the real code against. */
function fakeIndexedDb() {
  const stores = new Map<string, Map<string, unknown>>();
  const opened: Array<{ name: string; version: number }> = [];
  let closed = 0;

  const db = {
    objectStoreNames: {
      contains: (name: string) => stores.has(name),
    },
    createObjectStore(name: string) {
      stores.set(name, new Map());
    },
    transaction(name: string) {
      const store = stores.get(name);
      if (!store) {
        throw new DOMException("no such object store", "NotFoundError");
      }
      const tx: Record<string, unknown> = {
        error: null,
        objectStore: () => ({
          clear: () => store.clear(),
        }),
      };
      queueMicrotask(() => {
        (tx.oncomplete as (() => void) | undefined)?.();
      });
      return tx;
    },
    close() {
      closed += 1;
    },
  };

  const indexedDB = {
    open(name: string, version: number) {
      opened.push({ name, version });
      const request: Record<string, unknown> = { result: db, error: null };
      queueMicrotask(() => {
        if (!existed) {
          (request.onupgradeneeded as (() => void) | undefined)?.();
        }
        (request.onsuccess as (() => void) | undefined)?.();
      });
      return request;
    },
  };

  let existed = false;

  return {
    indexedDB,
    stores,
    opened,
    closes: () => closed,
    /** Pretend the database already exists, with the given stores. */
    seed(names: string[]) {
      existed = true;
      for (const name of names) {
        stores.set(name, new Map());
      }
    },
  };
}

function install(fake: ReturnType<typeof fakeIndexedDb>) {
  vi.stubGlobal("indexedDB", fake.indexedDB);
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("discardWorldCache", () => {
  it("clears every stored session key", async () => {
    const fake = fakeIndexedDb();
    fake.seed(["index", "keys", "outbox", "meta"]);
    fake.stores.get("keys")!.set("a-user-scope", { crypto: "key" });
    fake.stores.get("index")!.set("asset:1", { size: 10 });
    install(fake);

    await discardWorldCache("6f1b2a3c-0000-4000-8000-000000000000");

    // The key is gone, which is what makes the blobs on disk inert.
    expect(fake.stores.get("keys")!.size).toBe(0);
    // The index is not this operation's business: reclaiming it is the
    // engine's background job, and losing it early would only cost a cold
    // cache anyway.
    expect(fake.stores.get("index")!.size).toBe(1);
    expect(fake.closes()).toBe(1);
  });

  /**
   * Opening at the schema version the Rust side uses, and creating the same
   * stores if this is the first open, is what stops a sign-out on a machine
   * that never cached anything from leaving a version-1 database with no
   * object stores — which the cache would then never be able to create.
   */
  it("never creates the database, because a version-less create would break the Rust side", async () => {
    // Sign-out must not bring a cache database into existence. Creating one
    // here — at any version, with or without stores — is how the Rust side's
    // own `open` ends up seeing its version as already current, never firing
    // `upgradeneeded`, and permanently finding none of its object stores.
    //
    // It also must not pin a version. Pinning means the day someone bumps
    // DB_VERSION in Rust, this file can no longer open the database at all
    // and sign-out silently stops discarding keys — a security-relevant
    // failure caused by a schema change nobody would connect to it.
    const fake = fakeIndexedDb();
    install(fake);

    await discardWorldCache("6f1b2a3c-0000-4000-8000-000000000000");

    expect(fake.stores.size, "no database should have been created").toBe(0);
    for (const open of fake.opened) {
      expect(
        open.version,
        "opens must be version-less so a Rust-side DB_VERSION bump cannot lock us out",
      ).toBeUndefined();
    }
  });

  it("clears the keys of an existing database at whatever version it is on", async () => {
    // The forward-compat case the version pin would have broken: a database
    // already migrated past version 1.
    const fake = fakeIndexedDb();
    fake.seed(["index", "keys", "outbox", "meta"]);
    fake.stores.get("keys")!.set("some-scope", { crypto: "key" });
    install(fake);

    await discardWorldCache("6f1b2a3c-0000-4000-8000-000000000000");

    expect(fake.stores.get("keys")!.size).toBe(0);
  });

  it("signs the user out even with no user id to scope anything to", async () => {
    const fake = fakeIndexedDb();
    fake.seed(["index", "keys", "outbox", "meta"]);
    fake.stores.get("keys")!.set("a-user-scope", { crypto: "key" });
    install(fake);

    await expect(discardWorldCache(null)).resolves.toBeUndefined();

    expect(fake.stores.get("keys")!.size).toBe(0);
  });

  /** A cache that could fail a sign-out would be worse than no cache. */
  it("resolves when IndexedDB is unavailable", async () => {
    vi.stubGlobal("indexedDB", undefined);
    await expect(
      discardWorldCache("6f1b2a3c-0000-4000-8000-000000000000"),
    ).resolves.toBeUndefined();
  });

  it("resolves when the database refuses to open", async () => {
    vi.stubGlobal("indexedDB", {
      open() {
        const request: Record<string, unknown> = {
          result: null,
          error: new Error("nope"),
        };
        queueMicrotask(() => {
          (request.onerror as (() => void) | undefined)?.();
        });
        return request;
      },
    });

    await expect(
      discardWorldCache("6f1b2a3c-0000-4000-8000-000000000000"),
    ).resolves.toBeUndefined();
  });
});
