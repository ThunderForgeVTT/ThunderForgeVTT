//! Genie Condition Registry
//!
//! Client-side resolver for Genie's condition keys, mirroring
//! packs/systems/genie/system.json's `conditions` block the same way
//! derived-data.ts's `calculateMaxWishPoints` mirrors `wishPoints` — a
//! plain, structurally-identical lookup table recomputed on read rather
//! than fetched from the manifest at render time (spec 018 User Story 4,
//! T050/T051).

/** One entry from the manifest's `conditions` block. */
export interface GenieConditionDefinition {
  key: string;
  label: string;
  description: string;
}

/**
 * Known Genie condition keys, mirroring `conditions` in
 * packs/systems/genie/system.json. Also the source of truth the server's
 * `validate_trait_data` (packs/systems/genie/server/src/validators.rs)
 * hardcodes its `valid_conditions` list against — keep the two in sync.
 */
export const GENIE_CONDITIONS: readonly GenieConditionDefinition[] = [
  {
    key: 'bound',
    label: 'Bound',
    description:
      "The Genie's power is constrained by their vessel, a Patron's rule, or a captor's ward — wish-granting abilities are suppressed or restricted until the binding is broken.",
  },
  {
    key: 'exposed',
    label: 'Exposed',
    description:
      "The Genie's true nature or vessel has been revealed, stripping away the concealment that normally protects them from mortal notice and interference.",
  },
  {
    key: 'favored',
    label: 'Favored',
    description:
      "The Genie currently holds a Patron's blessing, granting an edge on Manifestation rolls until the favor lapses or is spent.",
  },
];

const CONDITIONS_BY_KEY: ReadonlyMap<string, GenieConditionDefinition> = new Map(
  GENIE_CONDITIONS.map((condition) => [condition.key, condition]),
);

/**
 * Resolves a single condition key (as stored in `trait_data.active_conditions`)
 * to its manifest definition. Returns `undefined` for an unknown key rather
 * than throwing, so a stale/unrecognized key doesn't crash the sheet or
 * token rendering — callers decide how to display that case.
 */
export function resolveCondition(key: string): GenieConditionDefinition | undefined {
  return CONDITIONS_BY_KEY.get(key);
}

/**
 * Resolves a full `active_conditions` list to their manifest definitions,
 * in the same order, dropping any keys that don't resolve to a known
 * condition (see `resolveCondition`).
 */
export function resolveConditions(keys: readonly string[]): GenieConditionDefinition[] {
  return keys
    .map((key) => resolveCondition(key))
    .filter((condition): condition is GenieConditionDefinition => condition !== undefined);
}
