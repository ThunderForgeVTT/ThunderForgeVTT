/**
 * apps/web/src/utils/raceOnSheet.ts
 *
 * A creature's race, as its game system declares where a race is written
 * (spec 044 FR-007a, research R7): the manifest's `appearance.race.source`.
 * The hero builder narrows its dice to that race; a system that declares no
 * source has no races, and its creatures roll as "any".
 *
 * What is returned is the sheet's own free text ("High Elf"). Turning it into
 * a look is `matchRace`'s job in `@thunderforge/heroes`, not this module's.
 * Nothing here writes a sheet (contract B5a).
 *
 * System-agnostic: it names no system and no field, as `sizeCategory.ts`
 * does not.
 */

import { slotKey } from "@/utils/sizeCategory";

export interface RaceSource {
  /** A manifest slot (`traitData`) and a field within it. */
  slot: string;
  field: string;
}

/** A manifest's `appearance.race.source`, or null when it declares none. */
export function raceSourceOf(manifest: unknown): RaceSource | null {
  const source = (
    manifest as { appearance?: { race?: { source?: unknown } } } | null
  )?.appearance?.race?.source as Partial<RaceSource> | undefined;
  if (typeof source?.slot !== "string" || typeof source?.field !== "string") {
    return null;
  }
  return { slot: source.slot, field: source.field };
}

/** The race a sheet names, read through the declared source, or null. */
export function raceOnSheet(
  manifest: unknown,
  systemData: Record<string, unknown> | null | undefined,
): string | null {
  const source = raceSourceOf(manifest);
  if (!source || !systemData) return null;
  const slot = systemData[slotKey(source.slot)] as
    | Record<string, unknown>
    | null
    | undefined;
  const value = slot?.[source.field];
  return typeof value === "string" && value.trim() !== "" ? value : null;
}
