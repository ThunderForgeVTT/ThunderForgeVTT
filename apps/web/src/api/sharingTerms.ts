import { postGraphQL } from "@/api/graphqlClient";

/**
 * Spec 039 US1: the words somebody agrees to before they publish, read from the
 * server.
 *
 * # Why not the Vite glob
 *
 * `legalDocuments.ts` compiles every file in `legal/` into the bundle, and the
 * policy *pages* rightly use it. This does not, because nobody attests to a
 * policy page. A client that rendered its own copy of the terms while quoting
 * the server's version identity could record an agreement to words nobody was
 * shown — and the whole value of an attestation is that it says *which words*.
 *
 * So the words and the identity come from the same request, and the identity
 * goes back unmodified.
 */
export interface LegalSection {
  heading: string | null;
  body: string;
}

export interface VersionedLegalDocument {
  /** Opaque. Echo it back; never parse it. */
  versionId: string;
  slug: string;
  sections: LegalSection[];
}

const DOCUMENT_FIELDS = `
  versionId
  slug
  sections {
    heading
    body
  }
`;

/** The sharing terms in force right now. Requires a session. */
export function readSharingTerms(): Promise<VersionedLegalDocument> {
  return postGraphQL<{ sharingTerms: VersionedLegalDocument }>(`
    query SharingTerms {
      sharingTerms { ${DOCUMENT_FIELDS} }
    }
  `).then((data) => data.sharingTerms);
}

/**
 * What an operator takes on (US8).
 *
 * Readable without a session, because the person it is shown to is setting an
 * instance up and does not have an account yet.
 */
export function readOperatorStatement(): Promise<VersionedLegalDocument> {
  return postGraphQL<{ operatorStatement: VersionedLegalDocument }>(`
    query OperatorStatement {
      operatorStatement { ${DOCUMENT_FIELDS} }
    }
  `).then((data) => data.operatorStatement);
}
