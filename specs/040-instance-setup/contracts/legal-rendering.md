# Contract: Operator values in published legal prose

The narrowest contract here, and the one with the sharpest edge: this is the
only place in the product where a value an operator typed is rendered inside
text the repository authored and a lawyer is expected to sign off.

## What is there today

- `apps/web/src/legal/legalDocuments.ts` reads `legal/*.md` with
  `import.meta.glob(..., { eager: true })` — **resolved by Vite at build
  time**. There is no fetch and nothing is read at runtime. Its module docs
  state the invariant: this text is ours, compiled in, which is why it may be
  rendered without the server's sanitizing markdown pipeline, and "nothing
  here should ever render a document that did not come from this glob."
- `apps/web/src/components/legal/LegalProse.tsx` renders four constructs —
  paragraphs, `**bold**`, `[text](url)` — on the stated grounds that the input
  is trusted.
- `legal/terms-of-service.md` carries **9** `[OPERATOR — …]` markers;
  `legal/privacy-policy.md` carries **5**.
- `apps/web/src/legal/__tests__/legalDocuments.test.ts` asserts the markers
  **survive rendering**: "a page that omits who holds your data while reading
  as complete is worse than one that visibly has a blank."
- `apps/web/src/pages/legal/DmcaCompliancePage.tsx` hard-codes the designated
  agent as JSX literals — `Copyright Agent, ThunderForge`, `[Configure via
  instance legal/compliance settings before launch]`,
  `dmca@thunderforge.example` — with a comment saying these are "instance
  configuration values… a value the operator supplies rather than something
  anyone writes." That comment has been waiting for this feature.

## The mechanism

The prose stays compiled in. A **closed set of named tokens** is substituted at
render time from resolved settings.

```ts
// apps/web/src/legal/operatorTokens.ts
/** The only tokens that may appear in legal/*.md. Closed, declared, tested. */
export const OPERATOR_TOKENS = {
  "{{operator.name}}":                 "operator.name",
  "{{operator.contact_email}}":        "operator.contact_email",
  "{{operator.jurisdiction}}":         "operator.jurisdiction",
  "{{notice.contact_name}}":           "notice.contact_name",
  "{{notice.contact_email}}":          "notice.contact_email",
  "{{notice.contact_postal_address}}": "notice.contact_postal_address",
} as const;

export interface Substituted {
  /** Segments alternating literal prose and substituted values. */
  segments: Array<{ kind: "prose"; text: string } | { kind: "value"; text: string }>;
}
```

## Rules

1. **The token set is closed.** A token not in `OPERATOR_TOKENS` is left in the
   text verbatim, exactly as `[OPERATOR — …]` is today. There is no
   general-purpose template engine over legal prose.
2. **An unset setting renders the existing visible marker**, not an empty
   string. `legalDocuments.test.ts`'s invariant survives verbatim: a page with
   a blank is better than a page that reads as complete and names nobody.
3. **A substituted value is rendered as text and never parsed.** It does not
   pass through `LegalProse`'s inline `**bold**` / `[text](url)` matcher. An
   operator name containing `[x](y)` must not become a link in the terms of
   service. This is the trust boundary `legalDocuments.ts` documents, and
   moving it is why this needs an ADR rather than a helper function.
4. **Values reach the client through an unauthenticated query**, because spec
   039's FR-056 requires the notice contact be discoverable by anyone who
   needs to file a notice, without an account. Only the six declared tokens
   are exposed by it; nothing else about the instance's configuration is.
5. **The DMCA agent designation reads the same settings** instead of JSX
   literals. Name, postal address and electronic contact become
   `notice.contact_name`, `notice.contact_postal_address`,
   `notice.contact_email`, with the same "configure before launch" text as the
   unset rendering.
6. **Setup states that registering a designated agent is the operator's own
   obligation** (spec 039 FR-055). This software does not file with any
   copyright office, and the setup screen says so where the notice contact is
   collected.

```graphql
type PublishedOperatorValues {
  operatorName: String
  operatorContactEmail: String
  operatorJurisdiction: String
  noticeContactName: String
  noticeContactEmail: String
  noticeContactPostalAddress: String
}

extend type Query {
  """
  The six values the published legal pages render. Deliberately unauthenticated
  (spec 039 FR-056): somebody who needs to file a notice has no account here.
  Null for a value the operator has not set — the page then shows its marker.
  """
  publishedOperatorValues: PublishedOperatorValues!
}
```

## The part this does not solve

Of the 14 markers, **4 are values** this substitutes. The other **10 are prose
an operator has to write**: the governing jurisdiction, how a change to the
terms and to the privacy policy will be announced, whether the instance is
invite-only, community-specific additions, the warranty disclaimer, the
liability limitation, and whether the instance is directed at children.

Collecting five fields does not make the terms of service placeholder-free.
The proposal — set out in research.md § D2 and needing the owner's
confirmation — is to split the markers into **fillable values** (required at
setup, refused if placeholder) and **operator prose blocks** (optional,
offered at setup, editable afterwards, each a readiness gap while unset), and
to read FR-005 as covering the values FR-002 collects.

**What is not proposed**: accepting free markdown from an operator and
rendering it through `LegalProse`. A prose block renders as plain paragraphs,
by rule 3, in the one document nobody re-reads.
