# Contract: The terms, the attestation, and the record

Three surfaces: how a client learns what the words are, what it sends back, and
how anyone reads the result years later.

The words come from `legal/sharing-terms.md`, compiled into the server with
`include_str!` and served from there. The web app keeps its Vite glob for the
policy *pages*; it does not use it here, because a client rendering its own copy
of the text while quoting the server's version identity can attest to words
nobody was shown.

## Reading the terms

```graphql
type LegalSection {
  "null for the text before the first heading."
  heading: String
  body: String!
}

type VersionedLegalDocument {
  "Opaque. e.g. \"sharing-terms@4f2a9c1e77b03d58\". Never parse it."
  versionId: String!
  slug: String!
  sections: [LegalSection!]!
}

extend type Query {
  "The sharing terms in force right now, and the identity to attest to."
  sharingTerms: VersionedLegalDocument!

  "The operator statement in force right now (US8)."
  operatorStatement: VersionedLegalDocument!

  """
  One archived version, by id. Used to read what an old attestation actually
  said. Admin-only: this is the notice-handling surface, not a public archive.
  """
  legalDocumentVersion(versionId: String!): VersionedLegalDocument
}
```

`sharingTerms` requires a session — you cannot publish without one — and is
cheap: the body is a compiled-in constant and the hash is computed once at
startup.

## Attesting

```graphql
input AttestationInput {
  "The versionId from sharingTerms, echoed back unmodified."
  termsVersionId: String!
}
```

That is the entire input, and its smallness is the point. The client sends no
text, no timestamp, no identity and no record — every one of those is the
server's to write (FR-014). A client that could supply them is a client that
could write its own evidence.

Every publishing mutation takes it. See `publishing-gate.md`.

## Reading a record

```graphql
type Attestation {
  id: UUID!
  "\"share\" or \"operator\"."
  purpose: String!
  "Null once the account has been deleted (FR-010, FR-037)."
  subjectUsername: String
  subjectUserId: UUID!
  attestedAt: String!
  termsVersionId: String!
  "The words as they were. Resolved from the archive, never from the constant."
  terms: VersionedLegalDocument!
  publishableKind: String
  publishableId: UUID
  worldId: UUID
}

extend type Query {
  """
  Every agreement under which one thing has been published, newest first: its
  own, and those of every collection it is currently in. Admin-only.
  publishableKind is "collection", "actor", "item", "ability" or "lore".
  """
  attestationsFor(publishableKind: String!, publishableId: UUID!): [Attestation!]!

  "The caller's own attestations, newest first."
  myAttestations(limit: Int): [Attestation!]!
}
```

## Rules

1. `versionId` is opaque. Nothing in the product parses it, compares it for
   ordering, or derives the document from it. It is compared for equality
   against `terms_versions` and nothing else.
2. `attestationsFor` is the SC-003 surface: a person handling a notice reaches
   who agreed, when and the exact words in one query, without a developer. It
   is admin-only for the same reason `moderationCase` is.
3. `Attestation.terms` resolves through `terms_versions`. It **must not** be
   implemented as "the current document with a version label", which is the
   failure FR-008 exists to prevent and the easiest one to write by accident.
4. An attestation is returned whether or not the share it authorised still
   exists (FR-007). There is no join to a share row and no filter on one.
5. `myAttestations` is available to a **disabled** account through the allowlist
   in `standing-and-termination.md` — a person shut out should still be able to
   see what they agreed to.
6. Deleting an account nulls `subjectUsername` and nothing else. The row stays.

## What is deliberately absent

- **No mutation that creates an attestation on its own.** An attestation exists
  only as a side effect of a publish succeeding. A standalone `attest` mutation
  would let a client bank agreements ahead of time, which is FR-003 defeated.
- **No `deleteAttestation`, and no admin edit.** A record somebody can revise is
  not a record.
- **No listing by version, by world or globally.** `attestationsFor` answers
  about one named thing, which is the question a notice actually asks.
  *Amended 2026-09-10:* "about one named thing" includes the collections that
  thing is in. A shared collection serves its members live, so a notice about an
  item — and about any lore entry, which is only ever published inside a
  collection — is answered by the collection's agreement as much as by the
  item's own. Membership is read as it is now: something removed from a
  collection after a copy was adopted is not reached, and following copies is
  ADR-079's question.
- **No IP address and no user agent.** Spec 035 set the rule that a record
  describes the act and never the person, and spec 036 followed it for sessions.
  An attestation is who, when, which words, which act — that is what makes it
  evidence, and an address adds nothing to it.
