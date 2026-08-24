/**
 * apps/web/src/utils/sizeCategory.ts
 *
 * Spec 018 (Genie house system) T047: resolves a staged NPC's size
 * category to a default token `scale`, using whatever the actor's game
 * system manifest publishes under its `sizeCategories` lookup table
 * (`packs/systems/*\/system.json`, research.md R6 of spec 018). Kept
 * system-agnostic (no import of any specific system pack) — apps/web
 * loads manifests dynamically per `gameSystemId`
 * (`@/contexts/GameSystemContext`'s `SystemManifest` has an index
 * signature, so any system can publish this key), and any future system
 * pack that wants "size category determines token footprint" gets it for
 * free by shipping the same `sizeCategories: { key: { scale } }` shape
 * Genie's `system.json` does. Mirrors
 * `packs/systems/genie/web/src/lib/sizeCategory.ts`'s `resolveSizeScale`
 * exactly (same fallback behavior), since the two run in different
 * packages that don't depend on each other.
 */

export interface SizeCategoryManifestEntry {
  scale: number;
  label?: string;
}

export type SizeCategoriesLookup = Record<string, SizeCategoryManifestEntry>;

/** Token scale used when no manifest entry matches — "medium" (1.0), the
 * engine's existing default token scale (`CreateTokenInput.scale` is
 * optional and defaults server-side to 1), so a system with no
 * `sizeCategories` table, or an NPC with no/unknown size category,
 * degrades to today's no-size-category behavior. */
export const DEFAULT_SIZE_SCALE = 1;

/**
 * Resolve an NPC's `size_category` trait-data string to its game
 * system's manifest `scale` value. Falls back to `DEFAULT_SIZE_SCALE`
 * for a missing lookup table, a null/undefined category, or a category
 * with no matching entry.
 */
export function resolveSizeScale(
  sizeCategories: SizeCategoriesLookup | null | undefined,
  category: string | null | undefined,
): number {
  if (!sizeCategories || !category) {
    return DEFAULT_SIZE_SCALE;
  }

  const entry = sizeCategories[category];
  if (!entry || typeof entry.scale !== "number") {
    return DEFAULT_SIZE_SCALE;
  }

  return entry.scale;
}
