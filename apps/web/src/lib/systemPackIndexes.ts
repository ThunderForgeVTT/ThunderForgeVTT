/**
 * `systemId -> the pack's web index`, discovered rather than listed.
 *
 * A system pack's `packs/systems/<id>/web/src/index.ts` is where it offers
 * the host optional pieces that are not a sheet: a character-sheet reader
 * (spec 048) and its own stat rolls for the play dock. Each such piece is an
 * optional export, and a host feature that wants one loads the index through
 * here and looks for its export. Nothing here names a game system.
 *
 * The glob is resolved by Vite at build time, so only reviewed, bundled packs
 * contribute, the same reasoning as `systemActorSheets.ts`. Each index is its
 * own chunk, fetched only for the system somebody is looking at.
 */

/** What a pack's web index may export. Every member is optional. */
export interface PackIndex {
  /** Lazily imports the pack's sheet reader compiled for the browser. */
  sheetReader?: () => Promise<unknown>;
  /**
   * The rolls a character's stored numbers offer, worked out by the pack's
   * own rules. Takes the actor's system data slots (`ability_data`,
   * `proficiency_data`, `trait_data`, ...) and returns `{ key, label,
   * formula }` entries. A pack without it gets the host's generic reading.
   */
  statRolls?: (systemData: Record<string, unknown>) => unknown;
}

const DISCOVERED = import.meta.glob<PackIndex>(
  "../../../../packs/systems/*/web/src/index.ts",
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

/** Whether a system's pack ships a web index at all. */
export function hasPackIndex(gameSystemId: string | null | undefined): boolean {
  return !!gameSystemId && gameSystemId in PACK_INDEXES;
}

/**
 * A system's web index, or `null` where there is no system or its pack ships
 * no index. Vite caches the module, so a second call costs nothing.
 */
export async function loadPackIndex(
  gameSystemId: string | null | undefined,
): Promise<PackIndex | null> {
  if (!gameSystemId) return null;
  const load = PACK_INDEXES[gameSystemId];
  return load ? load() : null;
}
