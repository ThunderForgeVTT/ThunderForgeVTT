# Quickstart: proving spec 060 works

Each scenario below lists the command, then what it must show. Command
shapes are in [contracts/cli.md](contracts/cli.md), and the entities in
[data-model.md](data-model.md).

**Prerequisites**:

- The local e2e stack prerequisites, the same as for `node ./scripts/e2e-parallel.mjs`.
- No other e2e run in progress. The run lock refuses a second one.
- Nothing editing or committing in the tree while a slice runs.

## 1. The coverage check is green and fast (US3, SC-001)

```bash
time node scripts/check-e2e-slices.mjs
pnpm verify
```

- Expect `e2e slices: 181 specs in 27 slices, 0 orphans`. The spec count
  is whatever `apps/web/e2e` holds that day.
- Expect exit 0 in under 1 s, and the `e2e-slices` step in verify's
  summary.

## 2. The check catches what it must (US3, FR-014)

The unit tests in `scripts/e2e/__tests__/slices.test.mjs` prove each case
against fixtures. They run inside step 1. To see one case against the real
tree:

```bash
cp apps/web/e2e/dice-roll.spec.ts apps/web/e2e/zz-orphan.spec.ts
node scripts/check-e2e-slices.mjs; echo "exit $?"
rm apps/web/e2e/zz-orphan.spec.ts
```

- Expect exit 1 and `apps/web/e2e/zz-orphan.spec.ts belongs to no slice — …`.
- Do not do this during an e2e run: a new spec file there would join a
  running Playwright walk.

## 3. Listing and lookup (US2, US4, SC-004)

```bash
pnpm e2e:slices
pnpm e2e:slices combat
time pnpm e2e:which apps/web/e2e/status-display.spec.ts src/app/schema.graphql docs/CONTRIBUTING.md
pnpm e2e:which --diff=HEAD~5; echo "exit $?"
```

- `list` shows 27 rows, each with a time. Before the proof, the times are
  `est.`; after it they are dated.
- `which` answers in under 5 s:
  - the spec file maps to its owner and its borrowers;
  - `schema.graphql` answers "FULL SUITE";
  - the doc answers "no e2e needed".
- The diff lookup exits 0, or 3 while any file is uncovered. Each uncovered
  file must be either given a slice (`paths`) or deliberately left
  uncovered before this feature closes.

## 4. The existing entry point is unchanged (FR-008)

```bash
pnpm e2e:hero-builder
```

- Expect the standalone run with 7 passed, then the integration run with
  16 passed, 0 failed, 0 flaky and 0 skipped. That matches the 2026-09-21
  result, now resolved through `--slice=hero-builder`.
- `grep ✘` on the log finds nothing.

## 5. Lanes still separate inside a slice (R6)

```bash
pnpm e2e:engine-other:integration
```

- The log shows the measured lane: a release build, run serially.
- The warning names the slice as measured-only.
- The run is green.

## 6. The proof: every slice, once, measured (FR-016, FR-017, SC-002, SC-005)

```bash
for s in $(node scripts/e2e-slice.mjs list --json | jq -r '.slices[].name'); do
  pnpm "e2e:$s:integration" -- --record-durations > "/tmp/claude-1000/slice-$s.log" 2>&1
  echo "$s exit $?"; grep -c '✘' "/tmp/claude-1000/slice-$s.log"
done
pnpm e2e:slices
```

- Every slice exits 0 with no `✘`.
- `scripts/e2e/slice-durations.json` has one dated record per slice.
- `list` shows no `over limit` row, and at least 80% of rows are within
  10 minutes.
- A slice over the limit is split, not accepted. The spec's edge cases
  describe this.
- Run the slices one at a time. The lock would refuse a parallel run, and
  the numbers would be meaningless anyway.
- Commit `slice-durations.json`, never `.e2e-shards-durations.json`.

## 7. Templates carry the principle (US5, FR-018, FR-019)

```bash
grep -n "Principle VI\|slice" .specify/templates/plan-template.md .specify/templates/tasks-template.md
```

- The plan template's Constitution Check asks for the slice name and the
  neighbouring specs that cross the feature's seams.
- The tasks template's final phase has a proof task of the form
  "Run `pnpm e2e:<slice>` and record its result", plus a task to add the
  slice to `scripts/e2e/slices.json`.
