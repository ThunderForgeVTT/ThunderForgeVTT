# Tasks: A Slice for Every Feature

**Input**: Design documents from `specs/060-a-slice-for-every-feature/`

**Prerequisites**: [plan.md](plan.md), [spec.md](spec.md),
[research.md](research.md), [data-model.md](data-model.md),
[contracts/](contracts/), [quickstart.md](quickstart.md)

**Tests**: This feature needs unit tests. The check must be seen to fail in
each case it guards (FR-014), and `node --test` is how the plan proves that
(research R9). Test tasks sit beside the code they prove.

**Organization**: Tasks are grouped by user story. US1, US2 and US3 are all
P1, and each depends only on Phase 2.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: The task can run in parallel, because it touches different files
  and depends on no incomplete task.
- **[Story]**: US1–US6, from [spec.md](spec.md).

## Standing rules (apply to every task)

- **Never edit, commit or run `pnpm verify` while an e2e run is in progress.**
  Every slice run in this ledger runs alone. The run lock enforces this for
  runs; nothing enforces it for edits.
- Search every e2e log for `✘` before calling a run green.
- **Stage explicit paths.** Never use `git add -A`. Commit through
  `flock /tmp/claude-1000/thunderforge-commit.lock git commit -F <msg> -- <paths>`,
  and confirm the commit is signed (`git log -1 --format=%G?` shows `G`).
- Never commit `.e2e-shards-durations.json` with this feature's work.
  `scripts/e2e/slice-durations.json` is the file this feature tracks.
- No spec in `apps/web/e2e` may be deleted, skipped or weakened to make a
  slice pass or fit (FR-021). If a spec is flaky inside a slice, fix the
  spec where it lives.

---

## Phase 1: Setup

- [X] T001 Create the `scripts/e2e/__tests__/` directory. Add a root
      `package.json` script, `"test:scripts": "node --test scripts/e2e/__tests__"`.
      Confirm `node --test` runs, and passes, on an empty placeholder test
      file, `scripts/e2e/__tests__/slices.test.mjs`.
      - 2026-09-22: `pnpm test:scripts` passes 1/0 on the placeholder. The script is `node --test "scripts/e2e/__tests__/*.test.mjs"`, not the bare directory: Node 24 treats a directory argument as a module to load and fails with `MODULE_NOT_FOUND`, so the glob (which `node --test` expands itself) is what runs the directory. Later tasks that say `node --test scripts/e2e/__tests__` should use `pnpm test:scripts` or the glob.
- [X] T002 [P] Write ADR-107, "A feature is proven by a declared slice", in
      `docs/adrs/20260922-107-a_feature_is_proven_by_a_declared_slice.md`,
      in the format of ADR-106. Status: Proposed. It records these decisions:
      - one list, in JSON (R1);
      - one owner per spec, any number of neighbours, and the more specific
        match wins (R2);
      - `--slice` resolves to exact paths, and `--only` is unchanged (R3);
      - the scripts carry only the slice name and are checked for drift (R4);
      - cross-cutting paths are answered "full suite" (R5);
      - durations are measured, not estimated, in their own file (R10).

      It also records each rejected alternative from research.md, and names
      Principle VI as the *what* this ADR implements. Add a row to
      `docs/adrs/README.md` if that file indexes ADRs.
      - 2026-09-22: Written, status Proposed; it becomes Accepted once the
        proof (quickstart scenario 6) has run every slice. It records R1–R5
        and R10 as six decisions, and every rejected alternative from R1–R10.
        `docs/adrs/README.md` indexes ADRs, so it gained a Proposed row.

---

## Phase 2: Foundational (blocks every story)

**Purpose**: The declared list and the module that reads it. Every later
task reads the list through `scripts/e2e/slices.mjs` and never parses
`slices.json` on its own.

- [X] T003 Move spec discovery out of the runner. Move `allSpecFiles(suite)`
      and `PERF_LANE_SPECS` from `scripts/e2e-parallel.mjs` (lines 137 and
      203), with `isPerfSpec`, `isFirstRunSpec` and `isGithubAppsSpec`, into a
      new `scripts/e2e/specs.mjs`. The runner imports them from there, and
      its behaviour is byte-for-byte the same.
      - Prove it before and after with `node scripts/e2e-parallel.mjs --help`,
        or with a dry listing if the runner has one; otherwise use a
        throwaway `node -e` that prints `allSpecFiles()`.
      - The move exists so the check and the CLI see exactly the file set
        the runner runs: `torture/` included, `journeys/` excluded.
      - 2026-09-22: `scripts/e2e/specs.mjs` exports `SUITES`, `PERF_LANE_SPECS`, `allSpecFiles`, `isFirstRunSpec`, `isPerfSpec` and `isGithubAppsSpec`, moved with their comments; `SUITES` came too, since `allSpecFiles` reads it. The runner has no `--help` or dry listing (`--help` is "Unknown argument"), so a throwaway script printed every file of both suites with its three lane predicates, plus `PERF_LANE_SPECS`, from the old source and from the new module: the two outputs are identical (181 e2e specs, paths relative to `apps/web`, e.g. `e2e/combat-panel.spec.ts`).
- [X] T004 Implement `scripts/e2e/slices.mjs`, following
      [data-model.md](data-model.md) and
      [contracts/slices-json.md](contracts/slices-json.md). It exports:
      - `loadSlices(root)`, which parses JSON, rejects unknown keys (so
        `neighbors` fails) and requires slices sorted by name;
      - `ownerOf(specPath, slices)`, where an exact name beats a prefix, a
        longer prefix beats a shorter one, and a tie throws with both slice
        names;
      - `resolveSlice(name, slices, specFiles)`, which returns exact
        repo-relative paths (owned ∪ neighbours, sorted, deduplicated) and
        throws a `SliceError` listing the valid names on an unknown name;
      - `sliceLanes(paths)`, which uses the `specs.mjs` predicates;
      - `matchPath(path, globs)`, which uses `path.matchesGlob`.

      Pure functions: nothing in the module touches git or the network.
      - 2026-09-22: exports `loadSlices(root = ROOT_DIR)` → `{ crossCutting, slices }`, `validateSlices(document)` (the shape rules, for fixtures), `ownerOf(specPath, slices)` → name or `null`, `ownershipOf(specPath, slices)` → `{ slice, entry, exact }` or `null` (for `list <name>`'s exact/prefix column), `resolveSlice(name, slices, specFiles = allSpecFiles())`, `sliceLanes(paths)` → lanes in `LANES` order, `matchPath(path, globs)` → the first matching glob or `null`, plus `SliceError`, `specKey`, `entryMatches`, `isExactEntry`, `SLICES_FILE`, `RESERVED_SLICE_NAMES` and `LANES`. `resolveSlice` returns paths as `specFiles` spells them — `allSpecFiles()`'s `e2e/x.spec.ts`, relative to `apps/web` — because that is what the runner hands Playwright; `ownerOf` accepts that form, the `apps/web/e2e/…` form or the bare name. A tie is judged only at the winning specificity, so two slices sharing a prefix do not tie on a file a third names exactly. `validateSlices` also rejects non-kebab, duplicate and reserved (`slices`, `which`) names and a prefix given as a neighbour.
- [X] T005 [P] Unit tests for T004 in `scripts/e2e/__tests__/slices.test.mjs`,
      against in-memory fixtures rather than the real tree:
      - exact beats prefix; longer prefix beats shorter; a tie throws;
      - a prefix selects `torture/` files;
      - resolve deduplicates a neighbour that is also owned;
      - an unknown slice throws and lists names;
      - an unknown key is rejected;
      - an unsorted list is rejected.
      - 2026-09-22: 19 tests, 19 pass (`pnpm test:scripts`). Beyond the list above: a tie below an exact winner is not a tie, every path spelling gets the same owner, a missing neighbour throws, duplicate/non-kebab/reserved names are rejected, `loadSlices` reads a temp root and rejects bad JSON, `sliceLanes` covers all four lanes, and `matchPath` crosses directories with `**`.
- [X] T006 Author `scripts/e2e/slices.json` with the 27 slices in
      [research.md R8](research.md#r8-the-slice-grouping).
      - For each slice: `name`, `summary` (naming its `specs/NNN-*`
        directories), `own` and `neighbours` (each with a `seam` sentence
        copied from R8), and `"paths": []` for now; US2 fills `paths`.
      - The hero-builder slice gets
        `"standalone": "pnpm -F @thunderforge/hero-builder-app test:e2e"`.
      - Add `"crossCutting": []` for now; US2 fills it.
      - Before committing, confirm with a throwaway `node -e` that uses
        T004's `ownerOf` over `allSpecFiles()`: every spec has exactly one
        owner, there are no ties, and every entry matches something.
      - Each exact-name entry that settles an R2 overlap gets that overlap
        named in its slice's `summary`: `scene-lighting` and
        `interactive-lighting` go to lighting, `scene-preload` to
        world-cache.
      - 2026-09-22: 27 slices, 181 specs, 181 owned, no orphans, no ties, every `own` entry wins at least one spec, every neighbour exists and is owned by another slice (checked with `ownershipOf` and `resolveSlice` over `allSpecFiles()`). Owned / neighbours / resolved: accounts 13/0/13, actors 9/2/11, book-import 3/1/4, canvas 8/1/9, collections 13/0/13, combat 11/2/13, companion 5/1/6, compendium 4/1/5, engine-limits 2/0/2, engine-other 4/0/4, feedback 6/0/6, game-systems 3/1/4, genie 14/1/15, hero-builder 3/1/4, instance 9/0/9, interactive 7/2/9, lighting 5/1/6, lore 2/0/2, moderation 5/1/6, play-pause 5/2/7, scenes 8/1/9, status 8/1/9, tokens 8/2/10, torture 7/0/7, world-cache 8/0/8, world-cache-core 4/0/4, worlds 7/0/7. Where R8 lists names, a prefix is used when it selects exactly those files, so a new spec joins without an edit: `lore-`, `canvas-` (which covers `canvas-authoring`, so it needs no exact entry), `world-cache` (the four core specs win by exact name) and `engine-` for engine-other (engine-limits' two win by exact name). R8's "`actor-` (6)" is five files today; actors owns 9, not 10. Seams are R8's parentheticals written out as sentences. canvas and engine-other include measured-lane specs; instance spans the default, first-run and GitHub-apps lanes; lore includes the GitHub-apps lane.

**Checkpoint**: `node --test scripts/e2e/__tests__` is green, and the list
resolves every spec to one owner.

---

## Phase 3: User Story 1 - Prove a change to one feature in minutes (Priority: P1) 🎯 MVP

**Goal**: Any slice runs by name, on one shard, running only its specs.

**Independent test**: `pnpm e2e:combat:integration` with nothing else
running. Only the resolved combat specs run, the report names the slice,
and the run is green.

- [X] T007 [US1] Add `--slice=<name>` to `scripts/e2e-parallel.mjs`:
      - Parse the flag in the argv loop (around line 1013).
      - `--slice` with `--only` or `--all` exits 2 with a message.
      - Resolve the slice through `resolveSlice`. An unknown name exits 2
        and prints the valid names.
      - Use the resolved list *in place of* the `onlyPatterns` filter at
        line 1122. It must pass through the same three-lane partition
        unchanged.
      - The report header reads `e2e slice <name>: <n> specs (<shards> shard)`.
      - The measured-only warning at line 1171 also fires for `--slice`,
        reworded to name the slice. It stays a warning, never a failure
        (spec Edge Cases).
      - `--only` behaviour is untouched (FR-020).
      - 2026-09-22: `--slice=<name>` is parsed with the other flags and checked before the dependency check, the lock or any build: `--slice` with `--only`, `--all` or `--suite=playtest` exits 2 naming the conflict (playtest added: the playtest suite has no slices), and an unknown name exits 2 with `unknown slice "x". Valid slices: …` (the list does not load either → exit 2). The resolved paths replace the `--only` filter through `selectedSpecs(args)` in the new `scripts/e2e/select.mjs`, which the engine-profile decision (`measuredSpecsSelected`) now uses too, so a measured slice still gets its release build; the three-lane partition is untouched. The header `e2e slice combat: 13 specs (1 shard)` is logged right after the lock and again as the first line of the end-of-run digest. The measured-only note fires for a slice as `Slice <name> has only measured specs (…), so the run is the serial lane alone, on a release build.` — without `--only`'s "pass --all" advice, since `--all` is refused with `--slice`; still a log line, never a failure. Verified by hand: `--slice=nope`, `--slice=combat --only=x`, `--slice=combat --all` each exit 2 and leave no `.e2e-running`.
- [X] T008 [P] [US1] Add a regression test to
      `scripts/e2e/__tests__/slices.test.mjs`. `--only`'s substring match
      over a fixture file list must return exactly what it returns today,
      so extract the filter into a small exported function if that is what
      makes it testable. `--slice` must return exact paths. The
      `--only=lighting` case from R3 is the example: with `--only` it pulls
      in `engine-lighting-limits`; with `--slice=lighting` it does not.
      - 2026-09-22: in `scripts/e2e/__tests__/runner.test.mjs`, not `slices.test.mjs` (that file belongs to T005; a file of its own kept parallel agents apart). The `--only` filter is now `filterOnly(files, only)` / `onlyPatterns(only)` in `scripts/e2e/select.mjs`, the same expression the runner had inline. Tests pin: no `--only` selects all; `--only=lighting` selects `engine-lighting-limits` and a `torture/` file; comma lists trim and drop empties; `--slice=lighting` returns exactly its own and neighbour files; `sliceConflict` names each conflict; and the runner itself exits 2 on an unknown slice, `--slice`+`--only` and `--slice`+`--all` (spawned — safe beside a live run, since those paths exit before the lock).
- [ ] T009 [US1] Add the root `package.json` scripts for all 27 slices, in
      the canonical form from [contracts/cli.md](contracts/cli.md):
      `e2e:<slice>`, `e2e:<slice>:integration`, and `e2e:<slice>:standalone`
      for the hero builder only.
      - Change `e2e:hero-builder:integration` to
        `node ./scripts/e2e-parallel.mjs --shards=1 --slice=hero-builder`.
      - Leave `e2e:hero-builder` and `e2e:hero-builder:standalone` as they
        are (FR-008).
      - Keep the `e2e:*` scripts together, in slice-name order.
- [ ] T010 [US1] Proof for US1. Run each of these alone, then `grep ✘` its
      log:
      - `pnpm e2e:hero-builder`, which must still give standalone 7 passed
        and integration 16 passed / 0 failed / 0 flaky / 0 skipped, as on
        2026-09-21;
      - `pnpm e2e:combat:integration`, where the log lists only combat's
        resolved specs.

      Record both results and wall times under this task.

**Checkpoint**: US1 is shippable. Every slice is runnable by name, even
before the lookup and the check exist.

---

## Phase 4: User Story 2 - Find which slice covers what I changed (Priority: P1)

**Goal**: `pnpm e2e:which` maps paths or a diff to slices, "full suite",
"no e2e needed" or "uncovered".

**Independent test**: `pnpm e2e:which` on a combat source file names
combat. On `src/app/schema.graphql` it answers FULL SUITE. On a docs file it
answers "no e2e needed". On an unclaimed file it answers UNCOVERED and exits
3. It answers in under 5 s.

- [ ] T011 [US2] Fill `crossCutting` in `scripts/e2e/slices.json` with the
      initial set in [data-model.md](data-model.md#cross-cutting-rule),
      each with a `why`.
      - Confirm each glob against `git ls-files`: the real SDK bindings
        directory and the real shared-UI directory. Replace any that match
        nothing with the real path. Never drop the surface.
      - `scripts/e2e/slices.json` and `scripts/e2e/slice-durations.json`
        are **not** cross-cutting, because editing the list is not a
        harness change.
- [ ] T012 [P] [US2] Fill `paths` for the play-surface slices in
      `scripts/e2e/slices.json`:
      - slices: tokens, canvas, scenes, lighting, interactive, status,
        play-pause, combat;
      - sources: `apps/web/src/**`, `src/server/src/**`, `src/engine/src/**`
        and `packages/**`;
      - derive each glob from the source directories and files those
        specs' features live in. Read the spec's `specs/NNN-*/plan.md`
        Project Structure section to find them.
- [ ] T013 [P] [US2] Fill `paths` for the content slices: actors,
      hero-builder, compendium, collections, book-import, lore, genie,
      game-systems, worlds, world-cache-core, world-cache. Same method as
      T012. Include `packs/**` and `packages/**` where a slice owns a
      package.
- [ ] T014 [P] [US2] Fill `paths` for the platform slices: accounts,
      instance, moderation, feedback, companion, engine-limits,
      engine-other, torture. Same method. The engine slices claim
      `src/engine/**` paths that the play-surface slices do not claim.
      Overlap between slices is allowed: several slices may claim one path.
- [ ] T015 [US2] Implement the lookup in `scripts/e2e-slice.mjs which`,
      following [contracts/cli.md](contracts/cli.md) and research R5:
      - Inputs: `<path>…`, `--diff[=<base>]` (default `origin/main`, plus
        uncommitted and untracked files via `git status --porcelain`),
        `--staged`, `--json`.
      - The order of answers: cross-cutting, then a spec file (owner plus
        borrowers), then `paths` globs, then the built-in no-e2e rule
        (`docs/**`, `specs/**`, `**/*.md`, `.specify/**`, `marketing/**`),
        then uncovered.
      - The final `Run:` line gives the union of slices, each once, plus the
        full-suite line when anything is cross-cutting.
      - Exit codes are 0, 3 and 2.
      - One `git` call per input mode, so the whole lookup answers in under
        5 s.
- [ ] T016 [P] [US2] Lookup tests in `scripts/e2e/__tests__/slices.test.mjs`,
      with the path list passed in and no git involved:
      - every answer kind;
      - precedence: a cross-cutting path that also matches a slice glob
        answers FULL SUITE;
      - a spec file answers its owner plus its borrowers;
      - union deduplication;
      - exit code 3 when anything is uncovered.
- [ ] T017 [US2] Add root scripts `"e2e:which": "node ./scripts/e2e-slice.mjs which"`
      and `"e2e:slices": "node ./scripts/e2e-slice.mjs list"`. `list` is a
      stub until T026.
- [ ] T018 [US2] Triage what is still uncovered. Run
      `git ls-files | xargs node scripts/e2e-slice.mjs which --json`, then
      take every `uncovered` source path and do one of two things:
      - give it to a slice's `paths`, when a slice's specs do exercise it;
      - leave it uncovered deliberately, when no e2e exercises it. Examples
        are pure Rust crates proven by `cargo test`, and build tooling. List
        those under this task with the reason.

      The lookup keeps reporting them as UNCOVERED. That is the honest
      answer (FR-012), and it is not hidden by widening the no-e2e rule.

**Checkpoint**: US2 is shippable. The quickstart's scenario 3 passes.

---

## Phase 5: User Story 3 - No spec falls out of every slice (Priority: P1)

**Goal**: `pnpm verify` and pre-commit fail on an orphan spec, a stale entry
or drifted scripts.

**Independent test**: Copy a spec to `zz-orphan.spec.ts` and run
`node scripts/check-e2e-slices.mjs`. It exits 1 and names the file. Remove
the copy and it exits 0 in under 1 s. Do this only with no e2e run in
progress.

- [ ] T019 [US3] Implement `scripts/check-e2e-slices.mjs` with rules 1–9
      from [research R7](research.md#r7-the-coverage-check):
      - rule 4 reads `git ls-files` once;
      - rule 9 treats a missing `slice-durations.json` as empty;
      - each failure prints the file and the fix, in the wording in
        [contracts/cli.md](contracts/cli.md);
      - on success it prints the one-line summary, then runs
        `node --test scripts/e2e/__tests__`.

      Exit 1 on any failure.
- [ ] T020 [US3] Add `--fix` to `scripts/check-e2e-slices.mjs`. It rewrites
      only the `e2e:<slice>*` scripts in root `package.json` to canonical
      form (rule 6), preserving the order and formatting of every other
      key. It never edits `slices.json`.
- [ ] T021 [P] [US3] Checker tests in `scripts/e2e/__tests__/check.test.mjs`,
      one fixture per rule, each asserting the failure message: an orphan;
      a missing neighbour; a dead prefix; a dead glob; a tie; a missing,
      extra and non-canonical script; a missing standalone script; a bad
      slice name; a stale duration record. Also test that `--fix` output
      is canonical and leaves unrelated keys untouched. To make this
      possible, the checker takes its inputs (the list, spec files,
      tracked files, `package.json`) as arguments, with a thin CLI wrapper
      on top.
- [ ] T022 [US3] Register the check in `scripts/verify.mjs`:
      - id `e2e-slices`, name "e2e slices", command
        `["node", "./scripts/check-e2e-slices.mjs"]`;
      - `--fix` maps to `--fix`;
      - write a comment in the style of the neighbouring steps saying why
        it exists (Principle VI, FR-013).
- [ ] T023 [US3] Add `e2e-slices` to the `--only=` id list in
      `.hooks/pre-commit` (line 34). Update the header comment in
      `scripts/verify.mjs` that counts "the flat-cost four" and the pre-push
      "eight" if those counts are now wrong. Measure the step's time and
      confirm it is under 1 s.
- [ ] T024 [US3] Proof for US3. Run `pnpm verify` and confirm it is green
      and includes `e2e slices`. Run the quickstart's scenario 2: orphan,
      fail, remove, pass. Record both under this task.

**Checkpoint**: US3 is shippable. Slices cannot silently decay.

---

## Phase 6: User Story 4 - Each slice's cost is on record (Priority: P2)

**Goal**: Measured wall time per slice, dated, visible in `pnpm e2e:slices`,
with over-target and over-limit flags.

**Independent test**: Run `pnpm e2e:lore:integration -- --record-durations`.
`scripts/e2e/slice-durations.json` gains a dated `lore` record, and
`pnpm e2e:slices` shows it.

- [X] T025 [US4] Make `scripts/e2e-parallel.mjs` record slice time. When
      `--slice` and `--record-durations` are both given, it writes the
      slice's record to `scripts/e2e/slice-durations.json`, in the shape
      given in [contracts/slices-json.md](contracts/slices-json.md):
      - `wallSeconds` runs from lock acquired to report written;
      - `commit` is the short SHA of `HEAD`, with `-dirty` when
        `git status --porcelain` is non-empty;
      - counts come from the run's report;
      - a red run is recorded too.

      Keys are sorted, with two-space indentation and a trailing newline.
      The existing per-spec `recordDurations` behaviour is unchanged.
      - 2026-09-22: `scripts/e2e/slice-durations.mjs` exports `readSliceDurations(root = ROOT_DIR)` → the parsed file, or `{}` when it is missing (a corrupt file throws); `recordSliceDuration(file, name, record)` (merges, sorts slice names, writes fields in the contract's order, two spaces, trailing newline, via temp file + rename); `measuredCommit(root)` (short SHA, `-dirty` when `git status --porcelain` is non-empty, `null` outside git); `measuredDate(date)` (local `YYYY-MM-DD`); `SLICE_DURATIONS_FILE`, `RECORD_FIELDS`. The runner takes the clock and the commit when the lock is acquired and writes the record right after `e2e-summary.json`, with counts from that summary's totals and `specs` = the resolved list's length. A red or crashed run is recorded; a signal-interrupted one (exit 130) is not, since it measures the Ctrl-C rather than the slice. Unit-tested in `runner.test.mjs`. No `slice-durations.json` is committed; the first recorded run creates it.
- [ ] T026 [US4] Implement `scripts/e2e-slice.mjs list [<name>] [--json]`
      per [contracts/cli.md](contracts/cli.md):
      - one row per slice: specs, own, neighbours, lanes, time, measured
        date and commit;
      - the state column follows [data-model.md](data-model.md#slice-measurement):
        not measured (shown with `est.` time), measured, over target, over
        limit, red;
      - `est.` is the per-spec sum from `.e2e-shards-durations.local.json`,
        falling back to `.e2e-shards-durations.json`, and is never shown
        unlabelled (FR-016);
      - totals appear in a footer;
      - `list <name>` prints the full detail described in the contract.
- [ ] T027 [P] [US4] State tests in `scripts/e2e/__tests__/slices.test.mjs`:
      - the boundaries at 600 s and 720 s;
      - `red` taking precedence over time;
      - a slice that has not been measured shows `est.`.
- [ ] T028 [US4] Proof for US4. Run
      `pnpm e2e:lore:integration -- --record-durations` alone, then
      `pnpm e2e:slices`. Confirm the lore row is dated and every other row
      reads `est.`. Record the result under this task.

---

## Phase 7: User Story 5 - New features are born with a slice (Priority: P2)

**Goal**: The plan and tasks templates carry Principle VI.

**Independent test**: The quickstart's scenario 7. The two templates
contain the Principle VI Constitution Check item and the slice tasks.

- [X] T029 [P] [US5] Edit `.specify/templates/plan-template.md`, the core
      template, following the precedent of commit 462bd7e:
      - Under Constitution Check, add a required item: "**VI. Every feature
        is proven by its own slice** — the slice's name; its own specs; for
        each existing surface this feature changes, the neighbouring specs
        that cross that seam (see `pnpm e2e:which --diff`); and whether a
        standalone half exists."
      - In Project Structure, note that `scripts/e2e/slices.json` gains or
        extends this feature's entry.
      - 2026-09-22: Done. The item follows the constitution gates, marked
        required in every plan, uses `<slice>` placeholders, and asks a
        cross-cutting change to say so (its proof is the full suite). A
        **Slice** note follows the Structure Decision.
- [X] T030 [P] [US5] Edit `.specify/templates/tasks-template.md`:
      - In Phase 1, add
        `- [ ] TXXX Add or extend this feature's slice in scripts/e2e/slices.json (own prefix, neighbours with their seams, paths) and its e2e:<slice> scripts; pnpm verify's e2e-slices check must pass`.
      - Before the `pnpm verify` task in the final phase, add
        `- [ ] TXXX Proof: run pnpm e2e:<slice> alone with --record-durations, grep the log for ✘, and record the result and wall time here. The full suite is for releases and cross-cutting changes (pnpm e2e:which says which)`.
      - 2026-09-22: Done, both lines verbatim: the first after T003 in
        Phase 1, the second between the quickstart validation and
        `pnpm verify` in Phase N.
- [X] T031 [US5] Proof for US5. Run the quickstart's scenario 7 `grep`, and
      confirm both templates still render as valid Spec Kit templates.
      `setup-plan.sh` and `setup-tasks.sh` should copy and read them without
      error in a dry run on a scratch feature directory, which is then
      deleted without being committed.
      - 2026-09-22: Green. The scenario 7 grep finds the item in the plan
        template (lines 47–51, and the Slice note at 116) and both tasks in
        the tasks template (lines 55 and 161). With `.specify/feature.json`
        pointed at a scratch `specs/999-scratch-template-check` holding only
        a stub spec.md, `setup-plan.sh --json` exited 0 and copied the plan
        template, and the copy holds both additions; `setup-tasks.sh --json`
        exited 0, resolved the core tasks template, and its
        TASKS_TEMPLATE_CONTENT holds both new tasks. The scratch directory
        was deleted, no branch was created, and feature.json was restored
        byte for byte (same sha256).

---

## Phase 8: User Story 6 - The stack-free half, wherever a harness exists (Priority: P3)

**Goal**: Standalone halves are part of the declared list, the scripts and
the listing.

**Independent test**: `pnpm e2e:slices hero-builder` shows the standalone
command. Every other slice shows none. `pnpm e2e:hero-builder:standalone`
runs with no stack up.

- [ ] T032 [US6] Make sure `list` shows the `standalone` command in the
      slice detail. In the table, mark slices that have one, for example
      `+standalone` in the lanes column. Slices without one show nothing,
      not an empty entry. Confirm check rule 7 covers a `standalone` whose
      package script does not exist.
- [ ] T033 [US6] Proof for US6. With no stack running, run
      `pnpm e2e:hero-builder:standalone`: 7 passed. Then break the
      standalone half on purpose with a throwaway failing assertion that is
      never committed, and run `pnpm e2e:hero-builder`. The integration half
      must never start, so no run lock is taken. Revert the assertion.
      Record the result under this task.

---

## Phase 9: Polish, and this feature's proof

- [ ] T034 [P] Add a "Proving a change" section to `docs/CONTRIBUTING.md`,
      beside the "Standalone harnesses" section. Cover:
      - `pnpm e2e:which --diff`, then `pnpm e2e:<slice>`;
      - when the full suite is the gate, which is any cross-cutting path;
      - how to add a spec to a slice;
      - what the verify check says when you forget.

      Link ADR-107 and Principle VI.
- [ ] T035 **The proof (FR-016, FR-017, SC-002, SC-005).** Run every slice's
      integration half once, one at a time, with nothing else running:
      `pnpm e2e:<slice>:integration -- --record-durations`. Use the loop in
      [quickstart.md §6](quickstart.md). This is roughly 3 hours of wall
      time, so hand it to a background agent, and have no edits in the tree
      while it runs.
      - `grep ✘` every log.
      - Record each slice's result and wall time in a table under this task.
      - A slice that fails is fixed where the defect is: in the spec, or in
        the slice's neighbours if a seam was missing. It is never fixed by
        skipping a spec. Then re-run that slice alone.
      - A slice over 12 minutes is split along a seam R8 has not already
        used, with the new slices added to `slices.json` and `package.json`,
        and both halves re-measured. A slice that cannot be split without
        losing a seam stays whole, flagged, with the reason recorded here
        (spec Edge Cases).
      - Done when every slice is green, none is over the limit, and at least
        80% are at 10 minutes or less.
- [ ] T036 Commit `scripts/e2e/slice-durations.json` from T035 on its own,
      and not `.e2e-shards-durations.json`.
- [ ] T037 [P] Mark ADR-107 as Accepted, dated, naming T010, T024 and T035
      as the proof. Re-read the spec's Assumptions against what shipped and
      correct anything that moved, such as the final slice count and any
      splits made in T035.
- [ ] T038 Run the quickstart, scenarios 1–7, and record the result under
      this task.
- [ ] T039 Run `pnpm verify` and `node --test scripts/e2e/__tests__` before
      the final commit. Nothing in Rust or the engine changes, so
      `make lint` is not a gate. Run it only if the pre-push hook demands it.
      - This feature's own Principle VI slice is stack-free: the verify
        check and its tests, plus T010's hero-builder run as the real-stack
        neighbour, whose resolution this feature changed.
      - The full suite is not re-run for this feature. FR-020 changes
        nothing it does, and T008 guards `--only`.

---

## Dependencies

```text
Phase 1 (setup) ── T002 (ADR) may run beside everything
  └─ Phase 2 (specs.mjs, slices.mjs, slices.json) ── blocks every story
       ├─ Phase 3  US1 (--slice, scripts)            ── MVP
       ├─ Phase 4  US2 (paths, crossCutting, which)
       ├─ Phase 5  US3 (the check)                   ── rule 6 needs T009's scripts;
       │                                                rule 4 needs T011–T014's globs
       ├─ Phase 6  US4 (durations, list)             ── needs US1's --slice
       ├─ Phase 7  US5 (templates)                   ── independent; names e2e:which from US2
       └─ Phase 8  US6 (standalone in list)          ── needs T026's list
Phase 9 (proof) ── after every story; T035 needs US1 + US4
```

- US1, US2 and US3 depend only on Phase 2 and may proceed in parallel.
- The US3 check's script-sync rule (6) is only meaningful after T009. Its
  glob rule (4) is only meaningful after T011–T014. Implement the rules
  whenever you like, but prove US3 (T024) after US1 and US2 have landed.
- Within a story, the order is: module → tests → CLI or runner wiring →
  scripts → proof.

## Parallel opportunities

- **Phase 1:** T002 can run alongside T001.
- **Phase 2:** T005 (tests) alongside T004 once its exports are named.
  T006 needs T004's `ownerOf` for its confirmation step.
- **Phase 4:** T012, T013 and T014 split `paths` across three agents by
  slice family. They all edit `slices.json`, so each agent writes its
  slices' `paths` to a scratch fragment and one agent merges the fragments.
  Alternatively, run them sequentially: the file has one writer at a time.
- **Across stories:** after Phase 2, US1 (`e2e-parallel.mjs`,
  `package.json`), US2 (`e2e-slice.mjs`) and US3 (`check-e2e-slices.mjs`,
  `verify.mjs`, `.hooks/pre-commit`) touch different files. The exception
  is `package.json`: T009 and T017 both edit it, so serialize those two.
- **US5** (templates) is fully independent and can run at any time.
- **E2e runs never run in parallel** with each other or with edits: T010,
  T028, T033 and T035 each run alone.

### Parallel example: after Phase 2

```text
Agent A: T007 → T008 → T009 → T010           (US1: runner + scripts + proof)
Agent B: T011 → T012/T013/T014 → T015 → T016 (US2: coverage data + lookup)
Agent C: T019 → T020 → T021 → T022 → T023    (US3: the check; T024 after A and B land)
Agent D: T029, T030, T031                    (US5: templates)
```

Agent A's T010 is an e2e run. Agents B, C and D must not edit the tree
while it runs, or it must be scheduled after they finish.

## Implementation strategy

1. **MVP = Phases 1–3.** Every area can be proven by name in minutes.
   That alone ends the need for the full suite on a single-feature change.
2. **Then US3.** It keeps the list complete as specs are added. This is
   the difference between a list that exists and a list that stays true.
3. **Then US2.** The lookup turns "which slice?" from memory into an
   answer.
4. **Then US4, US5 and US6**, then the Phase 9 proof, which is the long
   run and goes last.

## Notes

- `[P]` means different files and no dependency on unfinished work.
- Record results under their tasks as indented bullets, in the same style
  as spec 044's T102.
- Commit per task, or per logical group, signed and serialized.
