import React from 'react';
import { resolveSizeScale, type SizeCategoriesLookup } from '../lib/sizeCategory';

export interface SizeCategoryBadgeProps {
  /** The NPC's `trait_data.size_category` value (models.rs/validators.rs). */
  sizeCategory: string | null | undefined;
  /** The Genie manifest's `sizeCategories` lookup (`system.json`, research.md R6). */
  sizeCategories: SizeCategoriesLookup | null | undefined;
}

/**
 * Genie: SizeCategoryBadge (spec 018 tasks.md T046, US3).
 *
 * Displays an NPC's size category alongside the token `scale` it resolves
 * to via the manifest's `sizeCategories` lookup table (research.md R6) —
 * the same resolution `resolveSizeScale` (lib/sizeCategory.ts) that the
 * GM staging -> token placement flow (T047) uses to default a staged
 * token's footprint.
 */
export const SizeCategoryBadge: React.FC<SizeCategoryBadgeProps> = ({
  sizeCategory,
  sizeCategories,
}) => {
  if (!sizeCategory) {
    return null;
  }

  const entry = sizeCategories?.[sizeCategory];
  const scale = resolveSizeScale(sizeCategories, sizeCategory);
  const label = entry?.label ?? sizeCategory;

  return (
    <span
      className="genie-size-category-badge"
      data-testid="genie-size-category-badge"
      data-size-category={sizeCategory}
      data-scale={scale}
      title={`Default token scale: ${scale}x`}
    >
      {label}
    </span>
  );
};

export default SizeCategoryBadge;
