/**
 * apps/web/src/utils/abilityFacets.ts
 *
 * Spec 025 (T023, FR-010..FR-013): resolves an ability classification to the
 * label the world's active game system wants to show for it.
 *
 * The underlying classification set is fixed and shared by every system
 * (FR-009), which is what keeps ability data portable across a system change
 * (FR-013). Each system may optionally publish an `abilityFacets` block in its
 * `packs/systems/<id>/system.json` re-expressing those classifications in its
 * own vocabulary — a 5E-style system showing "Spells"/"Feats", Genie showing
 * "Scrolls".
 *
 * This resolves the naming collision with the manifest's existing top-level
 * `abilities` key, which holds ability *scores* (Might/Cunning/Spirit,
 * STR/DEX/…) — a different concept entirely, and one every shipped pack
 * already uses. Neither concept is renamed (FR-014); the new block is
 * `abilityFacets`.
 *
 * Kept system-agnostic, mirroring `./sizeCategory.ts`: no import of any
 * specific system pack, and any future pack that ships the same shape gets
 * facet labels for free. Every lookup here is total — it always returns a
 * usable string, never throws, never returns undefined — so a missing,
 * partial, or malformed manifest block degrades to the built-in labels rather
 * than breaking the view.
 */

export type AbilityClassificationKey = "spell" | "feat" | "power" | "talent";

export interface AbilityFacetEntry {
  label: string;
  pluralLabel?: string;
}

/** Shape published under a manifest's `abilityFacets` key. Keys are not
 * constrained to `AbilityClassificationKey` because a pack is free to ship
 * junk; unknown keys are simply ignored. */
export type AbilityFacetsLookup = Record<string, AbilityFacetEntry>;

/** Built-in labels, used whenever a system supplies no facet for a
 * classification — which is every currently-shipped pack until one opts in.
 * A system is never required to publish facets (FR-011). */
export const DEFAULT_ABILITY_FACETS: Record<
  AbilityClassificationKey,
  Required<AbilityFacetEntry>
> = {
  spell: { label: "Spell", pluralLabel: "Spells" },
  feat: { label: "Feat", pluralLabel: "Feats" },
  power: { label: "Power", pluralLabel: "Powers" },
  talent: { label: "Talent", pluralLabel: "Talents" },
};

export const ABILITY_CLASSIFICATION_KEYS: AbilityClassificationKey[] = [
  "spell",
  "feat",
  "power",
  "talent",
];

/** Returns the manifest entry for a classification only if it is structurally
 * usable — a pack can ship a bare string, a null, or an empty label, and none
 * of those should reach the UI. */
function usableEntry(
  lookup: AbilityFacetsLookup | null | undefined,
  classification: AbilityClassificationKey,
): AbilityFacetEntry | null {
  if (!lookup || typeof lookup !== "object") {
    return null;
  }
  const entry = lookup[classification];
  if (!entry || typeof entry !== "object") {
    return null;
  }
  if (typeof entry.label !== "string" || entry.label.trim() === "") {
    return null;
  }
  return entry;
}

/**
 * Singular label for one classification — the badge on a single ability.
 * Falls back to the built-in default for a missing table, a missing key, a
 * non-object entry, or an empty/whitespace-only label.
 */
export function resolveAbilityLabel(
  lookup: AbilityFacetsLookup | null | undefined,
  classification: AbilityClassificationKey,
): string {
  const entry = usableEntry(lookup, classification);
  return entry ? entry.label : DEFAULT_ABILITY_FACETS[classification].label;
}

/**
 * Plural label — group headings and filter labels.
 *
 * Falls back to the entry's own singular `label` when it publishes no
 * `pluralLabel`, and **never** derives a plural by appending "s": not every
 * term or language pluralizes that way, so guessing would produce worse output
 * than reusing the singular. Only when the entry is unusable altogether does
 * this fall back to the built-in default plural.
 */
export function resolveAbilityPluralLabel(
  lookup: AbilityFacetsLookup | null | undefined,
  classification: AbilityClassificationKey,
): string {
  const entry = usableEntry(lookup, classification);
  if (!entry) {
    return DEFAULT_ABILITY_FACETS[classification].pluralLabel;
  }
  if (
    typeof entry.pluralLabel === "string" &&
    entry.pluralLabel.trim() !== ""
  ) {
    return entry.pluralLabel;
  }
  return entry.label;
}

/** Narrows an arbitrary string (e.g. a GraphQL enum lowercased) to a known
 * classification key, so callers can resolve a label without an unchecked
 * cast. Returns `"spell"` for anything unrecognized, matching the server's
 * own `AbilityClassification::from_db_str` fallback. */
export function toAbilityClassificationKey(
  value: string,
): AbilityClassificationKey {
  const lowered = value.toLowerCase();
  return (ABILITY_CLASSIFICATION_KEYS as string[]).includes(lowered)
    ? (lowered as AbilityClassificationKey)
    : "spell";
}
