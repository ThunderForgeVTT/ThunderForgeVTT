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

## Phase 4: the prose reader, per kind (T023-T025)

**Date**: 2026-09-12 · Same 246 books

### The design gap this measurement found

The prose reader was specified as "a name, then the paragraphs under it". Run
over real books, that description turned out to match **every section of every
book**:

| Kind | As first declared | Cause |
|---|---|---|
| magicItem | **72,974** | Returned every heading with prose under it — `Table of Contents`, `About` |
| feat | **72,974** | *Identical*, because the two declarations were indistinguishable |
| classFeature | **42,295** | Same, via bold rather than heading |

Two prose kinds declared as "heading, then prose" cannot be told apart from
each other or from ordinary book structure. This is not a threshold to tune;
the model was incomplete.

**The fix**: a prose pattern must declare `confirmedBy` — phrases, one of which
must appear near the name. Refused at install by `pack_system_spec`, and
refused again by the reader, because the failure mode is silent and enormous.

### After the discriminator

| Kind | Before | After | Uncertain before → after |
|---|---|---|---|
| spell *(anchored)* | 1228 | **1228** | 259 → 259 |
| magicItem | 72,974 | **2073** | 2935 → **63** |
| feat | 72,974 | **1674** | 2935 → **71** |
| classFeature | 42,295 | **not declared** | — |

The uncertain count collapsing by ~98% alongside the totals is the useful
signal: the entries thrown away were overwhelmingly the ones the reader was
already unsure of.

### Class features: zero, deliberately

5e declares no `classFeature` kind. A class feature is a **bold run-in name** —
`Rage. In battle, you…` — and `layout::Line::bold` is true only when every run
on the line is bold, so the line reads as ordinary prose. Declared as a heading
it matches everything. There is no marker line beneath it to confirm it by.

Catching run-in names needs sub-line run data, which the layout pass
deliberately collapses. Spec 049 FR-013 is amended (FR-013a) rather than the
code bent to it. **Zero is the honest answer; 42,295 wrong ones is not.**

### Spell recall: a cross-check, *not* the hand count SC-002 asks for

SC-002 wants 90% of spells found, measured against a hand count. A hand count
means a person reading the book, which this is not. What follows is an
independent structural check: the reader anchors on `Casting Time`, so counting
a *different* label in the same block estimates how many blocks exist.

| Book | Reader found | `Duration` lines | `Components` lines |
|---|---|---|---|
| Aldri's Lost Spellbook | 10 | 11 | 10 |
| Spells That Don't Suck | 150 | 168 | 150 |
| Codex of Cantrips Vol I | 0 | 0 | 0 |

- Against `Components` the reader is **exact** in both readable books.
- Against `Duration` it is 91% and 89% — straddling SC-002's target.
- The third book yields nothing by any measure, which is consistent with it
  being one of the 51 image scans rather than a miss.

**Verdict: SC-002 is plausible and not established.** Two measures disagree,
one sits either side of the bar, and neither is the hand count the criterion
names. It stays outstanding and needs a person. Recording it as "met" on this
evidence would be exactly the confident wrongness this spec exists to prevent.

---

## Phase 10: entry identity — the gate on the delta model (T067-T070)

**Date**: 2026-09-13 · 246 books, 71 of which yield creatures
**Command**: `cargo run --release -p thunderforge --example identity -- dnd5e creature <files>`

### The experiment the spec asked for cannot be run

Spec 050 FR-029 names it: re-parse books "that exist in more than one file — a
re-save, a later printing, or the same file under an improved parser". **The
corpus contains no such pair.** 246 books, no repeated content hash, no title
appearing twice. The experiment presupposes a property of the library that the
library does not have, and no amount of care makes it runnable.

Two of its three questions can still be answered honestly. The third cannot,
and is left unanswered rather than approximated.

### 1. Collisions — can kind-and-name identify an entry at all?

| Measure | Value |
|---|---|
| Creatures read | 2750 |
| **Sharing a kind and name with another entry in the same book** | **107 (3.9%)** |

Worst cases: *Limitless Monsters* 24, *Monster Manual* 22, *Ghosts of
Saltmarsh* 2. So roughly **one entry in twenty-six cannot be told from another
in its own book** by the rule spec 050 FR-025 uses to re-attach a delta.

### 2. Stability — does identity survive a genuine re-parse?

Not the spec's re-parse, but a better one: the browser reads a book in chunks
of 25 pages (`bookImport.ts`) and the same file read whole is a different
traversal of the same document. This exercises code that actually ships.

| Measure | Value |
|---|---|
| Identities found reading whole but not in chunks | 7 |
| Found only when read in chunks | 3 |
| **Stable across both readings** | **2740 of 2750 (99.6%)** |

**And all ten differences are misparses.** A hypothesis that they would fall on
chunk seams was tested and is wrong — they sit mid-chunk (p125 against a seam
at p101, p200 against a seam at p176). What they actually are:

```
lost   p125   hit: 14 (2d8 + 5) bludgeoning damage.
lost   p200   one target. hit: 8 (1d8 + 4) piercing damage.
gained p201   m edium humanoid (human). lawful neutral
```

Fragments of attack prose read as creature names. A false positive depends on
incidental surrounding context, so it appears or vanishes as the window moves;
a real creature does not. **Every real entry was stable.**

That is a second, unlooked-for result: it puts visible faces on the misparses
this file's precision caveat has been warning about since the baseline.

### 3. Renames — not measured

Needs two editions of one work. The corpus has none. Stated rather than
approximated, because a number invented here would be used.

### Verdict on FR-025

**Kind-and-name stands, with an amendment.** It is stable where it matters —
every real entry survived a genuine re-parse — and the alternatives are worse:
page number breaks on any reflow, and fuzzy matching introduces silent
wrongness into the one place where being wrong rewrites a Game Master's work.

But 3.9% is not zero, and a rule that cannot always identify an entry must not
guess. **Spec 050 FR-025 is amended**: where two entries in a compendium share
a kind and a name, a delta MUST NOT be attached to either. Ambiguity is
reported, not resolved.

**What would reopen it**: a corpus that does contain two editions of one work,
which would finally answer the rename question.

---

## Still to run

- **Corpus read, per kind** (049 FR-060, T023-T025) — phase 4
- **Entry identity across a re-parse** (050 FR-029, T067-T070) — phase 10, a
  gate on the delta model
- **Storage** (050 FR-071, T064) — phase 9
- **Delta resolution latency** (050 FR-072, T076) — phase 11, which fixes
  SC-004's margin rather than guessing it

---

## Phase 9: one book, eight worlds, measured in bytes (T064)

**Date**: 2026-09-13 · **Commit**: `05b515e`

```bash
cargo run -p thunderforge-server --features test-support \
    --example library_storage
```

A synthetic book of 1500 entries is read onto one account's shelf and
switched on in 8 worlds. Stored bytes are Postgres's own
`pg_column_size` over the rows each thing owns — the compendium and its
entries for the book, the `world_books` rows for the links — taken inside one
transaction that is rolled back, so the numbers come from a real database and
leave nothing in it.

| Measure | Bytes |
|---|---|
| One book (1500 entries stored) | **744232** |
| One world's link to it | **144** |
| That book in 8 worlds, inherited | **745384** |
| That book in 8 worlds, copied | 5953856 |
| Saved | **5208472** (8.0x) |
| A second, different book | 736232 |

### Reading this honestly

**SC-002 holds and is asserted, not observed.** The example fails rather than
prints if stored content grows when a world switches a book on, so the first
row is the size before any world and after all of them. A world costs 0.019% of a book to run it.

**SC-001 is the third row against the fourth.** Eight worlds and one book cost
745384 bytes; the same eight worlds under the copy-per-world model spec
049 was first written with would cost 5953856. The saving is the whole
motivation, and it is 8.0x at eight worlds. It grows with every world
added, because the book term does not repeat and only a 144-byte link
does.

**FR-070's other half is the last row.** Stored size grows with distinct books
read: a second book costs a second book. A measurement that showed only the
first result would be equally consistent with nothing being stored at all.

**What this is not.** It is not a measure of a real sourcebook's entries,
which vary in size by kind, and it is not table overhead, index size or TOAST
behaviour — `pg_column_size` measures the datum, not the page it lands on.
Both matter to an operator's disk and neither is measured here; both apply to
a copied entry at least as much as to a link, so neither can close the gap.

**What would reopen it**: a delta table that stores anything per world beyond
a link (Phase 11), any column added to `world_books`, or a change to what an
entry stores.
