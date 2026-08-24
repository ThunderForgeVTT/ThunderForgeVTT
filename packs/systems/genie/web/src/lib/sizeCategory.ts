/**
 * Genie: size-category -> token-scale resolution (spec 018 US3, T046).
 *
 * The manifest (`system.json`'s `sizeCategories` key, research.md R6)
 * carries a lookup from a Genie NPC's `size_category` trait_data value
 * (validators.rs's six-value enum: diminutive/small/medium/large/huge/
 * colossal) to a numeric token `scale` multiplier. This is the one pure,
 * unit-testable seam both `SizeCategoryBadge.tsx` (display) and the GM
 * staging -> token placement flow (T047) resolve through, so the mapping
 * logic itself lives here rather than being duplicated in each caller.
 */

export interface SizeCategoryManifestEntry {
  scale: number;
  label?: string;
}

export type SizeCategoriesLookup = Record<string, SizeCategoryManifestEntry>;

/** Token scale used when no manifest entry matches — "medium" (1.0), the
 * engine's existing default token scale, so an unrecognized/missing size
 * category degrades to today's no-size-category behavior rather than an
 * arbitrary or jarring size. */
export const DEFAULT_SIZE_SCALE = 1;

/**
 * Resolve a Genie NPC's `size_category` string to its manifest `scale`
 * value. Falls back to `DEFAULT_SIZE_SCALE` for a missing lookup table,
 * a null/undefined category, or a category with no matching entry (e.g.
 * stale/unknown data that slipped past `validate_trait_data`).
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
