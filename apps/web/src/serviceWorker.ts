/**
 * Registers the canvas-asset service worker (`public/sw.js`).
 *
 * That worker no longer caches anything. It exists to delete the cache-first
 * store an earlier version of it left behind, which held plaintext canvas art
 * that survived a member being revoked from a world (spec
 * `028-client-world-cache`, T045b). Registration is what carries that purge to
 * browsers that already installed the old worker.
 *
 * Kept deliberately small and failure-tolerant: nothing the app renders
 * depends on the worker, so every failure path here is a silent no-op rather
 * than something that could stop the app loading.
 */

const SERVICE_WORKER_URL = "/sw.js";

/** True where service workers are available and permitted. */
function isSupported(): boolean {
  // Unavailable in insecure contexts (except localhost) and in some private
  // browsing modes, where accessing it can throw rather than be undefined.
  try {
    return "serviceWorker" in navigator && window.isSecureContext;
  } catch {
    return false;
  }
}

export function registerAssetCache(): void {
  if (!isSupported()) {
    return;
  }

  // After load: registration competes with the initial render for bandwidth
  // and main-thread time, and nothing on first paint depends on it.
  window.addEventListener("load", () => {
    void navigator.serviceWorker.register(SERVICE_WORKER_URL).catch(() => {
      // A failed registration leaves any previously cached bytes in place.
      // Nothing else breaks: assets are fetched normally either way.
    });
  });
}

/**
 * Discards any canvas assets a previous version of the worker cached.
 *
 * Called on logout. The current worker purges on activate and caches nothing
 * afterwards, so this is normally a no-op; it still matters for a client that
 * cached under the old worker and signs out before the new one activates. A
 * cache is per browser profile, not per account, so two users sharing a
 * machine would otherwise share cached scene art — and the second would read
 * bytes the server never authorised for them.
 */
export function clearAssetCache(): void {
  if (!isSupported()) {
    return;
  }
  navigator.serviceWorker.controller?.postMessage({
    type: "clear-canvas-asset-cache",
  });
}
