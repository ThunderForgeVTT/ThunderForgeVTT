# Contract: slice commands

Every command runs from the repository root. Output is human-readable by
default; `--json` is available where noted for agents.

## `pnpm e2e:<slice>` · `:integration` · `:standalone`

| Script | Body (canonical; checked by R7 rule 6) |
|---|---|
| `e2e:<slice>:integration` | `node ./scripts/e2e-parallel.mjs --shards=1 --slice=<slice>` |
| `e2e:<slice>:standalone` | The slice's `standalone` command. Only present when the slice declares one. |
| `e2e:<slice>` | `pnpm run e2e:<slice>:standalone && pnpm run e2e:<slice>:integration` if a standalone suite exists, otherwise `pnpm run e2e:<slice>:integration` |

Extra runner flags pass through: `pnpm e2e:combat:integration -- --record-durations`,
and `-- --keep`.

## `node scripts/e2e-parallel.mjs --slice=<name>`

- Runs exactly `specs(slice)`, routed through the existing lanes, lock and
  bypass.
- The report header names the slice and its spec count, e.g.
  `e2e slice combat: 13 specs (1 shard)`.
- `--slice` combined with `--only` or `--all` exits **2** with a message.
- An unknown name exits **2** and prints the valid names.
- `--record-durations` also writes the slice's measurement (R10).
- The exit code is otherwise the runner's current one: 0 when green,
  non-zero on any failure.

## `pnpm e2e:slices` → `node scripts/e2e-slice.mjs list [--json]`

One row per slice:

```text
slice              specs  own  nb  lanes              time       measured
combat                13   11   2  default            6m 40s     2026-09-23 7928092
canvas                10    9   1  default,measured   est. 6.3m  not measured
engine-limits          2    2   0  measured           11m 05s    2026-09-23  over target
```

The footer gives totals: slices, specs, and how many are measured, over
target, over limit or red. The command exits 0.

`list <name>` prints one slice in full:

- its owned specs, and for each whether it matched by exact name or prefix;
- its neighbours, each with its seam;
- its `paths` globs and its standalone command;
- the command to run it.

## `pnpm e2e:which` → `node scripts/e2e-slice.mjs which …`

**Input** is one of:

- `which <path>…`
- `which --diff[=<base>]`: the base defaults to `origin/main`; the command
  also includes uncommitted and untracked changes.
- `which --staged`

**Output** is one line per path, then a verdict:

```text
apps/web/src/…/combat/CombatPanel.tsx         combat
apps/web/e2e/status-display.spec.ts            status (owner) · combat (neighbour)
src/server/migrations/…/up.sql                 FULL SUITE — schema history every slice reads
tools/random/thing.rs                          UNCOVERED — no slice or rule matches
docs/CONTRIBUTING.md                           no e2e needed

Run: pnpm run e2e:combat && pnpm run e2e:status
But: a cross-cutting path changed → run the full suite (node ./scripts/e2e-parallel.mjs) before merging.
```

**Exit codes**:

| Code | Meaning |
|---|---|
| 0 | Every path is covered by a slice, a cross-cutting rule or the no-e2e rule. |
| 3 | At least one path is uncovered. Everything else is still printed. |
| 2 | Usage error, a bad base ref, or no git repository. |

`--json` gives:

```json
{ "paths": [ { "path": "…", "kind": "slice|spec|crossCutting|noE2e|uncovered", "slices": ["combat"], "why": "…" } ],
  "run": ["combat", "status"], "fullSuite": false, "uncovered": [] }
```

It must answer in under 5 s (SC-004).

## `node scripts/check-e2e-slices.mjs [--fix]` (verify step `e2e-slices`)

- Runs the rules in [research R7](../research.md#r7-the-coverage-check),
  then `node --test scripts/e2e/__tests__`.
- On success it prints one line:
  `e2e slices: 181 specs in 27 slices, 0 orphans`.
- On failure it prints one line per problem, each with the file and the
  fix, and exits **1**:
  - `apps/web/e2e/new-thing.spec.ts belongs to no slice — add it to "own" of a slice in scripts/e2e/slices.json`
  - `slice "combat": neighbour "combat-old.spec.ts" does not exist`
  - `package.json: "e2e:combat:integration" must be "node ./scripts/e2e-parallel.mjs --shards=1 --slice=combat" (run with --fix)`
- `--fix` rewrites only the `e2e:<slice>*` scripts in `package.json`.
  Membership is never guessed.
- It is registered in `scripts/verify.mjs` with id `e2e-slices`, and that
  id is added to the `.hooks/pre-commit` `--only=` list.
