import { postGraphQL } from "@/api/graphqlClient";
import type { VersionedLegalDocument } from "@/api/sharingTerms";
import type { ModerationEntityType } from "@/types/moderation";

/**
 * Spec 039 US2: what somebody agreed to, when, and in order to publish what.
 *
 * `terms` is the words as they were when the agreement was made, read from the
 * server's archive — never today's terms under an old label.
 */
export interface Attestation {
  id: string;
  /** "share" or "operator". */
  purpose: string;
  /** Null once the account has been deleted. */
  subjectUsername: string | null;
  subjectUserId: string;
  attestedAt: string;
  termsVersionId: string;
  terms: VersionedLegalDocument;
  /** What was published: the thing itself, or a collection it is in. */
  publishableKind: string | null;
  publishableId: string | null;
  worldId: string | null;
}

/**
 * Anything an agreement can be asked about. `lore` is never published on its
 * own, so its agreements are always a collection's.
 */
export type CoveredKind = "collection" | "actor" | "item" | "ability" | "lore";

const ATTESTATION_FIELDS = `
  id
  purpose
  subjectUsername
  subjectUserId
  attestedAt
  termsVersionId
  terms {
    versionId
    slug
    sections {
      heading
      body
    }
  }
  publishableKind
  publishableId
  worldId
`;

/**
 * The kind a moderation case's content is asked about as.
 *
 * Takes a plain string as well as the enum because the server knows one more
 * entity type (`WORLD_ABILITY`) than the web's union does.
 */
export function coveredKindForCase(
  entityType: ModerationEntityType | string,
): CoveredKind | null {
  switch (entityType) {
    case "WORLD_ACTOR":
      return "actor";
    case "WORLD_ITEM":
      return "item";
    case "WORLD_ABILITY":
      return "ability";
    case "WORLD_LORE_ENTRY":
      return "lore";
    default:
      return null;
  }
}

/**
 * Every agreement under which one thing has been published — its own, and
 * every collection it is in — newest first. Admin-only.
 */
export function getAttestationsFor(
  kind: CoveredKind,
  id: string,
): Promise<Attestation[]> {
  return postGraphQL<{ attestationsFor: Attestation[] }>(
    `
      query AttestationsFor($publishableKind: String!, $publishableId: UUID!) {
        attestationsFor(publishableKind: $publishableKind, publishableId: $publishableId) {
          ${ATTESTATION_FIELDS}
        }
      }
    `,
    { publishableKind: kind, publishableId: id },
  ).then((data) => data.attestationsFor);
}
