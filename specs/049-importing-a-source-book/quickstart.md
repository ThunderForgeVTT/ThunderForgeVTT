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

## Phase 8 — the measurement

```bash
cargo run -p thunderforge-pdf --example survey -- <corpus-dir>
```

Extended to report entries per kind. Lands as `measurements.md` beside this
file, in the shape spec 043's measurements took — corpus provenance, the
verdict rule, results, threats to validity, and what would reopen it — with the
numbers in generated JSON carrying `generatedAt` and `generatedBy`, so nothing
is transcribed by hand.

**SC-002's 90% spell recall is a target, not an observation.** This run either
meets it or replaces it with what was actually found. Replacing it is a
success, not a failure — the 717-creature figure was arrived at the same way.

---

## What "done" means for the feature

- All eight phases verified as above.
- `pnpm verify` green, and `tsc --noEmit` green for the web app separately.
- ADR-096 landed with Phase 1; **ADR-097 signed by the owner** before Phase 7.
- `measurements.md` exists with real numbers, and any Success Criterion the run
  contradicts has been corrected in `spec.md` rather than left aspirational.
- A Game Master can import a book and browse it. They **cannot** yet put a
  goblin on a map — that is spec 050, and research §1 records why that gap is
  deliberate rather than unfinished.
