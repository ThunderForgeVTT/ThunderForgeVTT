/**
 * Registers the canvas-asset service worker (`public/sw.js`).
 *
 * Kept deliberately small and failure-tolerant: the worker is a bandwidth
 * optimisation for large scene backgrounds, so every failure path here is a
 * silent no-op rather than something that could stop the app loading.
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
      // A failed registration just means assets are fetched normally.
    });
  });
}

/**
 * Discards cached canvas assets.
 *
 * Called on logout. A cache is per browser profile, not per account, so two
 * users sharing a machine would otherwise share cached scene art — and the
 * second would read bytes the server never authorised for them.
 */
export function clearAssetCache(): void {
  if (!isSupported()) {
    return;
  }
  navigator.serviceWorker.controller?.postMessage({ type: "clear-canvas-asset-cache" });
}
