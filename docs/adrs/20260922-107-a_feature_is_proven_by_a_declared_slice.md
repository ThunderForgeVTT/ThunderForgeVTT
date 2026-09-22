# ADR-107: A Feature Is Proven by a Declared Slice

**Date:** 2026-09-22
**Status:** **PROPOSED** 2026-09-22 with spec 060. It becomes Accepted once spec 060's proof has run every slice once, measured (quickstart scenario 6).
**Participants:** ThunderForgeVTT Team
**Related:** spec 060 (FR-001–FR-021; research R1–R10; contracts `cli.md`, `slices-json.md`), constitution Principle VI (v1.3.0), spec 044 (`pnpm e2e:hero-builder`, the model slice)

---

## Problem Statement

Constitution Principle VI says *what*: every feature is proven by its own
slice, the smallest set of e2e specs that crosses every seam the feature
touches, and the full suite is not the gate for an individual change. It
does not say *how* the repository knows which specs make up a slice.

Today the only slice is spec 044's, and its membership lives in the body of
a `package.json` script, `--only=hero-builder-,actor-art`. Retrofitting the
principle to 181 spec files that way would scatter membership across dozens
of scripts. The lookup ("which slice covers my change?") and the coverage
check ("does every spec belong somewhere?") would each need a copy, and the
copies would drift. `--only` also matches a substring anywhere in a path, so
a slice meant to be quick can quietly pull in a release-build perf spec.

## Decision

1. **One list, in JSON (R1).** Slices are declared in one hand-edited file,
   `scripts/e2e/slices.json`, beside the runner's other modules. The same
   file holds the cross-cutting rules. The runner, the lookup, the check and
   the scripts all read it through `scripts/e2e/slices.mjs`; nothing else
   parses it.

2. **One owner per spec, any number of neighbours; the more specific match
   wins (R2).** A slice has `own` entries (exact file names or file-name
   prefixes) and `neighbours` (exact file names only, each with a `seam`
   sentence saying why it is borrowed). Every spec has exactly one owning
   slice and may be a neighbour in any number of others. When two `own`
   entries match one file, an exact name beats a prefix and a longer prefix
   beats a shorter one. A tie at equal specificity fails the check.

3. **`--slice` resolves to exact paths, and `--only` is unchanged (R3).**
   `scripts/e2e-parallel.mjs --slice=<name>` runs exactly the files the
   slice names, own and neighbours, in place of `allSpecFiles()`. Lanes
   (first-run, measured, GitHub apps), the run lock and reporting proceed as
   before. `--slice` with `--only` is an error; an unknown name exits 2 and
   lists the valid ones. `--only` keeps its substring behaviour for ad hoc
   runs.

4. **The scripts carry only the slice name, and are checked for drift
   (R4).** Each slice has `e2e:<slice>` and `e2e:<slice>:integration`, whose
   body is exactly `node ./scripts/e2e-parallel.mjs --shards=1 --slice=<slice>`,
   and `e2e:<slice>:standalone` where a standalone suite exists.
   `scripts/check-e2e-slices.mjs`, verify step `e2e-slices` in the
   pre-commit group, fails on a missing, extra or non-canonical script, and
   on an orphaned spec, an entry or glob that matches nothing, or an
   ownership tie. `--fix` rewrites the scripts. The hero-builder's three
   script names are kept.

5. **Cross-cutting paths are answered "full suite" (R5).** `e2e-slice.mjs
   which` answers a path, in order: a cross-cutting rule ("full suite", with
   its reason); a spec file (its owner and every slice that borrows it); a
   source path (every slice whose `paths` globs match); otherwise
   "uncovered", reported explicitly with exit 3. Documentation and `.md`
   files need no e2e.

6. **Durations are measured, not estimated, in their own file (R10).** A
   `--slice=<name> --record-durations` run writes the whole run's wall time,
   from lock to report, with its counts, date and commit, to
   `scripts/e2e/slice-durations.json`. A failed run is recorded too. The
   listing prints measured times with their date and marks anything else
   `est.`. The file is separate from `.e2e-shards-durations.json`, which is
   never committed with feature work.

Principle VI is the *what*; this ADR is the *how*. A future change that
wants a second place to declare slice membership should amend this ADR
instead.

## Rationale

- **One list is the only list that cannot drift.** FR-001 asks for one
  declared list, and four consumers need it. JSON can be read by any tool,
  including an agent with only `jq`, without executing code, and a diff to
  it reads as a membership change and nothing else.
- **Single ownership makes the questions well defined.** "Whose spec is
  this?" and "which spec belongs to no one?" each have one answer. Prefix
  ownership lets a new `combat-foo.spec.ts` join without an edit, as
  Principle VI asks; exact-name neighbours keep a slice from growing when an
  unrelated spec happens to share a prefix.
- **A slice runs exactly the files it names.** Substring matching is what
  made `--only=lighting` select `engine-lighting-limits` and its release
  build. Resolving to paths in a module keeps that logic out of a
  runner that is already long.
- **A check that runs at commit time catches drift when it is cheap.** It
  costs well under a second, which is the criterion `verify.mjs` documents
  for the pre-commit group.
- **Honest numbers.** Per-spec sums miss stack start and engine build,
  which dominate small slices. Principle VI's ten-minute target is only
  meaningful against what a contributor actually waits for.

## Consequences

- Adding an e2e spec now means placing it in a slice. The check fails until
  it is, and the Spec Kit plan and tasks templates ask for it up front.
- `slices.json` is a file every feature touches. It has one writer at a
  time; parallel agents merge their fragments.
- Neighbours are declared by hand. The lookup and the `paths` globs make a
  missing neighbour visible, but only review decides whether a seam is
  really crossed.
- Measured-lane specs inside a slice still trigger the release engine build;
  such a slice's recorded time shows the cost rather than hiding it.
- Tests: `scripts/e2e/__tests__/slices.test.mjs` (`node --test`), run inside
  the `e2e-slices` verify step, covering the resolver, specificity, lookup
  order, each check failure and the `--only` regression.

## Alternatives Considered

- **A JS module exporting the list.** Allows comments and computed prefixes,
  but an agent must run it to read it, and code tends to grow in it. A `why`
  string per neighbour carries the rationale instead.
- **Per-spec annotations** (a `// @slice combat` header). Spreads membership
  across 181 files and cannot express neighbours: a spec would have to name
  every slice that borrows it.
- **Membership in `package.json` scripts**, as `--only` does today. The
  duplication FR-001 forbids once the lookup and the check need the same
  data.
- **Globs over spec paths.** More power than 181 flat files need, and a glob
  typo silently selects nothing.
- **Multiple owners per spec.** Makes the lookup ambiguous; the existing
  overlaps (`scene-lighting`, `scene-preload`, `interactive-lighting`) each
  have one obvious home.
- **Generated `--only=a,b,c` lists in `package.json`.** Duplicates
  membership.
- **A separate slice runner that spawns the parallel runner.** A second
  process owns the lock, and two places decide how lanes work.
- **Only `pnpm e2e:slice <name>`, no per-slice scripts.** Breaks the
  `e2e:<feature>` convention Principle VI names.
- **Generating the scripts at install time.** A hidden rewrite of a tracked
  file.
- **Coverage from import graphs or V8 coverage.** Precise, but needs a full
  run to build and goes stale; declared globs are cheap, reviewable and held
  honest by the check.
- **Running the check only in `pnpm verify`.** A spec committed without a
  slice would survive until the next push, for no saving.
- **Deriving slice times from the per-spec durations file.** Misses stack
  start, and that file must not be committed with feature work.
- **Moving `canvas-authoring` to an engine slice** to avoid a release build
  in *canvas*. It owns canvas behaviour; its cost is a measurement to take,
  not a reason to misfile it.
- **One slice per `specs/` directory, or slices balanced to equal
  duration.** Spec directories do not map onto e2e files, and a slice is
  chosen by what a change touches, not by how long it takes.
