/**
 * apps/web/src/utils/sizeCategory.ts
 *
 * A creature's size, as its game system declares sizes (spec 046 FR-030,
 * research R10): the manifest's `combat.sizes` block, which names where a
 * creature's size is stored (`source`) and how many squares a side each size
 * fills (`categories[].footprint`).
 *
 * Spec 018 began this as Genie's `sizeCategories: { key: { scale } }` table,
 * resolved into a token `scale` at placement. Spec 046 moved it: size is now
 * the footprint the grid uses — snapping, hit-testing, movement, reach — and
 * the server resolves each token's footprint for the board (`tokenGrid`). A
 * token's `scale` is the art multiplier again, and this module no longer
 * produces one. What is left here is for *showing* a size before a token is
 * placed ("Fills 2×2 squares").
 *
 * System-agnostic: it names no system and no field. Mirrors
 * `packs/systems/genie/web/src/lib/sizeCategory.ts`, since the two run in
 * packages that don't depend on each other.
 */

export interface SizeCategory {
  id: string;
  label: string;
  /** Squares a side. */
  footprint: number;
}

export interface DeclaredSizes {
  /** Where a creature's size is kept: a manifest slot (`traitData`) and field. */
  source: { slot: string; field: string };
  categories: SizeCategory[];
}

/** The footprint of a creature whose size is unknown, or undeclared: one square. */
export const DEFAULT_FOOTPRINT = 1;

/** A manifest's `combat.sizes`, or null when it declares none (M1). */
export function declaredSizesOf(manifest: unknown): DeclaredSizes | null {
  const sizes = (manifest as { combat?: { sizes?: unknown } } | null)?.combat
    ?.sizes as Partial<DeclaredSizes> | undefined;
  if (
    !sizes ||
    typeof sizes.source?.slot !== "string" ||
    typeof sizes.source?.field !== "string" ||
    !Array.isArray(sizes.categories)
  ) {
    return null;
  }
  return {
    source: { slot: sizes.source.slot, field: sizes.source.field },
    categories: sizes.categories.filter(
      (c): c is SizeCategory =>
        typeof c?.id === "string" && typeof c?.footprint === "number",
    ),
  };
}

/** `traitData` → `trait_data`: the key a slot is stored under on a sheet. */
export function slotKey(slot: string): string {
  return slot.replace(/[A-Z]/g, (letter) => `_${letter.toLowerCase()}`);
}

/** The size category a sheet names, or undefined. */
export function sizeCategoryOf(
  sizes: DeclaredSizes | null | undefined,
  category: string | null | undefined,
): SizeCategory | undefined {
  if (!sizes || !category) return undefined;
  return sizes.categories.find((c) => c.id === category);
}

/** The size id stored on a sheet, read through the declared source. */
export function sizeIdOnSheet(
  sizes: DeclaredSizes | null | undefined,
  sheet: Record<string, unknown> | null | undefined,
): string | null {
  if (!sizes || !sheet) return null;
  const slot = sheet[slotKey(sizes.source.slot)] as
    | Record<string, unknown>
    | null
    | undefined;
  const value = slot?.[sizes.source.field];
  return typeof value === "string" ? value : null;
}

/**
 * Squares a side for a category. Falls back to `DEFAULT_FOOTPRINT` for no
 * declaration, no category, or a category the system does not declare.
 */
export function resolveSizeFootprint(
  sizes: DeclaredSizes | null | undefined,
  category: string | null | undefined,
): number {
  return sizeCategoryOf(sizes, category)?.footprint ?? DEFAULT_FOOTPRINT;
}

/** "Fills 2×2 squares", "Fills ½ a square", "Fills 1 square". */
export function footprintText(footprint: number): string {
  if (footprint < 1) {
    return footprint === 0.5
      ? "Fills ½ a square"
      : `Fills ${footprint} of a square`;
  }
  if (footprint === 1) return "Fills 1 square";
  return `Fills ${footprint}×${footprint} squares`;
}
