/**
 * Whether this browser can keep world content on the device at all.
 *
 * # Why the panel needs this
 *
 * The cache is OPFS blobs, a WebCrypto key and IndexedDB records
 * (`thunderforge-cache-browser`). Where those are missing the crate raises
 * `CacheError::Unsupported` and degrades rather than crashing — which is the
 * right behaviour for the engine and the wrong thing to *show*, because a
 * cache that cannot work and a cache that has not been used yet both report
 * zero.
 *
 * A playtest found exactly that: a browser reporting "nothing served, 0 B
 * downloaded" against a world whose active scene had two placed, hashed
 * assets. The figures were true and the impression was false.
 *
 * Spec 031 FR-042: tell the user their browser cannot do this, rather than
 * showing an empty cache that reads as "nothing has happened yet".
 *
 * # Why a capability probe rather than a browser name
 *
 * The supported set is currently Chromium only (recorded in the
 * constitution), but naming browsers ages badly and is wrong the moment a
 * browser ships the missing piece. Asking whether the three APIs are present
 * answers the question the user actually has — *will my content be kept?* —
 * and keeps working when the answer changes.
 *
 * This deliberately does not probe *behaviour*, only presence. A browser that
 * exposes OPFS and then fails on write is a case for the engine's own error
 * reporting, not for a synchronous check the panel runs every few seconds.
 */

export interface CacheSupport {
  supported: boolean;
  /** Which required capabilities are absent, for telling the user why. */
  missing: string[];
}

export function detectCacheSupport(): CacheSupport {
  const missing: string[] = [];

  // Wrapped because a hostile or hardened context can throw on access rather
  // than returning undefined, and a diagnostics panel must never be the thing
  // that breaks the page.
  const has = (probe: () => boolean, label: string) => {
    let present = false;
    try {
      present = probe();
    } catch {
      present = false;
    }
    if (!present) missing.push(label);
  };

  has(
    () => typeof navigator !== "undefined" && !!navigator.storage?.getDirectory,
    "private file storage",
  );
  has(() => typeof crypto !== "undefined" && !!crypto.subtle, "encryption");
  has(() => typeof indexedDB !== "undefined", "local database");

  return { supported: missing.length === 0, missing };
}
