# Contract: The operator acknowledgement, and the notice contact

US8. Somebody who deploys their own instance never signs up for anything — they
clone a repository and run a binary — so first-run setup is the only moment at
which a human being becomes an operator and the only place this can be said.

**Boundary with spec 040.** Spec 040 (Instance Setup and Configuration) owns the
setup screen and *how* the operator identity and notice contact are collected
and edited: its FR-001 to FR-009. This spec owns the *rule* — that the instance
knows who runs it, has somebody to notify, and refuses to publish without one.
Where the two overlap they must agree; if they drift, 040 owns the screen and
039 owns the rule. This contract states only 039's half.

## The acknowledgement is an attestation

Not a parallel mechanism. FR-043 says the operator record is made "on the same
terms as a sharing attestation", and the only way to be sure of that is for it
to be the same table, the same version archive and the same code:
`attestations` with `purpose = 'operator'`, `publishable_kind` and
`publishable_id` null, the subject being the administrator completing setup.

```graphql
extend type Mutation {
  """
  Complete first-run setup. Gains one required field: the version of the
  operator statement the person was shown.
  """
  adminSetupBasic(
    username: String!
    email: String!
    password: String!
    operatorAcknowledgement: AttestationInput!
  ): AdminSetupResult!
}

extend type Query {
  "The operator statement in force, and its version. Readable without a session."
  operatorStatement: VersionedLegalDocument!

  "The acknowledgement this instance holds, if any. Admin only."
  instanceOperatorAcknowledgement: Attestation
}
```

`adminSetupBasic` will not complete without it (FR-041). There is no path
through setup that produces an instance with an administrator and no
acknowledgement.

*Amended 2026-09-10, as built:*

- **Setup is REST, not the GraphQL mutation sketched above.** Both first-run
  paths — `POST /authentication/setup/basic` and
  `POST /authentication/setup/oauth/{provider}/start` — carry
  `operator_acknowledgement: { terms_version_id }` as a **required** field, so a
  request without it does not parse. The local path checks the version against
  the archive (the operator document's, not the sharing terms') and records the
  attestation inside the transaction that creates the administrator. The OAuth
  path checks it before the provider round trip, stores it on the bootstrap
  session, and at the callback checks it again and records it in one
  transaction with the administrator and their provider link — which that path
  did not previously have.
- **One acknowledgement per version**, not one ever (migration
  `2026-09-10-120000-0000_operator_acknowledgement_per_version`). The same words
  twice is still refused; changed words can be acknowledged. The `down.sql`
  refuses to revert rather than delete an acknowledgement.
- **Re-acknowledgement (FR-044)** is an admin query and mutation:

  ```graphql
  type OperatorAcknowledgementState {
    "The most recent acknowledgement, of any version, in the words it was made to."
    acknowledgement: Attestation
    "The operator statement this build ships."
    currentVersionId: String!
    "False after an upgrade that changed the words, until acknowledged."
    isCurrent: Boolean!
  }

  extend type Query {
    instanceOperatorAcknowledgement: OperatorAcknowledgementState!
  }

  extend type Mutation {
    "Only the version this build ships is accepted."
    acknowledgeOperatorStatement(attestation: AttestationInput!): OperatorAcknowledgementState!
  }
  ```

  The administrator's landing page shows the words and asks whenever
  `isCurrent` is false — including on an instance set up before
  acknowledgement existed.

## Rules

1. **The statement is versioned like everything else** (FR-044) —
   `include_str!` of `legal/operator-responsibilities.md`, hashed, archived in
   `terms_versions` at startup. When an upgrade changes the words, the version
   changes, and the next administrative action shows what changed and asks for
   a new acknowledgement rather than applying it silently.
2. **It stays readable inside the running instance** (FR-045). `operatorStatement`
   answers without a session and the legal pages render it, so an operator
   never has to find a repository to re-read what they took on.
3. **The product says whose instance this is** (FR-046, FR-048). Every page that
   answers "who is responsible for this" names the instance's operator and how
   to reach them, and nothing anywhere implies the project can act on content in
   an instance it does not run. Somebody looking for a takedown on somebody
   else's instance is pointed at that instance's operator, because that is the
   only party with a remedy.
4. **Registering a designated agent is the operator's own act** (FR-055). The
   statement says so. Collecting a contact field must not read as having filed
   anything on anybody's behalf.

## The notice contact, and the rule 039 owns

```rust
// src/server/src/publishing.rs
pub fn require_notice_contact(state: &AppState) -> GraphQLResult<()>;
```

Called first by `require_attestation`, so **every** publishing path is covered
by the same call (see `publishing-gate.md`).

- An instance with no notice contact **refuses every operation that publishes
  content beyond a world**, and tells its administrator exactly what is missing
  (FR-053, FR-047).
- **Playing privately stays possible.** An instance with nobody to notify is a
  private instance, not a broken one. Nothing here refuses a login, a world, a
  scene, a roll or an invite.
- Until spec 040 owns the value it is read from `THUNDERFORGE_NOTICE_CONTACT`,
  with the parse-or-default shape the moderation values already use. Spec 040
  replaces the reader without touching the rule.
- **The harness must seed one.** With FR-053 enforced, a seeded instance with no
  notice contact refuses every existing share test in
  `apps/web/e2e/`. `src/server/seeds/` sets it, and that is a task rather than a
  debugging session.
- The contact is **discoverable without an account** (FR-056), from the DMCA
  page, which already answers anonymously.

## What belongs to spec 040 and is only referenced here

FR-050, FR-051, FR-052, FR-054 and FR-057 — collecting the operator identity and
notice contact at setup, storing them, rendering them into the `[OPERATOR]`
placeholders, recording a change to them, and refusing a blank or placeholder
value. Spec 040's FR-001 to FR-009 own all five.

**One thing worth flagging to whoever writes 040's plan**: the `[OPERATOR]`
markers are not one kind of thing. Some are **substitutable values** —
`terms-of-service.md:19` "name and, if applicable, legal entity",
`terms-of-service.md:22` "email address", and the two equivalents in
`privacy-policy.md`. The rest are **authoring prompts to a human**:
`terms-of-service.md:88` "this section and the next are the ones most likely to
need changing", `:119` "say how you will tell people these terms changed",
`:124` "name the jurisdiction whose law governs". Substituting a stored value
into the second kind produces nonsense. FR-052 is satisfiable for the first kind
and is a review item for the second.

## What is deliberately absent

- **No enforcement against a self-hosted operator.** The project has no access
  to somebody else's instance, no ability to take anything down, and no standing
  to try. The honest response is to say so plainly at the moment somebody
  becomes an operator, and this contract does not pretend a mechanism exists
  (FR-048).
- **No reuse of `instance_identity`.** It holds one UUID for spec 034's binding
  records and has a documented hole about database copies. Operator identity is
  a different thing with a different lifetime and different consequences when
  wrong; the checklist already said so.
- **No blocking of boot.** An instance with no notice contact boots, runs and is
  played on. It publishes nothing. That is the softest enforcement that is not
  merely advisory.
