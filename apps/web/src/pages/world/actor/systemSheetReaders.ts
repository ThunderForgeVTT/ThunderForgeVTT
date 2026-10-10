/**
 * `systemId -> character-sheet reader`, discovered rather than listed
 * (spec 048).
 *
 * A system pack that can read a character sheet exports `sheetReader` from
 * `packs/systems/<id>/web/src/index.ts`: a function that lazily imports the
 * pack's reader compiled for the browser. This file finds it, the way
 * `systemActorSheets.ts` finds sheets, and for that file's reasons: the glob
 * is resolved by Vite at build time, so only reviewed, bundled packs
 * contribute, and nothing here names a game system.
 *
 * Lazy twice over. The pack's `index.ts` is fetched only for the system of
 * the actor being imported into, and the reader's wasm only when a file is
 * actually read.
 */

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

type PackIndex = { sheetReader?: () => Promise<unknown> };

const DISCOVERED = import.meta.glob<PackIndex>(
  "../../../../../../packs/systems/*/web/src/index.ts",
);

function systemIdFromPath(modulePath: string): string | null {
  const match = /packs\/systems\/([^/]+)\/web\/src\/index\.ts$/.exec(
    modulePath,
  );
  return match ? match[1] : null;
}

const PACK_INDEXES: Record<string, () => Promise<PackIndex>> =
  Object.fromEntries(
    Object.entries(DISCOVERED).flatMap(([modulePath, load]) => {
      const systemId = systemIdFromPath(modulePath);
      return systemId ? [[systemId, load]] : [];
    }),
  );

const loading = new Map<string, Promise<SheetReaderModule | null>>();

/**
 * The reader for a system, initialised and ready, or `null` where the
 * system's pack ships none. Loaded once per system.
 */
export function loadSheetReader(
  gameSystemId: string | null | undefined,
): Promise<SheetReaderModule | null> {
  if (!gameSystemId) return Promise.resolve(null);
  const load = PACK_INDEXES[gameSystemId];
  if (!load) return Promise.resolve(null);
  let pending = loading.get(gameSystemId);
  if (!pending) {
    pending = (async () => {
      const { sheetReader } = await load();
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
