# ADR-076: The Sharing Attestation Record

**Date:** 2026-09-09
**Status:** ACCEPTED
**Participants:** ThunderForgeVTT Team
**Related:** spec 039 US1–US4, ADR-092 (operator values in legal prose),
spec 026 FR-026

> **Numbering.** Spec 039's plan reserved 080–083 and its task list said
> "ADR-093"; both were taken by specs 040 and 041 while this one waited.
> 076–080 were free — a gap left when 037's reserved block landed as 084–087 —
> so this feature takes 076–079. The decisions are what matter.

---

## Problem Statement

`legal/collection-sharing-terms.md` is shown at the collection share step, and
the moment passes. Nothing records who agreed, when, or **to which words**.

That is the whole problem. The position this project takes — the person who
uploads and publishes content is its accountable owner, and the instance is an
intermediary — is only as good as the evidence behind it. If a notice arrives
in eighteen months, "they clicked something" is not an answer. "This person, at
this time, agreed to this exact text" is.

So three questions have to be answered together, because answering them
separately is how the answers stop agreeing:

1. Where do the terms live, such that the server can require agreement to them?
2. What identifies a *version* of them?
3. What is written down, and how long does it outlive everything around it?

---

## Decision

**The terms are compiled into the server, identified by the hash of their own
words, archived at startup, and attested to in the transaction that publishes.**

### The prose is `include_str!`, not a database row and not a fetch

`legal/sharing-terms.md` is compiled in, on the precedent `admin.rs` already
set for the realm defaults. The text a person agreed to and the binary that
accepted the agreement ship together, so there is no configuration under which
the server enforces agreement to words it does not have.

The web app renders the *server's* copy rather than its own Vite glob. Two
copies of the terms is one copy too many the moment they disagree, and the copy
that matters is the one the gate checks against.

### A version is the hash of the normalised body

`version_id` is `<slug>@<first 16 hex of sha256(normalised body)>`.
Normalisation strips the leading HTML comment and trims — exactly what
`legalDocuments.ts`'s `sectionsOf` already does before rendering.

The consequence is the point: **editing a sentence mints a version; editing the
explanatory comment does not.** Nobody has to remember to bump anything, and
nobody *can* change the words without changing the identity. A hand-maintained
version number is a number that is wrong the first time somebody fixes a typo
without thinking about it.

Sixteen hex characters is 64 bits. This is not a collision-resistance problem —
an attacker who could forge a version id gains the ability to name a document
that already exists — it is a legibility problem, and a full 64 characters in
every log line and every error message buys nothing.

### The archive is written at startup, before anything can name a version

`ensure_terms_versions_recorded(state)` runs from `main.rs` beside
`ensure_admin_bootstrap_code` and `ensure_instance_identity`, and inserts
if-absent.

FR-016 — "an attestation always resolves to the words agreed to" — is a
property of that ordering plus one sentence: **nothing is ever updated or
deleted in `terms_versions`.** A version an attestation could name was archived
before the server that would accept the attestation was serving. There is no
window.

`attestations.terms_version_id` is a foreign key to that archive, and it is the
**only** foreign key this feature adds. It has to be one: an attestation naming
words that are gone is precisely the failure the archive exists to prevent, and
because the archive is append-only the constraint can never block a write.

### The record is written in the transaction that mints the share

Not after it, not from a background task. There is no interleaving in which a
share link exists without its attestation, and none in which an attestation
exists for a share that failed.

`share_id` is a plain uuid with **no** foreign key, so revoking or deleting the
share leaves the record standing (FR-007). It is a pointer, not a dependency —
the same reason `content_moderation_actions` points at content without
constraining it.

### What survives an account being deleted

`subject_username` becomes NULL. `subject_user_id` is kept as an opaque id that
now resolves to nothing. Everything else is unchanged.

What remains is: *somebody, identified only by an id no longer joinable to a
person, agreed to this exact text at this exact time in order to publish this
exact thing.* That is what a notice arriving in eighteen months needs, and it
names nobody. Deleting the row instead would mean an account deletion could
destroy the evidence for a claim already filed against it.

---

## Alternatives Considered

**Terms in the database, editable by an administrator.** Rejected. It makes the
words a runtime value, which means an operator can change what people have
agreed to without a deploy, and it makes "which words were these?" a question
about history rather than about content. The archive would then need to be the
source of truth rather than a cache of it, and every read becomes a join
against a mutable table.

**A hand-maintained version number in the document's front matter.** Rejected:
it is wrong the first time somebody fixes a typo without thinking about it, and
the failure is silent — an attestation pointing at a version whose text has
changed underneath it, which is exactly the thing being ruled out.

**Recording the whole body on every attestation** rather than a version id.
Correct, and wasteful in a way that gets worse forever: a few kilobytes per
share of text that is identical across millions of rows. The archive plus a
version id says the same thing once.

**Writing the attestation after the share succeeds.** Simpler, and it admits
the one state that must not exist: a published thing with no agreement behind
it. One transaction or nothing.

**A foreign key from `attestations` to `users`.** Rejected on the precedent
`content_moderation_actions` set: a record that must outlive its subject cannot
be constrained by its subject's existence. Account deletion would have to
choose between failing and cascading, and both are wrong.

---

## Consequences

**Good**

- An agreement is evidence, not a moment.
- The words cannot drift from their identity, because the identity *is* the
  words.
- The server requires the agreement (ADR's sibling decision, in
  `contracts/publishing-gate.md`), so a direct API call cannot skip it — a
  policy enforced only in a page is a policy with a hole in it.
- One record serves the sharing attestation and the operator acknowledgement,
  which is the only way FR-043's "on the same terms" stays true.

**Costs**

- Editing `legal/sharing-terms.md` requires a deploy to take effect. Correct: a
  change to what people are agreeing to is a change to the product.
- Every prose edit mints a version, including a trivial one. Cheap — a row —
  and the alternative is somebody deciding which edits "count", which is the
  judgement this design removes.
- The web app can no longer render the terms offline from its own bundle. It
  reads them from the server, which is where the authoritative copy is.
