/**
 * One exponential backoff for the client.
 *
 * The delay before retry `n` (counting from 0) is `baseMs * 2^n`, capped at
 * `maxMs`. With `jitter: "full"` it is a random point between zero and that,
 * so a room of clients that lost the same network at the same moment do not
 * all come back on the same tick.
 *
 * Nothing is retried unless `shouldRetry` says so. A failure the server
 * answered (a refusal, a bad input) is not a reason to ask again; a request
 * that never arrived is.
 */

export interface Backoff {
  /** The first delay, before any doubling. */
  baseMs: number;
  /** No delay is longer than this. */
  maxMs: number;
  /** `"full"` picks a random delay up to the computed one. Default `"none"`. */
  jitter?: "none" | "full";
}

/** The wait before retry `attempt`, where 0 is the first retry. */
export function backoffDelay(
  attempt: number,
  { baseMs, maxMs, jitter = "none" }: Backoff,
  random: () => number = Math.random,
): number {
  const ceiling = Math.min(baseMs * 2 ** Math.max(0, attempt), maxMs);
  return jitter === "full" ? Math.floor(random() * ceiling) : ceiling;
}

export interface RetryOptions extends Backoff {
  /** Retries after the first try, so `retries: 2` is up to three tries. */
  retries: number;
  /** Whether `error`, from try `attempt` (0 first), is worth another try. */
  shouldRetry: (error: unknown, attempt: number) => boolean;
  /** Told before each wait; for telemetry and tests. */
  onRetry?: (error: unknown, attempt: number, delayMs: number) => void;
  /** Tests only: replaces the timer. */
  sleep?: (ms: number) => Promise<void>;
  /** Tests only: replaces `Math.random` for jitter. */
  random?: () => number;
}

const wait = (ms: number) =>
  new Promise<void>((resolve) => setTimeout(resolve, ms));

/**
 * Runs `run`, and again after a backoff each time it throws something
 * `shouldRetry` accepts, until it succeeds or the retries are spent. The
 * last error is the one thrown.
 */
export async function retry<T>(
  run: (attempt: number) => Promise<T>,
  options: RetryOptions,
): Promise<T> {
  const sleep = options.sleep ?? wait;
  for (let attempt = 0; ; attempt += 1) {
    try {
      return await run(attempt);
    } catch (error) {
      if (attempt >= options.retries || !options.shouldRetry(error, attempt)) {
        throw error;
      }
      const delayMs = backoffDelay(attempt, options, options.random);
      options.onRetry?.(error, attempt, delayMs);
      await sleep(delayMs);
    }
  }
}
