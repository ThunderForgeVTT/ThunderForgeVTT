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

## Phase 3: the generic reader, against the same corpus (T019)

**Date**: 2026-09-12 · **Same 246 books, same command shape**

```bash
find "<corpus>" -iname '*.pdf' -type f -print0 | sort -z \
  | xargs -0 cargo run --release -p thunderforge --example harvest -- dnd5e creature
```

| Measure | Baseline (5e-specific reader) | Phase 3 (declaration-driven) | Change |
|---|---|---|---|
| Creatures | 2155 | **2750** | +595 (+28%) |
| Attacks with a stated reach | 1880 | **3049** | +1169 (+62%) |
| Names flagged uncertain | 153 | 221 | +68 |

The reader that knows no game system finds **more** than the one written for
this one. That is the result FR-016 was betting on, and it is worth saying
plainly that it was not the expected outcome — the hope was parity.

### Where the increase comes from, as far as can be told

- The name search now stops at the previous anchor and breaks size ties toward
  the anchor, so entries that previously collapsed into their neighbour are
  read separately.
- Reach is read from the lines of the whole page the entry sits on, rather than
  from a block bounded by the old reader's section-heading state machine, which
  gave up at any heading it did not recognise.

### The honest caveat, unchanged from the baseline

**More is not automatically better.** The uncertain count rose with the total
(153 → 221), which is what finding more marginal entries looks like, and the
printed samples still include misparses. These counts are a **floor on recall**
and say nothing about precision. Precision is what T024's hand count and the
per-field certainty work are for, and until that runs nobody should quote 2750
as "2750 correct creatures".

**What would reopen it**: any change to `layout.rs`, to the anchor strategy, to
the 5e `contentPatterns` block, or to the corpus.

### A defect this measurement caught

The first full-corpus run of the new reader reported **0 reaches** and looked
otherwise healthy. A cargo *example* links its package's library, and
`src/app` is a binary-only package — so `system_packs.rs`, whose entire job is
forcing pack linkage, was not linked into the example, `contribution_for`
returned `None`, and no refinement ran.

It is the exact failure `system_packs.rs`'s own comment warns about, arriving
from a direction that comment does not cover, and it is invisible: the run
completes, prints per-book counts, and is simply missing a column. The example
now carries the same load-bearing `use … as _;` block with a comment saying
why.

---

## Still to run

- **Corpus read, per kind** (049 FR-060, T023-T025) — phase 4
- **Entry identity across a re-parse** (050 FR-029, T067-T070) — phase 10, a
  gate on the delta model
- **Storage** (050 FR-071, T064) — phase 9
- **Delta resolution latency** (050 FR-072, T076) — phase 11, which fixes
  SC-004's margin rather than guessing it
