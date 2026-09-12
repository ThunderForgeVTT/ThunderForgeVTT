# Research: Importing a Source Book

**Feature**: 049 | **Date**: 2026-09-12

Phase 0. Every open question in the plan's Technical Context, and the decisions
taken against them. Nothing here re-opens the spec's owner decisions — those are
settled and recorded in `spec.md`.

---

## 1. One arc, not two releases (owner, 2026-09-12)

**Decision: 049 and 050 are planned and built as a single arc.** The owner was
given the sequencing choice explicitly and took the merge, with its cost
stated: one large change set, and 050's partly-legal gate sitting in the same
body of work as 049's purely-engineering one.

**What this settles.** FR-042 — browse by compendium *in the world's Compendium
portal* — is satisfiable as written, because the book list that makes a
compendium reachable from a world is in scope. No requirement moves between
specs and neither spec is amended.

**What it costs, and how the cost is contained.** The named risk was that the
ADR-069 re-determination needed for sync-back would block the parser. That risk
is real but it is a **sequencing** problem, not a structural one, so the phase
order contains it:

- **Every gate is placed as late as it can honestly go.** The reader, the
  review, the send, the shelf and the book list — Arcs A to C — depend on
  neither gate and ship without them.
- **Two gates, both in the last third.** The entry-identity measurement (spec
  050 FR-029) precedes the delta model; the ADR-069 re-determination precedes
  sync-back, which is the final build phase.
- If the determination is refused or delayed, **everything up to and including
  deltas still ships**. What is lost is pushing a world's improvement back to
  the shelf — a real feature, and not one that anything else depends on.

**The rejected alternatives, kept because the reasoning still applies.**

- *049 alone, account-level browsing only, FR-042's world half deferred* — the
  recommendation before the owner chose otherwise. It kept each spec's proof
  obligation separate and cost one release in which a Game Master could import
  the Monster Manual and not put a goblin on a map. Superseded.
- *049 plus a crude all-or-nothing world use of the library* — rejected then
  and still rejected now, and the merge removes the temptation entirely. It
  would have had content reaching a world before the delta model defined where
  a per-world edit is stored, so an edit would have had nowhere defined to
  live.

**Consequence for the build.** Because both specs are in scope from the first
migration, there is no "account-owned for now" and no retrofit to plan around:
the compendium is account-owned, the world's link to it and the delta table
land in the same arc, and the shapes are designed together rather than one
being migrated to fit the other.

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

## 7. Where the compendium lives

**Decision: account-owned from the first migration, with the world's link to it
and the world's delta over it landing in the same arc.**

The compendium belongs to the importing account, not to a world (049 FR-040).
Under the merge that is no longer a shape that has to survive a later
migration — the world link (`book list`) and the delta table are designed
alongside it rather than fitted to it afterwards, which is the one clear
benefit the owner's choice buys.

Origin is recorded by the write path rather than supplied by the caller (049
FR-051, FR-057). A field the client can set is a field an attacker can set, and
this particular field is the one every sharing rule is enforced against.

The uploaded PDF is never stored, so it is never modelled.

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

## 9. The DMCA guardrail is engaged, and where exactly

**Finding: under the merge this arc does engage the constitution's guardrail —
in exactly one phase — and the rest of the arc is clear of it.**

ADR-069 determined that a link-shared collection is not a "centralized public
repository" under spec 015's policy, with one accepted risk. Its own "what this
determination does not cover" section names:

> **Versioned collections or any update path to already-copied content.** Spec
> 026 places these out of scope; an update path is a genuinely new distribution
> model and re-opens this determination.

Spec 050 FR-104 gives a collection versions and an update path from a world.
On that plain wording, **sync-back is the case ADR-069 says it does not
cover.**

**What is clear, and why that matters.** Everything in this arc except
sync-back involves content that is structurally incapable of leaving the
account that imported it. 049 FR-054a makes uploaded content unable to enter
the shared-collection path ADR-069 reasoned about at all, which **narrows**
what that determination has to carry rather than widening it. The reader, the
review, the send, the shelf, the book list and the deltas are all clear.

**What the guardrail requires before that one phase begins**, in the
constitution's own terms:

1. **(a) The notice-and-takedown program is operational.** It was confirmed so
   for ADR-069, and ADR-079 later extended a takedown's reach to adopted
   copies. This needs **re-confirming as still true**, not re-establishing.
2. **(b) An explicit, on-record determination** of whether a collection with an
   update path constitutes a centralized public repository — as an **amendment
   to ADR-069**, since that is the determination being re-opened.
3. **Signed by the accountable owner, not the implementer.** ADR-069 and
   ADR-079 were both owner-signed precisely because they are about liability.

**The question that determination has to answer**, stated so it is not
discovered late: spec 026 says an adopted copy is independent, so a sync-back
plausibly does **not** reach copies other people took — in which case the
"update path to already-copied content" limb is not triggered and only the
"versioned collections" limb is. That is a reasonable reading and it is not
mine to assert. It goes on the record or it does not.

**Consequence for the phase order**: sync-back is the last build phase, so a
refusal or a delay costs that feature and nothing before it (research §1).

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
