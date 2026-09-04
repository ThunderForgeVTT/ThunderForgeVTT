/**
 * Abilities an item carries (spec 033 FR-020).
 *
 * The item peer of `actorAbilities.ts`. Kept separate from the item's
 * *effects*, which are a different concept at a different layer: an effect is
 * a mechanical rule the resolution layer consumes, an ability is named,
 * described, permissioned, shareable content. They are reconciled where a
 * Game Master meets the confusion — on the item, as one list — and nowhere
 * else.
 */
import { postGraphQL } from "@/api/graphqlClient";

export interface ItemAbilityEntryRecord {
  id: string;
  /** `null` once the ability itself is deleted; the row survives as a tombstone. */
  abilityId: string | null;
  abilityName: string;
  /** The stored type identity, or `null` for a tombstone. */
  classification: string | null;
  grade: number | null;
}

type ItemAbilitiesQuery = { itemAbilities: ItemAbilityEntryRecord[] };

export function getItemAbilities(
  itemId: string,
): Promise<ItemAbilityEntryRecord[]> {
  return postGraphQL<ItemAbilitiesQuery>(
    `
      query ItemAbilities($itemId: UUID!) {
        itemAbilities(itemId: $itemId) {
          id
          abilityId
          abilityName
          classification
          grade
        }
      }
    `,
    { itemId },
  ).then((data) => data.itemAbilities);
}
