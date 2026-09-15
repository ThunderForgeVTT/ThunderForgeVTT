/**
 * Genie: a creature's size category (spec 018 US3, T046; moved by spec 046).
 *
 * The manifest declares Genie's sizes under `combat.sizes` (`system.json`):
 * where an NPC's size is kept (`trait_data.size_category`) and how many squares
 * a side each fills. Until spec 046 this was a `sizeCategories` table of token
 * `scale`s, applied when a token was placed; now a creature's size is its
 * footprint on the grid, resolved by the server for every board, and a
 * token's `scale` is only how its art is drawn. This is the one pure seam
 * `SizeCategoryBadge.tsx` reads.
 */

export interface SizeCategory {
  id: string;
  label: string;
  /** Squares a side. */
  footprint: number;
}

export interface DeclaredSizes {
  source: { slot: string; field: string };
  categories: SizeCategory[];
}

/** The footprint of a creature of no known size: one square. */
export const DEFAULT_FOOTPRINT = 1;

/** A manifest's `combat.sizes`, or null when it declares none. */
export function declaredSizesOf(manifest: unknown): DeclaredSizes | null {
  const sizes = (manifest as { combat?: { sizes?: unknown } } | null)?.combat?.sizes as
    | Partial<DeclaredSizes>
    | undefined;
  if (
    !sizes ||
    typeof sizes.source?.slot !== 'string' ||
    typeof sizes.source?.field !== 'string' ||
    !Array.isArray(sizes.categories)
  ) {
    return null;
  }
  return {
    source: { slot: sizes.source.slot, field: sizes.source.field },
    categories: sizes.categories.filter(
      (c): c is SizeCategory => typeof c?.id === 'string' && typeof c?.footprint === 'number',
    ),
  };
}

/**
 * Squares a side for a Genie size category. Falls back to `DEFAULT_FOOTPRINT`
 * for no declaration, no category, or one the manifest does not declare (e.g.
 * stale data that slipped past `validate_trait_data`).
 */
export function resolveSizeFootprint(
  sizes: DeclaredSizes | null | undefined,
  category: string | null | undefined,
): number {
  if (!sizes || !category) {
    return DEFAULT_FOOTPRINT;
  }
  return sizes.categories.find((c) => c.id === category)?.footprint ?? DEFAULT_FOOTPRINT;
}
