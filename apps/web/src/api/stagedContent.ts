/**
 * Spec 048 US3: the queue of pieces players brought in on their sheets,
 * which a Game Master or Trusted Player adopts into the world or declines.
 * Plain GraphQL fetches: the world store does not hold staged content.
 */
import { postGraphQL } from "@/api/graphqlClient";
import type { StagedState, UserSummary } from "@/api/sheetImport";

export interface StagedActor {
  id: string;
  label: string;
}

export interface StagedPiece {
  id: string;
  /** A vocabulary type, or `item`. */
  kind: string;
  name: string;
  fieldValues: Record<string, unknown>;
  state: StagedState;
  broughtBy: UserSummary;
  actors: StagedActor[];
  /** The piece of the same name this one differs from. */
  differsFrom: string | null;
  decidedBy: UserSummary | null;
  decidedAt: string | null;
  abilityId: string | null;
  itemId: string | null;
}

const FIELDS = `
  id kind name fieldValues state
  broughtBy { id username displayName }
  actors { id label }
  differsFrom
  decidedBy { id username displayName }
  decidedAt abilityId itemId
`;

export function listStagedContent(
  worldId: string,
  state?: StagedState,
): Promise<StagedPiece[]> {
  return postGraphQL<{ stagedContent: StagedPiece[] }>(
    `
      query StagedContent($worldId: UUID!, $state: StagedState) {
        stagedContent(worldId: $worldId, state: $state) { ${FIELDS} }
      }
    `,
    { worldId, state: state ?? null },
  ).then((data) => data.stagedContent);
}

export function adoptStagedContent(id: string): Promise<StagedPiece> {
  return postGraphQL<{ adoptStagedContent: StagedPiece }>(
    `
      mutation AdoptStagedContent($id: UUID!) {
        adoptStagedContent(id: $id) { ${FIELDS} }
      }
    `,
    { id },
  ).then((data) => data.adoptStagedContent);
}

export function adoptAllStagedContent(
  worldId: string,
  playerId: string,
): Promise<StagedPiece[]> {
  return postGraphQL<{ adoptAllStagedContent: StagedPiece[] }>(
    `
      mutation AdoptAllStagedContent($worldId: UUID!, $playerId: UUID!) {
        adoptAllStagedContent(worldId: $worldId, playerId: $playerId) { ${FIELDS} }
      }
    `,
    { worldId, playerId },
  ).then((data) => data.adoptAllStagedContent);
}

export function declineStagedContent(id: string): Promise<StagedPiece> {
  return postGraphQL<{ declineStagedContent: StagedPiece }>(
    `
      mutation DeclineStagedContent($id: UUID!) {
        declineStagedContent(id: $id) { ${FIELDS} }
      }
    `,
    { id },
  ).then((data) => data.declineStagedContent);
}

export function revisitStagedContent(
  id: string,
  state: Exclude<StagedState, "DECLINED">,
): Promise<StagedPiece> {
  return postGraphQL<{ revisitStagedContent: StagedPiece }>(
    `
      mutation RevisitStagedContent($id: UUID!, $state: StagedState!) {
        revisitStagedContent(id: $id, state: $state) { ${FIELDS} }
      }
    `,
    { id, state },
  ).then((data) => data.revisitStagedContent);
}
