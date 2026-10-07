import {
  download as downloadWith,
  downloadBytes as downloadBytesWith,
  type Download,
  type DownloadOptions,
  type DownloadSettings,
} from "@thunderforge/downloads";
import {
  currentFeatureFlags,
  FEATURE_DOWNLOAD_IN_PARTS,
} from "@/api/featureFlags";

/**
 * Spec 080: the web app's one way to fetch a large file.
 *
 * Parts are on unless the instance has switched them off, and they read as
 * on before the flags have answered: the engine starts downloading before
 * anyone has asked, and a server that has said nothing has not said no.
 *
 * In a dev build a test may override the numbers through
 * `globalThis.__thunderforgeDownloadSettings`, so a small fixture is "large".
 * A production build never reads it.
 */
export function downloadSettings(): Partial<DownloadSettings> {
  const enabled = currentFeatureFlags()[FEATURE_DOWNLOAD_IN_PARTS] ?? true;
  const override = import.meta.env.DEV
    ? (
        globalThis as {
          __thunderforgeDownloadSettings?: Partial<DownloadSettings>;
        }
      ).__thunderforgeDownloadSettings
    : undefined;
  return { ...override, enabled: enabled && (override?.enabled ?? true) };
}

export function download(
  url: string,
  options: Omit<DownloadOptions, "settings"> = {},
): Promise<Download> {
  return downloadWith(url, { ...options, settings: downloadSettings() });
}

export function downloadBytes(
  url: string,
  options: Omit<DownloadOptions, "settings"> = {},
): Promise<Uint8Array> {
  return downloadBytesWith(url, { ...options, settings: downloadSettings() });
}
