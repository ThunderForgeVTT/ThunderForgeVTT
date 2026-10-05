/**
 * Whether an error is a part of the app failing to download, as opposed to a
 * part of the app failing.
 *
 * A deploy replaces every hashed file, so a tab left open asks for a chunk
 * that is no longer there. Each browser words that differently, and Vite adds
 * its own for a stylesheet a chunk depends on.
 */
const CHUNK_LOAD_MESSAGES = [
  "Failed to fetch dynamically imported module",
  "error loading dynamically imported module",
  "Importing a module script failed",
  "Unable to preload CSS",
];

export function isChunkLoadError(error: unknown): boolean {
  if (!(error instanceof Error)) {
    return false;
  }
  return CHUNK_LOAD_MESSAGES.some((message) => error.message.includes(message));
}
