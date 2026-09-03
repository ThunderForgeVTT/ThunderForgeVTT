/**
 * What this world calls its abilities.
 *
 * # Why there is one of these and there were six
 *
 * `WorldCompendiumPage`, `AbilityCompendiumTab`, `AbilityPreviewPanel`,
 * `AbilityDetailPage`, `ActorAbilitiesPanel` and the shared-ability page each
 * fetched the system manifest and cast its `abilityFacets` table themselves.
 * Spec 033's FR-006 requires every surface naming an ability type to use the
 * system's word for it, and six independent readers is six chances to disagree
 * about something they are required to agree on.
 *
 * The server now assembles the answer — `src/server/src/ability_vocabulary.rs`
 * — because it needs the same vocabulary to make refusals the browser cannot
 * (FR-013, FR-019, FR-023), and assembling it twice in two languages is how
 * the two come to differ. This module fetches that answer and reads it.
 *
 * Every lookup here is **total**: an absent vocabulary, an unknown identity or
 * a missing label all produce something correct to render. A pack failure must
 * never show a blank label or an empty tab set (FR-016, SC-013).
 */
import { postGraphQL } from "@/api/graphqlClient";

/** What an ability of a type attaches to. Exactly one, never a set. */
export type AbilityBinds = "CHARACTER" | "ITEM" | "NOTHING";

/** An ordered value a type's abilities carry — 5e's Level, another system's
 * Rank or Circle. One shape, many words. */
export interface AbilityGradeFacet {
  label: string;
  min: number;
  max: number;
}

/** One ability type, built in or declared by the system. A GM cannot tell
 * which is which, and that is the point. */
export interface AbilityTypeDeclaration {
  /** Stable identity — what is stored on the ability. */
  id: string;
  label: string;
  pluralLabel: string;
  order: number;
  builtin: boolean;
  binds: AbilityBinds;
  grade: AbilityGradeFacet | null;
}

/** The system's word for the concept itself, replacing "Ability"/"Abilities". */
export interface AbilityUmbrella {
  label: string;
  pluralLabel: string;
}

export interface AbilityVocabulary {
  umbrella: AbilityUmbrella;
  /** In display order. */
  types: AbilityTypeDeclaration[];
}

/**
 * What to show before the answer arrives, and if it never does.
 *
 * Not a degraded mode — it is what a system declaring nothing gets, which is
 * most systems. Rendering this while the real vocabulary loads means the
 * compendium never flashes an empty tab set.
 */
export const DEFAULT_VOCABULARY: AbilityVocabulary = {
  umbrella: { label: "Ability", pluralLabel: "Abilities" },
  types: [],
};

const VOCABULARY_FIELDS = `
  umbrella { label pluralLabel }
  types { id label pluralLabel order builtin binds grade { label min max } }
`;

type AbilityVocabularyQuery = { abilityVocabulary: AbilityVocabulary };

export function getAbilityVocabulary(
  worldId: string,
): Promise<AbilityVocabulary> {
  return postGraphQL<AbilityVocabularyQuery>(
    `
      query AbilityVocabulary($worldId: UUID!) {
        abilityVocabulary(worldId: $worldId) {
          ${VOCABULARY_FIELDS}
        }
      }
    `,
    { worldId },
  ).then((data) => data.abilityVocabulary);
}

/**
 * The declaration for a stored type identity, if this world recognises it.
 *
 * `null` is a real answer and the caller must handle it: an ability authored
 * under a system the world no longer runs keeps its type, and FR-034 says it
 * stays listed and editable rather than being re-typed or hidden.
 */
export function typeFor(
  vocabulary: AbilityVocabulary,
  id: string,
): AbilityTypeDeclaration | null {
  return vocabulary.types.find((kind) => kind.id === id) ?? null;
}

/**
 * Singular label for one type — the badge on a single ability.
 *
 * Falls back to the stored identity itself rather than to a built-in word.
 * That is FR-035 as clarified: an unrecognised type is shown as what it was
 * authored as, never dressed up as something else. The old
 * `toAbilityClassificationKey` resolved anything unknown to `"spell"`, which
 * is exactly the silent mislabelling FR-034 forbids.
 */
export function labelFor(vocabulary: AbilityVocabulary, id: string): string {
  return typeFor(vocabulary, id)?.label ?? id;
}

/** Plural label — tab headings and filters. */
export function pluralLabelFor(
  vocabulary: AbilityVocabulary,
  id: string,
): string {
  return typeFor(vocabulary, id)?.pluralLabel ?? id;
}

/** Whether this world's active system recognises a stored type identity. */
export function recognises(vocabulary: AbilityVocabulary, id: string): boolean {
  return typeFor(vocabulary, id) !== null;
}
