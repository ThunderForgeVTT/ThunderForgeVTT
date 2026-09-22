# Implementation Plan: A Slice for Every Feature

**Branch**: work lands on `main` (feature directory `060-a-slice-for-every-feature`) | **Date**: 2026-09-22 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/060-a-slice-for-every-feature/spec.md`

## Summary

Retrofit constitution Principle VI (v1.3.0) to the whole e2e suite. Every
spec under `apps/web/e2e` is assigned to a named **slice**. A slice is the
smallest set of specs that crosses every seam of one feature area, and it
targets about ten minutes on one shard. All slices live in one declared list,
`scripts/e2e/slices.json`.

Four things read that list and nothing else:

- **The runner.** `scripts/e2e-parallel.mjs` gains `--slice=<name>`, which
  resolves the slice to exact spec paths. Its lock, lanes and rate-limit
  bypass are unchanged.
- **A small CLI.** `scripts/e2e-slice.mjs` lists the slices and looks up
  which slices cover a set of paths or a diff.
- **A coverage check in `pnpm verify`.** `scripts/check-e2e-slices.mjs` fails
  on a spec that belongs to no slice, on stale entries, on globs that match
  nothing, and on root `package.json` scripts that have drifted from the list.
- **The durations record.** Each slice's measured wall-clock time goes in a
  tracked `scripts/e2e/slice-durations.json`, written only when a run is
  asked to record.

The Spec Kit plan and tasks templates gain the Principle VI item and the
slice proof task. This feature's own proof runs every slice once, one after
another, and records each duration.

## Technical Context

**Language/Version**: Node.js ESM scripts (`.mjs`) on Node v24.19.0, the
version the repo already uses for `scripts/*.mjs`. No TypeScript build step.

**Primary Dependencies**:

- Node built-ins only: `node:fs`, `node:path`, `node:child_process` for
  `git diff --name-only`, and `path.matchesGlob` for globs (present in
  v24.19.0). No new npm dependency.
- Playwright and the existing `scripts/e2e-parallel.mjs` harness, unchanged
  apart from the new flag.

**Storage**: Two tracked JSON files:

- `scripts/e2e/slices.json`: the declared list, edited by hand.
- `scripts/e2e/slice-durations.json`: measurements, written only by
  `--record-durations`.

No database.

**Testing**:

- `node --test` unit tests for the resolver, the lookup and the checker, in
  `scripts/e2e/__tests__/`. This is the first `node:test` suite under
  `scripts/`; research R9 explains why it is not vitest.
- The coverage check runs in `pnpm verify`.
- The feature's proof is a real run of every slice.

**Target Platform**: Linux developer machines and agent worktrees. There is
no CI (per memory, 2026-09-14), so every check is local.

**Project Type**: Developer tooling: a CLI plus a verify step, inside a
pnpm monorepo.

**Performance Goals**:

- The coverage check finishes in under 1 s. It reads one JSON file, walks one
  directory and parses one `package.json`, so it fits the pre-commit
  "flat-cost" group.
- The lookup answers in under 5 s (SC-004), including one `git diff`.
- Each slice finishes in 12 minutes or less on one shard, and at least 80% of
  slices finish in 10 minutes or less (SC-002).

**Constraints**:

- The full suite's behaviour is untouched (FR-020). `--only`, `--all`,
  `--suite` and shard balancing behave exactly as before.
- The hero-builder's three scripts keep their names and results (FR-008).
- No spec is skipped, weakened or deleted (FR-021).
- A slice never depends on state left by a spec outside it (Principle VI).
- `.e2e-shards-durations.json` is not committed alongside this feature's
  work.

**Scale/Scope**:

- 181 spec files: 174 top-level plus 7 in `torture/`. `journeys/` is
  excluded, as it is from `allSpecFiles()` today.
- About 27 slices (research R8).
- About 80 minutes of serial spec time, from `.e2e-shards-durations.local.json`.

## Constitution Check

*Gate before Phase 0; re-checked after the Phase 1 design below.*

| Principle | How this plan satisfies it |
|---|---|
| **I. ECS owns simulation, React owns chrome** | Not touched. No engine, web-app or server code changes. The feature reads spec file names and repository paths only. |
| **II. Plugin-modular engine** | Not touched. The engine perf specs are grouped into two slices, but their assertions and the release build of the measured lane are unchanged. |
| **III. Ownership at the data boundary** | Not touched. No authority, mutation or data path changes. The accounts and auth slice regroups existing specs; it does not relax the rate-limit bypass. `run-lock.mjs` still sets it only for harness runs. |
| **IV. ADRs before divergent implementation** | One ADR: **ADR-107, "A feature is proven by a declared slice"**. It records the single list, JSON over a JS module, one owner per spec with any number of neighbours, cross-cutting paths answered "full suite", and measured-not-estimated durations. Principle VI says *what*; this ADR records *how*, so that the next person does not add a second list. |
| **V. Verify before claiming done** | Gates: `pnpm verify` with the new check, `node --test scripts/e2e/__tests__`, and the proof. The proof runs every slice sequentially with `--record-durations`, and each log is searched for `✘`. The full suite is not re-run: FR-020 changes nothing it does, and `--only`/`--all` are regression-tested in the resolver tests. |
| **VI. Every feature is proven by its own slice** | This feature is tooling, so its slice is stack-free. Its proof is `pnpm verify` plus the unit tests, plus `pnpm e2e:hero-builder:integration` as the one real-stack neighbour. That script now resolves through `--slice`, so it is the seam this feature changes. **Neighbouring specs:** the hero-builder slice (the existing entry point whose resolution changes) and a measured-lane slice (`e2e:engine-other`, which proves lanes still separate inside a slice). |

**Gate result (pre-research)**: PASS. No violations; Complexity Tracking is
empty.

**Gate result (post-design)**: PASS.

- The design adds two scripts and one module under `scripts/e2e/`, plus a
  flag. It adds no dependency and no second source of membership.
- `scripts/e2e-parallel.mjs` is already 1,464 lines, so slice resolution
  lives in `scripts/e2e/slices.mjs` and the runner only imports it (R3).

## Project Structure

### Documentation (this feature)

```text
specs/060-a-slice-for-every-feature/
├── plan.md              # This file
├── research.md          # Phase 0: decisions R1–R10 and the slice grouping
├── data-model.md        # Phase 1: Slice, Seam, Cross-cutting rule, Measurement
├── quickstart.md        # Phase 1: how to prove this feature works
├── contracts/
│   ├── slices-json.md   # The declared list's shape and its validation rules
│   └── cli.md           # e2e:<slice>, e2e:slices, e2e:which, the verify check
├── checklists/
│   └── requirements.md
└── tasks.md             # /speckit-tasks (not created here)
```

### Source Code (repository root)

```text
scripts/
├── e2e-parallel.mjs              # + --slice=<name> (imports slices.mjs); --only unchanged
├── e2e-slice.mjs                 # NEW: list | which <paths…> | which --diff[=<base>]
├── check-e2e-slices.mjs          # NEW: the coverage check (verify step "e2e-slices")
├── verify.mjs                    # + one step, in the pre-commit flat-cost group
└── e2e/
    ├── slices.json               # NEW: the declared list (hand-edited)
    ├── slice-durations.json      # NEW: measured wall-clock per slice (written by runs)
    ├── slices.mjs                # NEW: load, validate, resolve, match, lookup
    └── __tests__/
        └── slices.test.mjs       # NEW: node --test for resolve/lookup/check

package.json                      # + e2e:<slice>[:integration|:standalone] per slice,
                                  #   e2e:slices, e2e:which; hero-builder's three kept
.hooks/pre-commit                 # + "e2e-slices" in the flat-cost ids
.specify/templates/plan-template.md   # Constitution Check: Principle VI item
.specify/templates/tasks-template.md  # Proof task naming the slice + its result
docs/CONTRIBUTING.md              # "Proving a change" section: slices, which, full suite
docs/adrs/20260922-107-a_feature_is_proven_by_a_declared_slice.md
```

**Structure Decision**: All logic sits in one module, `scripts/e2e/slices.mjs`,
which the runner, the CLI and the check import. The runner stays a runner. It
receives a list of resolved paths and treats them like the files `--only`
selects today, so lanes, lock and bypass apply unchanged.

## Complexity Tracking

No violations to justify.
