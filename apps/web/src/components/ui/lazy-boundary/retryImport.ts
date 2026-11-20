import { retry } from "@thunderforge/retry";
import { isChunkLoadError } from "./chunkLoadError";

/**
 * A code download that tries again before it gives up.
 *
 * A phone that changes network, or a host whose interfaces change, aborts a
 * chunk in flight. The next try usually lands, so a page waits a moment
 * longer instead of showing that it did not load. A chunk a deploy removed
 * fails every try, and the boundary still says so.
 */
export function retryImport<T>(load: () => Promise<T>): () => Promise<T> {
  return () =>
    retry(load, {
      retries: 3,
      baseMs: 300,
      maxMs: 3000,
      jitter: "full",
      shouldRetry: isChunkLoadError,
    });
}
