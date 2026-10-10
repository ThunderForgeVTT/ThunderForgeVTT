/**
 * `systemId -> character-sheet reader`, discovered rather than listed
 * (spec 048).
 *
 * A system pack that can read a character sheet exports `sheetReader` from
 * `packs/systems/<id>/web/src/index.ts`: a function that lazily imports the
 * pack's reader compiled for the browser. This file finds it through
 * `lib/systemPackIndexes.ts`, which discovers the packs' web indexes the way
 * `systemActorSheets.ts` finds sheets, so nothing here names a game system.
 *
 * Lazy twice over. The pack's `index.ts` is fetched only for the system of
 * the actor being imported into, and the reader's wasm only when a file is
 * actually read.
 */

import { hasPackIndex, loadPackIndex } from "@/lib/systemPackIndexes";

/** What a pack's reader module looks like once loaded. */
export interface SheetReaderModule {
  /** wasm-bindgen's initialiser; must be awaited once before `readSheet`. */
  default: (init?: unknown) => Promise<unknown>;
  /** The answer as JSON: see `SheetAnswer`. Never throws. */
  readSheet: (bytes: Uint8Array) => string;
}

/**
 * What a reader says about a file. `reading` is the pack's
 * `ImportedCharacter`, which the host passes on and never interprets.
 */
export type SheetAnswer =
  | { recognised: true; reading: unknown; error?: undefined; code?: undefined }
  | {
      recognised: boolean;
      /** The sentence the server would refuse with. */
      error: string;
      /** The server's refusal code: `SHEET_ENCRYPTED`, `SHEET_TOO_LARGE`, … */
      code?: string;
      reading?: undefined;
    };

const loading = new Map<string, Promise<SheetReaderModule | null>>();

/**
 * The reader for a system, initialised and ready, or `null` where the
 * system's pack ships none. Loaded once per system.
 */
export function loadSheetReader(
  gameSystemId: string | null | undefined,
): Promise<SheetReaderModule | null> {
  if (!gameSystemId || !hasPackIndex(gameSystemId)) {
    return Promise.resolve(null);
  }
  let pending = loading.get(gameSystemId);
  if (!pending) {
    pending = (async () => {
      const { sheetReader } = (await loadPackIndex(gameSystemId)) ?? {};
      if (!sheetReader) return null;
      const module = (await sheetReader()) as SheetReaderModule;
      await module.default();
      return module;
    })();
    // A failed fetch is not remembered: the next attempt tries again.
    pending.catch(() => loading.delete(gameSystemId));
    loading.set(gameSystemId, pending);
  }
  return pending;
}

/**
 * Read a file with a system's reader, or `null` where that system has none.
 */
export async function readSheet(
  gameSystemId: string | null | undefined,
  bytes: Uint8Array,
): Promise<SheetAnswer | null> {
  const reader = await loadSheetReader(gameSystemId);
  return reader ? (JSON.parse(reader.readSheet(bytes)) as SheetAnswer) : null;
}
