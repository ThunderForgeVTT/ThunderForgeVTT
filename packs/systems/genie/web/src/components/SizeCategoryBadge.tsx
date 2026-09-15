import React from 'react';
import { resolveSizeFootprint, type DeclaredSizes } from '../lib/sizeCategory';

export interface SizeCategoryBadgeProps {
  /** The NPC's `trait_data.size_category` value (models.rs/validators.rs). */
  sizeCategory: string | null | undefined;
  /** The Genie manifest's `combat.sizes` (`system.json`; `declaredSizesOf`). */
  sizes: DeclaredSizes | null | undefined;
}

/**
 * Genie: SizeCategoryBadge (spec 018 tasks.md T046, US3).
 *
 * Displays an NPC's size category and how many squares it fills, from the
 * manifest's `combat.sizes` (spec 046) — the same footprint the server hands
 * every board for that NPC's tokens.
 */
export const SizeCategoryBadge: React.FC<SizeCategoryBadgeProps> = ({ sizeCategory, sizes }) => {
  if (!sizeCategory) {
    return null;
  }

  const entry = sizes?.categories.find((c) => c.id === sizeCategory);
  const footprint = resolveSizeFootprint(sizes, sizeCategory);
  const label = entry?.label ?? sizeCategory;

  return (
    <span
      className="genie-size-category-badge"
      data-testid="genie-size-category-badge"
      data-size-category={sizeCategory}
      data-footprint={footprint}
      title={`Fills ${footprint} ${footprint === 1 ? 'square' : 'squares'} a side`}
    >
      {label}
    </span>
  );
};

export default SizeCategoryBadge;
