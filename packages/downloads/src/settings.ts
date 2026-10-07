/**
 * Spec 080 FR-023: how the downloader behaves, in one place.
 *
 * Callers pass only what they change; nothing restates a default.
 */
export interface DownloadSettings {
  /** Files smaller than this (bytes) are one plain request. */
  threshold: number;
  /** The size of one part, in bytes. */
  partSize: number;
  /** How many parts travel at once; also how far ahead of the reader. */
  concurrency: number;
  /** Retries per part before the download fails. */
  retries: number;
  /** The first pause before a retry; it doubles each time, capped. */
  retryDelayMs: number;
  /** `false` makes every download one plain request (the runtime switch). */
  enabled: boolean;
}

const MiB = 1024 * 1024;

export const DEFAULT_DOWNLOAD_SETTINGS: Readonly<DownloadSettings> =
  Object.freeze({
    threshold: 16 * MiB,
    partSize: 8 * MiB,
    concurrency: 4,
    retries: 5,
    retryDelayMs: 250,
    enabled: true,
  });

/** The longest pause between two tries of one part. */
export const MAX_RETRY_DELAY_MS = 4000;

export function resolveSettings(
  overrides: Partial<DownloadSettings> = {},
): DownloadSettings {
  const s = { ...DEFAULT_DOWNLOAD_SETTINGS, ...overrides };
  return {
    ...s,
    partSize: Math.max(1, Math.floor(s.partSize)),
    concurrency: Math.max(1, Math.floor(s.concurrency)),
    retries: Math.max(0, Math.floor(s.retries)),
  };
}

/** The pause before try `attempt` (1-based) of a part. */
export function retryDelay(
  settings: DownloadSettings,
  attempt: number,
): number {
  return Math.min(
    settings.retryDelayMs * 2 ** Math.max(0, attempt - 1),
    MAX_RETRY_DELAY_MS,
  );
}
