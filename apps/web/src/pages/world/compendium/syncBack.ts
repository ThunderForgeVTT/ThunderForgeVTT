/**
 * Syncing a world's changes back to its collection (spec 050 FR-100 to
 * FR-105).
 *
 * Two calls, never one: the plan says everything the sync would do to the
 * shelf, and the sync lands only with that plan's stamp. If the collection or
 * the world's changes move on in between, the server refuses rather than
 * landing something nobody looked at (FR-103).
 */
import { postGraphQL } from "@/api/graphqlClient";
import type { ReadValue } from "@/engine/sdk/ReadValue";
import type { UnattachedDelta } from "./worldBooks";

export interface EntryContentShown {
  fieldValues: Record<string, ReadValue>;
  proseText: string | null;
}

export interface ShelfChange {
  kind: string;
  name: string;
  change: "REWRITTEN" | "TAKEN_OUT" | "ADDED";
  /** What the collection says now; null for an addition. */
  before: EntryContentShown | null;
  /** What it will say; null for an entry taken out. */
  after: EntryContentShown | null;
}

export interface SyncBackPlan {
  compendiumId: string;
  collectionTitle: string;
  worldName: string;
  baseVersion: number;
  changes: ShelfChange[];
  /** This world's changes that do not attach, which stay here. */
  staying: UnattachedDelta[];
  stamp: string;
}

export interface SyncBackOutcome {
  plan: SyncBackPlan;
  baseVersion: number;
  /** Changes in any world that no longer fit the new version. Still kept. */
  stranded: {
    worldId: string;
    worldName: string;
    deltas: UnattachedDelta[];
  }[];
}

const PLAN_FIELDS = `
  compendiumId
  collectionTitle
  worldName
  baseVersion
  changes {
    kind
    name
    change
    before { fieldValues proseText }
    after { fieldValues proseText }
  }
  staying { kind name form reason }
  stamp
`;

/** What syncing would do to the collection, without doing it. */
export async function syncBackPlan(
  worldId: string,
  compendiumId: string,
): Promise<SyncBackPlan> {
  const data = await postGraphQL<{ worldSyncBackPlan: SyncBackPlan }>(
    `query WorldSyncBackPlan($worldId: UUID!, $compendiumId: UUID!) {
       worldSyncBackPlan(worldId: $worldId, compendiumId: $compendiumId) {
         ${PLAN_FIELDS}
       }
     }`,
    { worldId, compendiumId },
  );
  return data.worldSyncBackPlan;
}

/** Land the plan that was shown, by its stamp. */
export async function syncBackToCollection(
  worldId: string,
  compendiumId: string,
  stamp: string,
): Promise<SyncBackOutcome> {
  const data = await postGraphQL<{ syncBackToCollection: SyncBackOutcome }>(
    `mutation SyncBackToCollection(
       $worldId: UUID!
       $compendiumId: UUID!
       $stamp: String!
     ) {
       syncBackToCollection(
         worldId: $worldId
         compendiumId: $compendiumId
         stamp: $stamp
       ) {
         plan { ${PLAN_FIELDS} }
         baseVersion
         stranded {
           worldId
           worldName
           deltas { kind name form reason }
         }
       }
     }`,
    { worldId, compendiumId, stamp },
  );
  return data.syncBackToCollection;
}
