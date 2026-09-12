# Quickstart: Proving a Book Import

**Feature**: 049 | **Date**: 2026-09-12

How to convince yourself each phase works. Not a test plan — the tests are in
`tasks.md`; this is what you run and what you should see.

## Prerequisites

- A working dev stack (`pnpm dev`).
- **A real book.** Not a fixture. Every defect found while building the reader
  produced text that looked plausible and was wrong on a real document and
  right on a synthetic one. The e2e suite builds its own minimal PDFs by hand
  because a copyrighted binary fixture tells nobody what it contains; those
  prove the *plumbing*. They do not prove the reading, and nothing but a real
  book does.
- For Phase 8, the 246-book corpus.

## Per-phase verification

Every phase, before it is called done (Principle V):

```bash
cargo check -p thunderforge-server -p thunderforge-canvas-core -p pack_system_spec
pnpm -F @thunderforge/web exec tsc --noEmit   # pnpm verify does NOT do this
pnpm verify
```

---

## Phase 1 — a system declares its content patterns

```bash
cargo test -p pack_system_spec
node scripts/check-system-registry.mjs
```

The registry check must pass **with no new entry in its `KNOWN` map**, which is
currently empty. If it cannot, the design is wrong and the phase stops — the
check does not get widened.

Then confirm the loader reads what the pack declares and falls back cleanly:
point it at a system with no `contentPatterns` and get a refusal with a reason,
not a crash and not an empty success.

## Phase 2 — the anchored reader, and the creature reader through it

```bash
cargo run -p dnd5e-server --example harvest -- <a-real-bestiary.pdf>
```

The existing example is the baseline: it reported **717 creatures and 747
per-attack reach values** across six bestiaries. After this phase the same
books must still yield creatures through the *generic* reader, and reach must
still come out — via the pack's contribution, since the declaration cannot
express it (contract: content-patterns, "what the declaration deliberately
cannot express").

A drop in either number is a regression, not a rounding difference. Nothing in
production calls `statblocks()` today, so there is nothing else to break.

## Phase 3 — the prose reader

Read a book with class features in it and confirm each entry has a name, its
text, and a page — and **no mechanical fields at all**. An entry that acquired
a damage value or a range is the failure this phase exists to prevent; prose
has no such values to find, so any that appear were invented.

## Phase 4 — the confirmation window

```bash
node scripts/e2e-parallel.mjs --shards=1 --only=book-import-review
```

with `THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT=1`.

The test watches the network and fails if any request body carries entry text
before submit is pressed. Then, by hand: open the review with a real book,
confirm counts per kind, confirm uncertain fields look different from read
ones, close it, and confirm the library is still empty.

## Phase 5 — sending

```bash
node scripts/e2e-parallel.mjs --shards=1 --only=book-import-commit
```

Induce a failure partway and confirm the account holds nothing from that book —
no half compendium, no orphan entries. Then submit a large book and watch
progress advance; it should move because bytes are moving, not because a timer
is running.

## Phase 6 — the compendium and the library

By hand: import two different books, confirm two compendiums with their own
counts and dates. Import one of them again and confirm you are told you already
have it — **and confirm the check works from a different world**, which is the
whole point of hashing against the account's library rather than one world's.

Then request removal of one and confirm you are told what is in use before
anything happens, and that confirming it leaves the other untouched.

## Phase 7 — origin

```bash
node scripts/e2e-parallel.mjs --shards=1 --only=content-origin
```

Every route out — share, publish, export, adopt into a collection — refused for
uploaded content, with a reason naming its origin. The same routes must still
work for content authored by hand, because the rule restricts an origin and not
a subject.

**This phase is gated on ADR-097 being signed by the accountable owner**, not
by the implementer. Precedent: ADR-069 and ADR-079 were both owner-signed
because they are about liability. Do not start Phase 7 against an unsigned ADR.

## Phase 8 — the book list

```bash
node scripts/e2e-parallel.mjs --shards=1 --only=library-book-list
```

Switch one book on in two worlds and confirm **stored content did not grow** —
that is the claim the whole architecture rests on, and it is the one that
quietly stops being true. Switch it off and confirm nothing is left behind.

Then open a world as a player and confirm the list is visible and unchangeable.
And open the world's Compendium portal and browse by compendium — that is 049
FR-042, satisfied here rather than deferred, which is what merging the two
specs bought.

## Phase 9 — the entry-identity measurement *(a gate)*

```bash
cargo run -p thunderforge-pdf --example survey -- --identity <corpus-dir>
```

Re-parse books that exist in more than one file — a re-save, a later printing,
or the same file under an improved parser — and report how many entries keep a
stable kind-and-name identity, how many are renamed, how many collide.

**This gates Phase 10.** If the evidence contradicts kind-plus-name, spec 050
FR-025 changes *before* the delta model is built. A rule invented at a desk for
re-attaching somebody's month of work to a re-parsed book is exactly the guess
this project has been burned by. Do not start Phase 10 on an unrun measurement.

## Phase 10 — deltas

```bash
node scripts/e2e-parallel.mjs --shards=1 --only=library-deltas
```

Change an entry in one world; confirm the other world is unchanged **and the
base is byte-identical to what the import produced**. Add a world-only entry
and confirm it is absent elsewhere. Hide one and confirm it is still there in
the rest.

Then check the origin split by hand: the changed entry cannot be shared, the
added one can. Same screen, same feature, different origin — this is the
subtlety most likely to be got wrong.

## Phase 11 — re-import under deltas

Import a book, edit entries in two worlds, re-import a changed version. The
deltas still apply. Any that cannot re-attach are **reported by name**, not
dropped. Induce a failure partway and confirm the previous base is still in
place and no delta was touched.

## Phase 12 — collections on the shelf

Author a collection by hand, confirm it sits on the same shelf as an imported
compendium and behaves the same in a world. Download it as JSON and open it in
something that is not ThunderForge. Then try to download an imported compendium
and confirm you cannot.

## Phase 13 — sync-back *(gated)*

**Do not begin this phase until ADR-098 is signed by the accountable owner and
the takedown programme has been re-confirmed operational.** Both are required
by the constitution's guardrail before implementation starts, not before it
ships. Research §9 states the question the determination must answer.

Then: change an entry in a world over an authored **collection**, sync it back,
confirm the shelf has it and a new world gets it, and confirm the world no
longer holds it as a delta. Confirm the previous version is recoverable.

Finally, the asymmetry that is the whole legal position — attempt the same over
an imported **compendium** and confirm there is no route, at any volume of
change, and that the reason is stated where the absence would be noticed.

---

## The measurements

Two runs land in `measurements.md`, in the shape spec 043's measurements took —
corpus provenance, the verdict rule, results, threats to validity, and what
would reopen it — with numbers in generated JSON carrying `generatedAt` and
`generatedBy`, so nothing is transcribed by hand.

- **The corpus read** (049 FR-060): per book, whether it opened, how many pages
  were silent, how many entries of each kind were found. **049 SC-002's 90%
  spell recall is a target, not an observation** — this run either meets it or
  replaces it. Replacing it is a success; the 717-creature figure was arrived
  at the same way.
- **Entry identity** (050 FR-029, Phase 9) and **storage** (050 FR-071): the
  saving measured against a realistic library, and the read-latency margin that
  050 SC-004 and FR-072 deliberately left for measurement to fix rather than
  guessing a number.

---

## What "done" means for the arc

- All thirteen phases verified as above.
- `pnpm verify` green, and `tsc --noEmit` green for the web app separately.
- ADR-096 with Phase 1; **ADR-097 owner-signed** before Phase 7; **ADR-098
  owner-signed, amending ADR-069**, before Phase 13.
- `measurements.md` with real numbers, and any Success Criterion the runs
  contradict corrected in the spec rather than left aspirational.
- A Game Master can read a book into their library, switch it on in whichever
  worlds they choose, change it in one without changing another, and cannot
  share any of it.
