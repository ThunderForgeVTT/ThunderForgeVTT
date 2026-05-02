import type { TokenDelta } from "./types";

type TokenDeltaCoalescerOptions = {
  flushIntervalMs: number;
  maxBatchSize: number;
  onFlush: (deltas: TokenDelta[]) => Promise<void>;
};

export type TokenDeltaCoalescer = {
  enqueue: (delta: TokenDelta) => Promise<void>;
  flush: () => Promise<void>;
  stop: () => Promise<void>;
};

export function createTokenDeltaCoalescer(options: TokenDeltaCoalescerOptions): TokenDeltaCoalescer {
  const pendingByToken = new Map<string, TokenDelta>();
  let flushTimer: ReturnType<typeof setInterval> | null = null;
  let flushing = false;

  async function runFlush() {
    if (flushing || pendingByToken.size === 0) {
      return;
    }

    flushing = true;

    try {
      const items = Array.from(pendingByToken.values()).slice(0, options.maxBatchSize);
      for (const item of items) {
        pendingByToken.delete(item.tokenId);
      }

      if (items.length > 0) {
        await options.onFlush(items);
      }
    } finally {
      flushing = false;
    }
  }

  function ensureTimer() {
    if (flushTimer) {
      return;
    }

    flushTimer = setInterval(() => {
      void runFlush();
    }, options.flushIntervalMs);
  }

  ensureTimer();

  return {
    async enqueue(delta) {
      pendingByToken.set(delta.tokenId, delta);
      if (pendingByToken.size >= options.maxBatchSize) {
        await runFlush();
      }
    },

    async flush() {
      await runFlush();
    },

    async stop() {
      if (flushTimer) {
        clearInterval(flushTimer);
        flushTimer = null;
      }

      await runFlush();
    },
  };
}
