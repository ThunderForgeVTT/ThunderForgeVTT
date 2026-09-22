# Data Model: A Slice for Every Feature

There is no database. The entities live in two tracked JSON files,
`scripts/e2e/slices.json` and `scripts/e2e/slice-durations.json`, and in root
`package.json` scripts derived from the first. The file shapes are in
[contracts/slices-json.md](contracts/slices-json.md).

## Slice

A named feature area and the specs that prove it.

| Field | Type | Rules |
|---|---|---|
| `name` | string | Kebab-case, unique, not a reserved script name. It becomes `e2e:<name>`. |
| `summary` | string | One line naming the feature area and its `specs/NNN-*` directories. |
| `own` | string[] | Exact spec file names (`combat-panel.spec.ts`) or prefixes (`combat-`). Paths are relative to `apps/web/e2e`, so `torture/` is a valid prefix. Each entry must match at least one spec. |
| `neighbours` | Seam[] | May be empty. Each names a spec owned by a *different* slice. |
| `paths` | string[] | Globs over repository paths that this slice covers for the lookup, such as `apps/web/src/features/combat/**` or `src/server/src/combat/**`. Each glob must match at least one tracked file. |
| `standalone` | string? | A `pnpm` command for a stack-free suite. It is set only where a harness exists; today that is only the hero builder (`pnpm -F @thunderforge/hero-builder-app test:e2e`). |

**Derived**:

- `specs(slice)` = the specs it owns ∪ its neighbours' specs. These are
  exact paths; they are what `--slice` runs.
- `lanes(slice)` = the runner lanes its specs fall in. This is informational
  and printed by `list`.

**Validation**: rules 1–9 in [research R7](research.md#r7-the-coverage-check).

## Seam

The reason a neighbouring spec belongs in a slice.

| Field | Type | Rules |
|---|---|---|
| `spec` | string | An exact spec file name that exists and is owned by another slice. |
| `seam` | string | One sentence naming the surface this slice changes and the neighbour asserts, e.g. "writes actor imagery that the art panel shows". |

## Ownership (relation)

Each spec file has exactly one **owner**: the slice whose `own` entry
matches it most specifically. An exact name beats a prefix, and a longer
prefix beats a shorter one. A tie is an error. A spec may appear as a
neighbour in any number of slices.

## Cross-cutting rule

A path whose change could break any slice, so the answer is "full suite".

| Field | Type | Rules |
|---|---|---|
| `glob` | string | Must match at least one tracked file. |
| `why` | string | Printed by the lookup, e.g. "every client speaks this schema". |

Initial set:

- `src/app/schema.graphql`
- the generated SDK bindings directory
- `src/server/src/auth/**`
- `src/server/src/schema.rs`
- `src/server/migrations/**`
- `apps/web/e2e/fixtures/**`
- `apps/web/e2e/harness/**`
- `apps/web/playwright.config.ts`
- `scripts/e2e-parallel.mjs`
- `scripts/e2e/**` except `slices.json` and `slice-durations.json`
- `apps/web/src/components/ui/**`
- `apps/web/src/styles/**`
- `pnpm-lock.yaml`
- `Cargo.lock`

The exact globs are confirmed against `git ls-files` during implementation.

## No-e2e rule

Paths whose change needs no e2e, answered "no slice needed": `docs/**`,
`specs/**`, `**/*.md`, `.specify/**`, `marketing/**`. This set is built in
rather than declared, so it cannot be widened by accident to hide code.

## Slice measurement

One record per slice in `scripts/e2e/slice-durations.json`, overwritten on
each recorded run.

| Field | Type | Rules |
|---|---|---|
| `wallSeconds` | number | From lock acquired to report written. Stack start and engine build are included. |
| `specs` | number | The number of spec files run. |
| `passed`, `failed`, `flaky`, `skipped` | number | Test counts from the run's report. |
| `measuredAt` | string | ISO date (`YYYY-MM-DD`). |
| `commit` | string | Short SHA of `HEAD` at the start of the run. `-dirty` is appended if the tree had changes. |

**States**, as `list` shows them:

| State | Condition |
|---|---|
| not measured | No record exists. `list` shows the per-spec estimate, labelled `est.` |
| measured | `wallSeconds ≤ 600` and `failed = 0` |
| over target | `600 < wallSeconds ≤ 720` |
| over limit | `wallSeconds > 720`, which fails SC-002 and means the slice must split |
| red | `failed > 0`, shown whatever the time |

A record for a slice that no longer exists is a check failure (rule 9).
