import { afterEach, describe, expect, it, vi } from "vitest";

import { discardWorldCache, onCrossTabSignOut } from "../worldCache";

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

/**
 * Capture what a sign-out puts on each cross-tab carrier (FR-021b).
 */
function fakeCarriers() {
  const posted: string[] = [];
  const closed: number[] = [];
  const stored = new Map<string, string>();
  const channelNames: string[] = [];

  class FakeBroadcastChannel {
    constructor(readonly name: string) {
      channelNames.push(name);
    }
    postMessage(data: string) {
      posted.push(data);
    }
    close() {
      closed.push(posted.length);
    }
  }

  return {
    posted,
    closed,
    stored,
    channelNames,
    install(options: { channel?: boolean; storage?: boolean } = {}) {
      const { channel = true, storage = true } = options;
      if (channel) {
        vi.stubGlobal("BroadcastChannel", FakeBroadcastChannel);
      } else {
        vi.stubGlobal("BroadcastChannel", undefined);
      }
      if (storage) {
        vi.stubGlobal("localStorage", {
          setItem: (key: string, value: string) => {
            stored.set(key, value);
          },
        });
      } else {
        vi.stubGlobal("localStorage", {
          setItem: () => {
            throw new DOMException("site data blocked", "SecurityError");
          },
        });
      }
    },
  };
}

/**
 * Sign-out has to reach tabs that are not the one signing out (FR-021b).
 *
 * The tab holding the engine has a live `CryptoKey` in wasm memory. Deleting
 * the IndexedDB record does nothing to it — it never reads that record again —
 * so unless the discard is *announced*, that tab goes on serving cached
 * content from a key that no longer exists anywhere else. These assert the
 * announcement is made, and that the payload is the one the Rust listener in
 * `crates/thunderforge-cache-browser/src/signal.rs` actually parses. That
 * shape is asserted on both sides on purpose: it is a string contract across a
 * language boundary, and drift in it fails silently.
 */
describe("cross-tab sign-out", () => {
  const userId = "6f1b2a3c-0000-4000-8000-000000000000";

  it("announces the sign-out on both carriers", async () => {
    const carriers = fakeCarriers();
    carriers.install();
    const fake = fakeIndexedDb();
    fake.seed(["index", "keys", "outbox", "meta"]);
    install(fake);

    await discardWorldCache(userId);

    expect(carriers.posted).toHaveLength(1);
    expect(JSON.parse(carriers.posted[0]!)).toMatchObject({
      kind: "signed-out",
    });
    expect(carriers.stored.get("thunderforge-cache:signal")).toBe(
      carriers.posted[0],
    );
    // The channel name is half of a cross-language contract; the Rust
    // listener subscribes to this exact string.
    expect(carriers.channelNames).toEqual(["thunderforge-cache"]);
    // A channel left open in a page that is navigating away is a leak with
    // no upside; the message is already queued for delivery.
    expect(carriers.closed).toEqual([1]);
  });

  it("carries a nonce, so a second sign-out is seen as a change", async () => {
    // `storage` events fire on change. Two identical writes would notify
    // nobody, which would make the fallback carrier work exactly once per
    // profile — the sort of bug that only shows up on the second sign-out.
    const carriers = fakeCarriers();
    carriers.install();
    const fake = fakeIndexedDb();
    fake.seed(["index", "keys", "outbox", "meta"]);
    install(fake);

    const now = vi.spyOn(Date, "now");
    try {
      now.mockReturnValue(1_000);
      await discardWorldCache(userId);
      const first = carriers.stored.get("thunderforge-cache:signal");
      now.mockReturnValue(2_000);
      await discardWorldCache(userId);
      const second = carriers.stored.get("thunderforge-cache:signal");

      expect(first).toBeDefined();
      expect(second).not.toBe(first);
      expect(JSON.parse(second!)).toMatchObject({ kind: "signed-out" });
    } finally {
      now.mockRestore();
    }
  });

  it("still uses storage where BroadcastChannel does not exist", async () => {
    // FR-021d: degrade to the other carrier rather than to nothing.
    const carriers = fakeCarriers();
    carriers.install({ channel: false });
    const fake = fakeIndexedDb();
    fake.seed(["index", "keys", "outbox", "meta"]);
    install(fake);

    await expect(discardWorldCache(userId)).resolves.toBeUndefined();

    expect(carriers.posted).toHaveLength(0);
    expect(carriers.stored.get("thunderforge-cache:signal")).toBeDefined();
  });

  it("still uses the channel where storage throws", async () => {
    const carriers = fakeCarriers();
    carriers.install({ storage: false });
    const fake = fakeIndexedDb();
    fake.seed(["index", "keys", "outbox", "meta"]);
    install(fake);

    await expect(discardWorldCache(userId)).resolves.toBeUndefined();

    expect(carriers.posted).toHaveLength(1);
  });

  it("announces even when the key could not be discarded", async () => {
    // The other tab dropping its in-memory key is the *only* protection left
    // when the stored delete failed, so this is exactly the case that must
    // not be skipped. Sign-out is never blocked by the cache, and the
    // announcement is never blocked by the discard.
    const carriers = fakeCarriers();
    carriers.install();
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

    await expect(discardWorldCache(userId)).resolves.toBeUndefined();

    expect(carriers.posted).toHaveLength(1);
  });

  it("announces even with no user id", async () => {
    const carriers = fakeCarriers();
    carriers.install();
    const fake = fakeIndexedDb();
    fake.seed(["index", "keys", "outbox", "meta"]);
    install(fake);

    await expect(discardWorldCache(null)).resolves.toBeUndefined();

    expect(carriers.posted).toHaveLength(1);
  });
});

/**
 * A minimal `window` for the storage carrier.
 *
 * This suite runs in vitest's node environment (see vitest.config.ts), and
 * jsdom is not a dependency. `EventTarget` is built in and gives exactly the
 * three methods the listener uses, which is cheaper and more honest than
 * pulling in a DOM implementation to test four lines of event plumbing.
 */
function installWindowStub(): () => void {
  const target = new EventTarget();
  const previous = (globalThis as { window?: unknown }).window;
  (globalThis as { window?: unknown }).window = target;
  return () => {
    (globalThis as { window?: unknown }).window = previous;
  };
}

/** A `storage`-shaped event, without needing StorageEvent to exist. */
function storageEvent(key: string, newValue: string | null): Event {
  const event = new Event("storage") as Event & {
    key?: string;
    newValue?: string | null;
  };
  event.key = key;
  event.newValue = newValue;
  return event;
}

describe("onCrossTabSignOut", () => {
  it("fires on a BroadcastChannel announcement from another tab", async () => {
    const listeners: ((e: MessageEvent) => void)[] = [];
    const original = globalThis.BroadcastChannel;
    // @ts-expect-error test double
    globalThis.BroadcastChannel = class {
      set onmessage(fn: (e: MessageEvent) => void) {
        listeners.push(fn);
      }
      close() {}
    };

    let fired = 0;
    const stop = onCrossTabSignOut(() => {
      fired += 1;
    });

    listeners[0]?.({
      data: JSON.stringify({ kind: "signed-out", nonce: "1" }),
    } as MessageEvent);
    expect(fired).toBe(1);

    stop();
    globalThis.BroadcastChannel = original;
  });

  it("fires on the storage carrier, which is why the carrier exists", async () => {
    // BroadcastChannel is not universal. A tab left presenting a signed-in
    // application after the user signed out elsewhere is the outcome this
    // second carrier exists to prevent, so it must work with no channel at
    // all.
    const original = globalThis.BroadcastChannel;
    // @ts-expect-error deliberately removing it
    delete globalThis.BroadcastChannel;
    const restoreWindow = installWindowStub();

    let fired = 0;
    const stop = onCrossTabSignOut(() => {
      fired += 1;
    });

    (globalThis.window as EventTarget).dispatchEvent(
      storageEvent(
        "thunderforge-cache:signal",
        JSON.stringify({ kind: "signed-out", nonce: "2" }),
      ),
    );
    expect(fired).toBe(1);

    stop();
    restoreWindow();
    globalThis.BroadcastChannel = original;
  });

  it("ignores unrelated storage traffic rather than signing people out", async () => {
    // Another writer on the same key, or noise on the channel, must not end
    // someone's session. A false positive here logs a user out mid-session
    // for no reason.
    const restoreWindow = installWindowStub();
    const stop = onCrossTabSignOut(() => {
      throw new Error("should not have fired");
    });
    const w = globalThis.window as EventTarget;

    w.dispatchEvent(storageEvent("something-else", "x"));
    w.dispatchEvent(storageEvent("thunderforge-cache:signal", "not json"));
    w.dispatchEvent(
      storageEvent(
        "thunderforge-cache:signal",
        JSON.stringify({ kind: "something-else" }),
      ),
    );

    stop();
    restoreWindow();
  });

  it("stops listening once unsubscribed", async () => {
    const restoreWindow = installWindowStub();
    let fired = 0;
    const stop = onCrossTabSignOut(() => {
      fired += 1;
    });
    stop();

    (globalThis.window as EventTarget).dispatchEvent(
      storageEvent(
        "thunderforge-cache:signal",
        JSON.stringify({ kind: "signed-out", nonce: "3" }),
      ),
    );
    expect(fired).toBe(0);
    restoreWindow();
  });
});
