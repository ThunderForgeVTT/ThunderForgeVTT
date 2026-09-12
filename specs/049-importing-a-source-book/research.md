# Research: Importing a Source Book

**Feature**: 049 | **Date**: 2026-09-12

Phase 0. Every open question in the plan's Technical Context, and the decisions
taken against them. Nothing here re-opens the spec's owner decisions — those are
settled and recorded in `spec.md`.

---

## 1. Does 049 ship before 050, and is it worth anything on its own?

**Decision: yes, and yes — option (a).** 049 lands the reader, the review, the
send, and an **account-owned compendium the Game Master can browse in their own
library**. Worlds cannot switch a book on until 050; that is 050's feature and
it is not being half-built here.

**Rationale.** The alternative that looked tempting — give worlds a crude
all-or-nothing use of the library so 049 is end-to-end — buys a demo and costs
a retrofit. 050's book list is per-world selection with a read-only player
view, and a world that has already been taught "you get everything in the
library" has to be untaught. Worse, the delta model (050 FR-020 to FR-029)
decides how a world's change is *stored*; content that reached a world by some
other route before that exists is content with no defined place to put an edit.

The honest cost, stated rather than hidden: **after 049 alone, a Game Master
can import the Monster Manual and look at it, and cannot yet put a goblin on a
map.** That is a real gap and it is one release long. It is acceptable because
the thing 049 proves is the thing that is hard and risky — that a book can be
read correctly, reviewed before it is trusted, and stored with an origin that
cannot be edited off. Putting a goblin on a map is not the risky part.

**Alternatives considered.**

- *(b) A crude world-wide library* — rejected above: a retrofit, and it
  pre-empts the delta model's storage decision.
- *(c) Plan 049 and 050 as one arc* — rejected. It produces one very large
  change set, and it merges two specs whose proof obligations are different:
  049's is "the reader is right about a real book", 050's is "nothing is copied
  and nothing crosses an account". Failing to prove one should not block the
  other.

**Consequence for sequencing.** 049's Phase 6 must build the compendium as
**account-owned from the first migration**, never world-owned "for now". A
world-scoped table that 050 has to migrate is exactly the retrofit this
decision exists to avoid.

**Consequence for the spec, owed plainly.** FR-042 says *"the world's
Compendium portal MUST let a Game Master browse by compendium, beside the
existing browse by kind."* Under this decision 049 cannot satisfy it, because a
world cannot reach a compendium until 050 gives it the book list. So:

- **049 delivers account-level browsing** — the library view, which is what
  US3's "browse what came out of one book" actually needs and what the phase
  table's Phase 6 builds.
- **FR-042's world-portal integration moves to spec 050**, alongside the book
  list it depends on.

That surface already exists and is substantial — `apps/web/src/pages/world/
compendium/` with tabs for NPCs, items, abilities and lore, reached at
`/world/:id/compendium`. Adding a "by compendium" dimension to it is a real
piece of work on a real page, and it belongs with the feature that makes a
compendium reachable from a world at all. 049's library view is a **new,
account-level surface**, not a change to that page.

---

## 2. Two kinds of content, and why the split is structural

**Decision: the importer reads two kinds and never one as the other.**
*Anchored* content — spells, items, creatures — is a block of labelled fields
introduced by an unambiguous anchor, and its fields are read into structured
values. *Prose* content — class features, feats, subclass options — is captured
as a name, its text, and where it came from, with no mechanical fields at all.

**Rationale — and it is evidence, not taste.** The existing creature reader
works for exactly one reason: armour class is an anchor. Every 5e creature has
one, it is always labelled, and the label appears nowhere else in a statblock.
That gives 717 creatures out of six bestiaries. Spells and magic items have the
same shape with different labels (`Casting Time:`, `Range:`, `Components:`).

Class features do not. A feature is a bold run-in name followed by paragraphs,
and there is no label, no fixed field, and no reliable boundary but the next
bold name. Any attempt to extract "the mechanics" from that is invention, and
invention is the failure mode this project has been bitten by repeatedly —
every parser defect found so far produced text that looked plausible and was
wrong.

**Alternatives considered.**

- *One reader with optional fields* — rejected. It makes "found nothing" and
  "there was nothing to find" the same value, which is precisely what FR-002
  forbids.
- *Try to extract mechanics from prose with a heuristic, marked uncertain* —
  rejected. Uncertainty marking is for a value that was read and might be
  wrong, not for a value that was never there. Marking a fabrication as
  uncertain launders it.

---

## 3. Where a content-pattern declaration lives, and in how many places

**Decision: three places, mirroring `vision` exactly — schema, runtime type,
loader — because that is what the existing extension point does and a second
shape would be a second thing to learn.**

| Where | What it is | The `vision` precedent to copy |
|---|---|---|
| `crates/pack_system_spec/src/lib.rs` | The manifest **schema** and its validation. | `SystemManifest.vision: Option<SystemVision>`, plus the extra rules in `validate_system_manifest`. |
| `crates/thunderforge-canvas-core/src/` | The **runtime type** the server reasons with. | `vision_declaration.rs` — `VisionDeclaration`, `ResolvedVision`. |
| `src/server/src/` | The **loader**: read this system's `system.json`, deserialize one key, fall back to a default when absent or malformed. | `vision_profiles.rs::vision_declaration_for_system`. |

The schema crate deliberately **duplicates** the runtime type rather than
importing it, and says so in its own doc comment — it is the manifest's schema,
not the engine's model, and the two are allowed to drift under validation. That
is a deliberate existing decision and this feature follows it rather than
tidying it.

**One thing not to copy.** There is no cached manifest loader: the pattern is a
file read per call, duplicated across `vision_profiles.rs`,
`declared_values.rs`, `attributes.rs`, `sheet.rs`, `turn_structure.rs` and
`ability_vocabulary.rs`. Importing a book will read the manifest once per
import, not once per entry, so this feature does not make it worse and does not
fix it. Naming it so the next person does not discover it as a surprise.

**Alternatives considered.**

- *A single shared type imported by both crates* — rejected. It is a live
  decision recorded in `pack_system_spec`'s own comments, and reversing it is a
  separate change with its own justification.
- *Declare patterns in the pack's Rust crate via `inventory` instead of the
  manifest* — rejected. FR-011 says the manifest, beside `vision`; and a
  declaration in Rust cannot be read by the browser, which is where the reading
  happens.

---

## 4. How the existing creature reader is reached through the declaration

**Decision: the declaration carries anchor, fields and a name rule, and the
generic anchored reader replaces `statblock.rs`'s general machinery. What the
declaration cannot express is named here rather than quietly preserved
(FR-016).**

The existing reader is not wired to anything: `statblocks()` is called only by
`packs/systems/dnd5e/server/examples/harvest.rs` and its own tests — there is
**no production caller**. That is unusually good news. Wiring the declaration
up is additive, and nothing regresses if the generic reader supersedes it,
because nothing depends on it today.

What `statblock.rs` does that a declaration must be able to express:

- **Anchor on a label** — `Armor Class`. Directly expressible.
- **Read labelled numeric and string fields** — hit points, hit dice, speed,
  challenge. Directly expressible.
- **Find the name by walking backwards** up to six lines from the anchor, using
  size and boldness. Expressible as a *name rule*, which is why the declaration
  has one rather than assuming the name precedes the anchor immediately.
- **Per-field certainty** — `Confidence { name, armor_class, hit_points }`.
  Becomes the general per-field certainty the data model already requires; not
  system-specific at all.

What it does that a declaration **cannot** express today, stated plainly:

- **Per-attack reach parsing.** `Action { name, text, reach_feet }` extracts a
  reach in feet out of an attack's prose. That is not a labelled field; it is a
  system-specific reading of free text, and it is load-bearing — spec 045's
  playtest established reach is per-attack, not per creature size.

**Decision on that one thing: it stays in the pack, as a pack-supplied
refinement over the generic result, not in shared code and not pretended into
the declaration.** The mechanism already exists — `SystemContribution` in
`crates/thunderforge-canvas-core/src/system_contribution.rs` collects `fn`
pointers per system via `inventory`, which is exactly how a pack contributes
behaviour that is not data. Shared code still names no system; the 5e pack
still owns the one thing only 5e knows.

**Alternatives considered.**

- *Express reach in the declaration as a regular expression* — rejected. It
  moves system-specific knowledge into a config string, where it is harder to
  test and still system-specific; and a pattern language rich enough for this
  is a language, which is a much larger thing to own.
- *Leave `statblock.rs` as a parallel path for creatures only* — rejected by
  FR-016, and rightly: two readers disagree eventually, and the disagreement
  shows up as a creature that imports differently depending on which path ran.

---
## 5. Proving "nothing was sent before submit"

**Decision: prove it by observing the network in an end-to-end test, not by
reading the code** (FR-061).

**Rationale.** FR-020 and FR-024 are the spec's central safety promise and they
are a promise about absence. Absence is not provable by inspection: a future
telemetry call, an analytics beacon, or an eager pre-flight added by somebody
who did not read this spec would all satisfy a code review and break the
promise. A test that fails when any request carries book content is the only
form of this claim that stays true.

**Alternatives considered.**

- *Unit-test the submit handler* — rejected. It proves the handler does not
  send; it proves nothing about the rest of the page.
- *Trust the architecture, since parsing is local* — rejected for the same
  reason FR-065 exists. Every defect so far was in something believed obvious.

---

## 6. All-or-nothing, and where the boundary sits

**Decision: the server applies an import inside one transaction, and the
progress a person sees is progress of upload, not of commit** (FR-030 to
FR-032).

**Rationale.** FR-032 says a failure partway leaves the world exactly as it
was. If the commit were streamed and applied incrementally, "partway" would be
a state a Game Master could be left holding, and the only repair would be a
compensating delete that has to be as correct as the import was. One
transaction makes the failure case free.

That does mean a large book is a large transaction, and the plan must bound it:
FR-035 already requires a stated entry bound, refused before sending. The bound
is what keeps the transaction sane, and it belongs in the contract rather than
being discovered at the database.

**Alternatives considered.**

- *Stream and apply per chunk, with a compensating rollback* — rejected. It
  trades a bounded transaction for a rollback path that is exercised only on
  failure, which is the worst place to keep untested code.
- *Stage to a scratch table, then swap* — held in reserve. If the bound proves
  too small in measurement, this is the escape hatch; it is not needed to start.

---

## 7. Where the compendium lives before 050 exists

**Decision: an account-owned compendium and its entries, written by the import
and read by a library view, with origin recorded automatically and immutably.**

**Rationale.** Follows from decision 1 and from FR-040: the compendium belongs
to the importing account, not to a world. Building it account-owned from the
first migration is what makes 050 additive — 050 adds a link from a world and a
delta table, and changes nothing already written.

Origin is recorded by the write path rather than supplied by the caller
(FR-051, FR-057). A field the client can set is a field an attacker can set,
and this particular field is the one the sharing rules are enforced against.

---

## 8. Origin as an invariant, not a check

**Decision: a collection cannot contain uploaded content by construction
(FR-054a), enforced at the data boundary rather than at each route that might
lead there.**

**Rationale.** The spec's own words: an invariant rather than a check on the
routes known today. Sharing, publishing, export, collection adoption and
whatever is built next year are all routes; enumerating them is a game that is
lost the first time somebody adds one without reading spec 049. Enforcing where
the content is written is a smaller surface and it does not rot.

**This is the decision that needs an owner-signed ADR**, not an implementer's.
The precedent is explicit: ADR-079 was accepted by the accountable owner
"because it is about liability, so it was theirs to sign and not the
implementer's", and ADR-069 before it. This one is the same kind.

---

## 9. What ADR-069 already covers, and what it does not

**Finding: 049 does not re-open ADR-069's determination, and materially
strengthens one of its conditions. 050 may re-open it, and that is 050's
problem to solve before it is built.**

ADR-069 determined that a link-shared collection is not a "centralized public
repository" under spec 015's policy, with one accepted risk. Its "what this
determination does not cover" section names, among others:

> **Versioned collections or any update path to already-copied content.** Spec
> 026 places these out of scope; an update path is a genuinely new distribution
> model and re-opens this determination.

- **049 is clear of it.** Uploaded content never leaves the account by any
  route. There is no share, no adoption, no cross-world surface. FR-054a makes
  uploaded content structurally incapable of entering the shared-collection
  path that ADR-069 reasoned about, which narrows what that determination has
  to carry rather than widening it.
- **050 is not obviously clear of it.** Spec 050 FR-104 gives a collection
  versions and an update path from a world. On the plain wording above, that is
  the case ADR-069 says it does not cover. Whether adoption copies are reached
  by a sync-back — spec 026 says a copy is independent, so probably not — is
  exactly the question that has to be answered on the record rather than
  assumed.

**Consequence.** This is a further argument for decision 1's option (a): 049's
gate is engineering, 050's gate is partly legal, and coupling them would put
the legal gate in front of the reader. Flagged here so 050's planning starts
with it rather than discovering it.

---

## 10. What the measurement measures, and where it goes

**Decision: extend `crates/thunderforge-pdf/examples/survey.rs` rather than
write a second tool, and land the result as
`specs/049-importing-a-source-book/measurements.md` with generated JSON
beside it** (FR-060).

**Rationale.** `survey.rs` already walks a directory, opens every document, and
reports counts and short samples — and it is deliberately built to report
counts rather than content, because a library of commercial books is not
something to page through in a terminal. Per-kind entry counts are the same
shape of question.

The output format follows two existing precedents: spec 043's
`measurements.md` (corpus provenance, the verdict rule, results, threats to
validity, and what would reopen it), and the repository's generated-metrics
convention of a JSON carrying `generatedAt` and `generatedBy` so no number in
prose is transcribed by hand.

**What it must report per book**: whether it opened, how many pages were
silent, and how many entries of each kind were found. SC-002's 90% spell-recall
target is measured against a hand-counted sample of at least three books, and
the spec already says that number is a target rather than an observation — the
run either meets it or replaces it.

**Alternatives considered.**

- *A new bespoke measurement binary* — rejected. Two tools that walk the same
  corpus will disagree, and the disagreement will be discovered late.
- *Report into `marketing/`* — rejected. That directory is for numbers the
  project publishes about itself; this one is engineering evidence for a spec
  and belongs beside the spec.

---

## 11. There is no per-account authorization helper, and this feature needs one

**Finding, and the decision that follows: 049 introduces the account-ownership
check that does not currently exist, as a named helper beside the world one.**

The server has a clear, well-used helper for world scope —
`src/server/src/auth/world_membership.rs::require_world_member`, with
`actor_in_world` as the single place a stored role string becomes a `Role`, and
`is_dm_of_scene` written fail-closed. There is **no equivalent for account
scope**. Mutations that operate on a user's own rows compare
`authenticated_user(ctx)?.user_id` against the row's user column inline, or
gate on `admin_user(ctx)`.

Inline comparison is fine when there is one row and one column. A compendium is
not that: FR-044's removal, FR-047's hash check, the library view, and every
route Phase 7 has to refuse all ask the same question — *does this account own
this compendium?* — and asking it six times inline is six chances to write it
slightly differently, in the feature where getting it wrong means showing one
person another person's imported book.

**Decision: one helper, used everywhere, fail-closed, beside
`require_world_member` rather than somewhere new.** Principle III asks for
authorization at the data boundary; this is that boundary acquiring the check
it was missing.

**Note for Phase 7.** The invariant that a collection cannot contain uploaded
content (FR-054a) lands at `src/server/src/collections/` and the
`world_collections` table — a world-scoped thing being fed from an
account-scoped one. That crossing is exactly where the two helpers meet, and it
is the place to be most careful.
