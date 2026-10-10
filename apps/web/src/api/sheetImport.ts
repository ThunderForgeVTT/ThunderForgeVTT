/**
 * Spec 048: bringing a character in from a sheet
 * (contracts/graphql-sheet-import.md). The browser reads the sheet and asks
 * for a plan; nothing is uploaded until the player applies it, and the
 * server reads the file again and refuses unless its plan is the same one.
 */
import { postGraphQL, postGraphQLMultipart } from "@/api/graphqlClient";

export type FieldCertainty = "READ" | "UNCERTAIN" | "UNREAD" | "CORRECTED";
export type ContentResolution =
  | "WORLD"
  | "STAGED_EXISTING"
  | "STAGED_NEW"
  | "DIFFERS";

export interface SheetSource {
  page: number;
  x0: number;
  y0: number;
  x1: number;
  y1: number;
  text: string;
}

export interface SheetFieldChange {
  path: string;
  target: string;
  old: unknown;
  new: unknown;
  certainty: FieldCertainty;
  reason: string | null;
  source: SheetSource | null;
  playState: boolean;
}

export interface SheetContentChange {
  kind: string;
  name: string;
  resolution: ContentResolution;
  worldId: string | null;
  stagedId: string | null;
  removed: boolean;
  /** On a re-import: on the sheet and linked already. */
  linked: boolean;
}

export interface SheetUnmapped {
  path: string;
  value: unknown;
  goesTo: string;
}

/** A derived number the sheet prints that the rules disagree with. */
export interface SheetCrossCheck {
  path: string;
  sheet: unknown;
  derived: unknown;
}

/** Re-import: a value in play kept unless named in `overwritePlayState`. */
export interface SheetKeptInPlay {
  target: string;
  current: unknown;
  sheet: unknown;
}

export interface SheetImportPlan {
  readerId: string;
  readerVersion: string;
  fields: SheetFieldChange[];
  content: SheetContentChange[];
  unmapped: SheetUnmapped[];
  crossChecks: SheetCrossCheck[];
  keptInPlay: SheetKeptInPlay[];
  isReimport: boolean;
  planHash: string;
}

export interface UserSummary {
  id: string;
  username: string;
  displayName: string;
}

export interface ActorImportRecord {
  id: string;
  kind: "import" | "rollback";
  appliedAt: string;
  appliedBy: UserSummary;
  versionNo: number | null;
  restoredFrom: string | null;
  correctedFields: string[];
  fileAvailable: boolean;
  versionId: string | null;
}

/** Corrections are a map of the reading's neutral path to the value. */
export type SheetCorrections = Record<string, unknown>;

const PLAN_FIELDS = `
  readerId
  readerVersion
  fields {
    path
    target
    old
    new
    certainty
    reason
    source { page x0 y0 x1 y1 text }
    playState
  }
  content { kind name resolution worldId stagedId removed linked }
  unmapped { path value goesTo }
  crossChecks { path sheet derived }
  keptInPlay { target current sheet }
  isReimport
  planHash
`;

const RECORD_FIELDS = `
  id
  kind
  appliedAt
  appliedBy { id username displayName }
  versionNo
  restoredFrom
  correctedFields
  fileAvailable
  versionId
`;

/** The plan for `reading` on the actor. Writes nothing. */
export function getSheetImportPreview(
  actorId: string,
  reading: unknown,
  corrections: SheetCorrections = {},
): Promise<SheetImportPlan> {
  return postGraphQL<{ sheetImportPreview: SheetImportPlan }>(
    `
      query SheetImportPreview($actorId: UUID!, $reading: JSON!, $corrections: JSON) {
        sheetImportPreview(actorId: $actorId, reading: $reading, corrections: $corrections) {
          ${PLAN_FIELDS}
        }
      }
    `,
    { actorId, reading, corrections },
  ).then((data) => data.sheetImportPreview);
}

/** The actor's imports and rollbacks, newest first. */
export function getActorImports(actorId: string): Promise<ActorImportRecord[]> {
  return postGraphQL<{ actorImports: ActorImportRecord[] }>(
    `
      query ActorImports($actorId: UUID!) {
        actorImports(actorId: $actorId) {
          ${RECORD_FIELDS}
        }
      }
    `,
    { actorId },
  ).then((data) => data.actorImports);
}

export interface ApplySheetImportInput {
  actorId: string;
  file: Blob;
  corrections: SheetCorrections;
  overwritePlayState: string[];
  planHash: string;
  onProgress?: (sent: number, total: number) => void;
}

/** Upload the sheet and apply the reviewed plan. */
export function applySheetImport({
  actorId,
  file,
  corrections,
  overwritePlayState,
  planHash,
  onProgress,
}: ApplySheetImportInput): Promise<ActorImportRecord> {
  return postGraphQLMultipart<{ applySheetImport: ActorImportRecord }>(
    `
      mutation ApplySheetImport(
        $actorId: UUID!
        $file: Upload!
        $corrections: JSON
        $overwritePlayState: [String!]
        $planHash: String!
      ) {
        applySheetImport(
          actorId: $actorId
          file: $file
          corrections: $corrections
          overwritePlayState: $overwritePlayState
          planHash: $planHash
        ) {
          ${RECORD_FIELDS}
        }
      }
    `,
    { actorId, corrections, overwritePlayState, planHash },
    file,
    "file",
    { onUploadProgress: onProgress },
  ).then((data) => data.applySheetImport);
}

export type StagedState = "PENDING" | "ADOPTED" | "DECLINED";

/** An actor's link to a piece the world does not hold yet. */
export interface ActorStagedLink {
  id: string;
  stagedId: string;
  /** A vocabulary type, or `item`. */
  kind: string;
  name: string;
  state: StagedState;
}

/** The actor's links to staged pieces, which the play reads withhold. */
export function getActorStagedLinks(
  actorId: string,
): Promise<ActorStagedLink[]> {
  return postGraphQL<{ actorStagedLinks: ActorStagedLink[] }>(
    `
      query ActorStagedLinks($actorId: UUID!) {
        actorStagedLinks(actorId: $actorId) { id stagedId kind name state }
      }
    `,
    { actorId },
  ).then((data) => data.actorStagedLinks);
}
