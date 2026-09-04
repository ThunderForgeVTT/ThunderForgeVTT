/**
 * What changing a world's system would affect, in numbers.
 *
 * Spec 033 FR-025 and ADR-065. Fetched when a Game Master opens the
 * confirmation, and the `digest` is handed back to the mutation — so the
 * acknowledgement means "I acknowledge **these** numbers" rather than "I
 * clicked something", and a world that gained content while the dialog was
 * open is re-confirmed rather than switched behind the GM's back.
 */
import { postGraphQL } from "@/api/graphqlClient";

export interface ContentCount {
  /** `actors`, `abilities`, `items`. */
  kind: string;
  /** The system this content was authored for, where it records one. */
  systemId: string | null;
  count: number;
}

export interface ContentInventory {
  counts: ContentCount[];
  /** Abilities that would lose their own tab under the target system. */
  becomingUnrecognised: number;
  /** True when the world switches without ceremony (FR-029). */
  isEmpty: boolean;
  digest: string;
}

type InventoryQuery = { worldContentInventory: ContentInventory };

export function getWorldContentInventory(
  worldId: string,
  targetSystemId?: string,
): Promise<ContentInventory> {
  return postGraphQL<InventoryQuery>(
    `
      query WorldContentInventory($worldId: UUID!, $targetSystemId: String) {
        worldContentInventory(worldId: $worldId, targetSystemId: $targetSystemId) {
          counts { kind systemId count }
          becomingUnrecognised
          isEmpty
          digest
        }
      }
    `,
    { worldId, targetSystemId },
  ).then((data) => data.worldContentInventory);
}
