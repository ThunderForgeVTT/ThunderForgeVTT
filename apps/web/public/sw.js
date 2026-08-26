/**
 * Service worker: cache-first delivery of canvas image assets.
 *
 * Scene backgrounds are the largest thing this app moves — a single imported
 * battle map is several megabytes — and today every scene switch, page reload
 * and reconnect re-downloads them. The engine fetches them itself from inside
 * wasm, so there is no JS call site to wrap; a service worker is the only
 * layer that sees those requests.
 *
 * # Why cache-first is safe here
 *
 * `/api/canvas-assets/{uuid}` is effectively content-addressed: the id is
 * minted per upload and its bytes never change. Re-importing a map produces a
 * *new* asset id and the scene points at that instead. So a cached response
 * can never be stale, and there is nothing to revalidate — which is what makes
 * plain cache-first correct rather than merely convenient.
 *
 * # What is deliberately not cached
 *
 * - **Anything non-200.** A 401 from an expired session or a 403 from a
 *   world-membership check must never be cached: doing so would pin a user out
 *   of an asset they later regain access to, and would survive a re-login.
 * - **Every other `/api/` route.** GraphQL, session, and the rest are dynamic
 *   by definition.
 * - **Range requests.** A partial response cached as if whole would corrupt
 *   every later read of that asset.
 *
 * Authorization still happens on the server for the first fetch of any asset.
 * A cached entry is per-origin and per-browser-profile, so it cannot leak
 * across users on different machines — but it *is* shared between accounts
 * using the same browser profile, which is why the cache is cleared on logout
 * (see `clearCanvasAssetCache` in the app).
 */

// Bumping this name is how a deploy discards previously cached bytes.
const CACHE_NAME = "thunderforge-canvas-assets-v1";
const ASSET_PREFIX = "/api/canvas-assets/";

self.addEventListener("install", (event) => {
  // Take over without waiting for existing tabs to close, so a fix to this
  // worker reaches users on their next load rather than their next session.
  event.waitUntil(self.skipWaiting());
});

self.addEventListener("activate", (event) => {
  event.waitUntil(
    (async () => {
      // Drop caches from previous versions of this worker.
      const names = await caches.keys();
      await Promise.all(
        names
          .filter((name) => name.startsWith("thunderforge-canvas-assets-") && name !== CACHE_NAME)
          .map((name) => caches.delete(name)),
      );
      await self.clients.claim();
    })(),
  );
});

self.addEventListener("message", (event) => {
  // The app asks for this on logout — see the note on profile sharing above.
  if (event.data?.type === "clear-canvas-asset-cache") {
    event.waitUntil(caches.delete(CACHE_NAME));
  }
});

self.addEventListener("fetch", (event) => {
  const request = event.request;
  const url = new URL(request.url);

  if (
    request.method !== "GET" ||
    url.origin !== self.location.origin ||
    !url.pathname.startsWith(ASSET_PREFIX) ||
    // A cached partial would corrupt every later read of the asset.
    request.headers.has("range")
  ) {
    return;
  }

  event.respondWith(
    (async () => {
      const cache = await caches.open(CACHE_NAME);
      const cached = await cache.match(request);
      if (cached) {
        return cached;
      }

      const response = await fetch(request);

      // Only a complete, successful, same-origin response is worth keeping.
      // `type === "basic"` excludes opaque cross-origin responses, whose
      // status is not readable and so cannot be checked.
      if (response.ok && response.type === "basic") {
        // Clone before returning: a Response body can only be read once.
        cache.put(request, response.clone()).catch(() => {
          // Storage pressure or a quota rejection. The response is still fine
          // to serve — caching is an optimisation, never a requirement.
        });
      }

      return response;
    })(),
  );
});
