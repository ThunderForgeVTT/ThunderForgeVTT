# Measurements — 049/050, importing a source book

Evidence for the arc, in the shape spec 043's measurements took: where the
corpus came from, what was measured, what was found, and what would reopen it.
Numbers here are **generated, not transcribed** — each section names the command
that produced it.

---

## Baseline, before any of this arc was built (T001)

**Date**: 2026-09-12 · **Commit**: `9f5537e`

**Corpus**: 246 PDFs, the owner's own library. Not in the repository and never
will be — these are commercial books. Only counts are reported; the tooling is
built that way on purpose (`survey.rs`: *"a library of commercial books is not
something to page through in a terminal"*).

```bash
find "<corpus>" -iname '*.pdf' -type f -print0 | sort -z \
  | xargs -0 cargo run --release -p dnd5e-server --example harvest --
```

| Measure | Value |
|---|---|
| Books | 246 |
| Books that failed to open | **0** |
| Creatures found | **2155** |
| Attacks with a stated reach | **1880** |
| Names flagged uncertain | 153 |

### Reading this honestly

**This supersedes the "717 creatures, 747 reaches" figure carried in
`TOMORROW.md` and in spec 049's prose.** That number was real but was measured
over a hand-picked subset — the bestiaries alone. Re-running it today over the
same single directory (`D&D 5e/Bestiary/`, 8 books) gives **451 creatures, 538
reaches**, so the old figure's book set is not recoverable and is not worth
recovering. The whole corpus is the baseline from here, because it is the only
set that can be specified exactly and re-run by anybody.

**2155 is not 2155 correct creatures.** The samples printed alongside include
obvious misparses — a fragment of sentence read as a creature name, for
instance. The number is a *floor on recall* and an upper bound on precision,
and precision is what T024's hand count and the per-field certainty work are
for. Anybody comparing a later run to this one is comparing the same flawed
measure to itself, which is what makes it useful as a regression signal and
useless as a quality claim.

**What would reopen it**: a change to `layout.rs` (the column and
reading-order work moved these numbers by a factor of two once already), a
change to the anchor strategy, or a change to the corpus.

---

## Still to run

- **Corpus read, per kind** (049 FR-060, T023-T025) — phase 4
- **Entry identity across a re-parse** (050 FR-029, T067-T070) — phase 10, a
  gate on the delta model
- **Storage** (050 FR-071, T064) — phase 9
- **Delta resolution latency** (050 FR-072, T076) — phase 11, which fixes
  SC-004's margin rather than guessing it
