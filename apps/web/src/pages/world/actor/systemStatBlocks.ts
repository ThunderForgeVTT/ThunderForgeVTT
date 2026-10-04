/**
 * `systemId -> stat blocks` dispatch, discovered rather than listed.
 *
 * Some systems print creatures: a block of numbers a Game Master copies onto
 * a monster so it can fight. Which numbers, and where each one is stored, is
 * the system's business entirely. So a pack that has them puts a
 * `StatBlockSource` at `packs/systems/<id>/web/src/StatBlocks.ts`, and this
 * file finds it, the same way `systemActorSheets.ts` finds a sheet and for
 * the same reason: a build-time glob, nothing loaded at run time, and no
 * shared file that names a game system.
 *
 * # What crosses the seam
 *
 * Data, in one direction. The host asks "which blocks do you have?" and
 * "what would applying this one write?", and gets back slot contents and a
 * list of attacks to create. `applyStatBlock.ts` then does the writing with
 * the application's own API. A pack is not handed a way to create abilities
 * or write another actor's data; it describes, the host acts, and the server
 * authorizes each write against the person at the keyboard as it always has.
 *
 * A system with no such file has no stat blocks, and every caller treats
 * that as the ordinary answer it is: the bestiary makes the creature's
 * pictures and leaves its numbers to the Game Master, exactly as before.
 */

import type { StatBlockSource, StatBlockSummary } from "@thunderforge/host";

const DISCOVERED = import.meta.glob<{ default: StatBlockSource }>(
  "../../../../../../packs/systems/*/web/src/StatBlocks.ts",
  { eager: true },
);

function systemIdFromPath(modulePath: string): string | null {
  const match = /packs\/systems\/([^/]+)\/web\/src\/StatBlocks\.ts$/.exec(
    modulePath,
  );
  return match ? match[1] : null;
}

export const SYSTEM_STAT_BLOCKS: Record<string, StatBlockSource> =
  Object.fromEntries(
    Object.entries(DISCOVERED).flatMap(([modulePath, module]) => {
      const systemId = systemIdFromPath(modulePath);
      return systemId ? [[systemId, module.default]] : [];
    }),
  );

/** The stat blocks a system ships, or `null` where it ships none. */
export function resolveStatBlocks(
  gameSystemId: string | null | undefined,
): StatBlockSource | null {
  if (!gameSystemId) {
    return null;
  }
  return SYSTEM_STAT_BLOCKS[gameSystemId] ?? null;
}

/**
 * The block a system has for a bestiary creature, or `null`: no blocks at
 * all, or none for this creature. Most of the bestiary has none in any
 * system, and that is not an error.
 */
export function statBlockForCreature(
  gameSystemId: string | null | undefined,
  bestiarySlug: string,
): StatBlockSummary | null {
  return (
    resolveStatBlocks(gameSystemId)?.blocks.find(
      (block) => block.bestiary === bestiarySlug,
    ) ?? null
  );
}
