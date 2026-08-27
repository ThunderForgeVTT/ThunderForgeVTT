/**
 * Service worker: a tombstone that deletes the canvas-asset cache.
 *
 * This worker used to serve `/api/canvas-assets/*` cache-first, on the
 * argument that those URLs are content-addressed — a new upload mints a new
 * asset id, so a cached response can never be stale. That argument is sound
 * about *staleness* and silent about *revocation*, which is the reason this
 * file no longer caches anything (spec `028-client-world-cache`, T045b).
 *
 * # Why it stopped caching rather than learning to evict
 *
 * A Cache Storage entry holds **plaintext** image bytes keyed by request URL,
 * per browser profile, with no notion of who fetched them or which world they
 * belong to. It outlived the one event that has to erase them: a member being
 * removed from a world. Clearing on logout does not cover that — a revoked
 * member stays signed in.
 *
 * Spec 028 replaced this layer with a store that does know: canvas image
 * reads now go through `crates/thunderforge-cache-browser` (OPFS, encrypted
 * under a session-scoped key) via the engine's `cached_assets` plugin, which
 * evicts on the server's sync plan and discards a whole world when the server
 * refuses that plan (T045a). Teaching this worker to participate in that
 * eviction was the alternative; it was rejected because it would keep
 * unencrypted bytes at rest between eviction events and would couple a layer
 * with no session context to one built specifically to have it.
 *
 * # Why the file still exists, and is still registered
 *
 * Deleting it would leave every browser that already installed the old worker
 * serving its cached plaintext indefinitely: unregistering a worker does not
 * empty Cache Storage, and no page code can reach entries a departed worker
 * left behind. Shipping a *new* worker that purges on activate is what
 * actually reclaims those bytes. The app keeps registering it so that purge
 * reaches existing installs on their next load.
 *
 * Bytes that used to come from here now come from the encrypted cache for the
 * canvas (the large assets this ever mattered for), and from the HTTP cache
 * for DOM `<img>` consumers such as the token palette — bounded by the
 * `private, max-age=3600` the byte route sets.
 */

// Every cache this worker has ever created, past and present.
const CACHE_PREFIX = "thunderforge-canvas-assets-";

/** Deletes every canvas-asset cache, whatever version created it. */
async function purgeCanvasAssetCaches() {
  const names = await caches.keys();
  await Promise.all(
    names.filter((name) => name.startsWith(CACHE_PREFIX)).map((name) => caches.delete(name)),
  );
}

self.addEventListener("install", (event) => {
  // Take over without waiting for existing tabs to close: the point of this
  // version is to displace a worker that is still serving cached plaintext,
  // and waiting would leave it doing that until every tab closed.
  event.waitUntil(self.skipWaiting());
});

self.addEventListener("activate", (event) => {
  event.waitUntil(
    (async () => {
      await purgeCanvasAssetCaches();
      await self.clients.claim();
    })(),
  );
});

self.addEventListener("message", (event) => {
  // The app still asks for this on logout. Activation has normally purged
  // already; this covers a client that installed the old worker, cached
  // bytes, and signed out before this version activated.
  if (event.data?.type === "clear-canvas-asset-cache") {
    event.waitUntil(purgeCanvasAssetCaches());
  }
});

// Deliberately no `fetch` handler. With none registered the browser treats
// every request as if this worker were not there, which is exactly the
// behaviour wanted — and it is the difference between "caches nothing" and
// "caches nothing but still costs a worker round-trip on every request".
