# Research: A Slice for Every Feature

Phase 0 for [plan.md](plan.md). The Technical Context had no open
questions. The decisions below record how each part of the spec is met and
what was rejected.

## R1. Where the slice list lives

**Decision**: The list is one hand-edited JSON file, `scripts/e2e/slices.json`,
next to the runner's other modules. It holds the slices, and in the same
file the cross-cutting rules (FR-011).

**Rationale**:

- FR-001 asks for one declared list. The runner, the lookup, the check and
  the `package.json` scripts all need it.
- JSON can be read by any tool, including an agent with nothing but `jq`,
  without executing code.
- A diff to the file reads as a membership change and nothing else.
- Keeping the cross-cutting rules in the same file means the lookup has one
  input.

**Alternatives considered**:

- *A JS module exporting the list.* This allows comments and computed
  prefixes, but an agent must run it to read it, and code tends to grow in
  it. Rationale is carried by a `why` string per neighbour instead (see the
  data model), and that string is what the listing prints.
- *Per-spec annotations* (a `// @slice combat` header in each spec). This
  spreads membership across 181 files. It also cannot express neighbours:
  a spec would have to name every slice that borrows it.
- *Membership kept in `package.json` scripts, as today's hero-builder
  `--only` does.* This is the duplication FR-001 forbids once the lookup and
  the check need the same data.

## R2. How a slice names its specs

**Decision**: A slice has `own` entries and `neighbours` entries.

- `own` entries are exact file names (`combat-panel.spec.ts`) or file-name
  prefixes (`combat-`). The prefix form means a new `combat-foo.spec.ts`
  joins without an edit, as Principle VI asks.
- `neighbours` are exact file names only, each with a `seam` sentence.
- Every spec has exactly **one owning slice** and may be a neighbour in any
  number of slices.
- When two `own` entries match the same file, the more specific one wins:
  an exact name beats a prefix, and a longer prefix beats a shorter one.
  Example: `scene-lighting.spec.ts` is owned by *lighting* through its exact
  name, even though *scenes* owns the `scene-` prefix.
- A tie between two slices at equal specificity is a check failure.

**Rationale**:

- Single ownership is what makes the lookup's answer ("your file's feature
  is *combat*; also run *status*, which borrows it") and the orphan check
  well defined.
- Neighbours by exact name keep a slice from quietly growing when an
  unrelated spec happens to share a prefix.

**Alternatives considered**:

- *Globs over spec paths.* More power than 181 flat files need, and a glob
  typo silently selects nothing. The check does catch an entry that matches
  nothing, but prefixes make that mistake less likely to begin with.
- *Allowing multiple owners.* The lookup would then be ambiguous. The
  existing overlaps (`scene-lighting`, `scene-preload`,
  `interactive-lighting`) all have one obvious home.

## R3. How the runner runs a slice

**Decision**:

- `scripts/e2e-parallel.mjs` gains `--slice=<name>`. The runner imports
  `resolveSlice(name)` from `scripts/e2e/slices.mjs`, which returns the
  slice's own and neighbour specs as exact relative paths.
- Those paths replace the result of `allSpecFiles()` for that run, as
  `--only` filtering does today. Lane assignment (first-run, measured,
  GitHub apps), the run lock, the bypass and reporting then proceed
  unchanged.
- `--slice` and `--only` together is an error.
- An unknown slice name exits 2 and lists the valid names.
- The slice scripts pass `--shards=1`.

**Rationale**:

- The runner already knows how to run a subset.
- Resolving to exact paths avoids a trap in `--only`, which matches a
  substring anywhere in the path. For example, `--only=lighting` selects
  `engine-lighting-limits` and so pulls a release build into what was meant
  to be a quick run. A slice runs exactly the files it names.
- Putting resolution in a module keeps a 1,464-line runner from growing
  logic it only consumes.

**Alternatives considered**:

- *Generating `--only=a,b,c` lists into `package.json`.* This duplicates
  membership, the drift FR-001 forbids.
- *A separate slice runner that spawns the parallel runner.* A second
  process owns the lock, and two places decide how lanes work.

## R4. Root `package.json` scripts

**Decision**:

- Each slice gets `e2e:<slice>`.
- Each slice gets `e2e:<slice>:integration`, whose body is exactly
  `node ./scripts/e2e-parallel.mjs --shards=1 --slice=<slice>`.
- A slice with a standalone suite also gets `e2e:<slice>:standalone`.
- `e2e:<slice>` is then `pnpm run e2e:<slice>:standalone && pnpm run e2e:<slice>:integration`.
  Without a standalone suite it is an alias of the integration script.
- The scripts are written by hand once. The coverage check fails if any is
  missing, extra, or has a body other than the canonical one.
- `node scripts/check-e2e-slices.mjs --fix` rewrites them, following the
  `verify.mjs --fix` idiom.
- Two further scripts: `e2e:slices` runs `node ./scripts/e2e-slice.mjs list`,
  and `e2e:which` runs `node ./scripts/e2e-slice.mjs which`.
- The hero-builder's three scripts keep their names (FR-008). Only the
  integration body changes, from `--only=hero-builder-,actor-art` to
  `--slice=hero-builder`. That is the same four files, now resolved exactly.

**Rationale**:

- The scripts carry only the slice name, so membership stays in one place.
- The check makes a hand edit that drifts fail at commit time instead of
  at run time.

**Alternatives considered**:

- *No per-slice scripts, only `pnpm e2e:slice <name>`.* This breaks the
  `e2e:<feature>` convention that Principle VI and FR-006 name.
- *Generating the scripts at install time.* This means a hidden rewrite of
  a tracked file.

## R5. The lookup

**Decision**: `e2e-slice.mjs which` takes either paths or a diff:

- paths as arguments;
- `--diff`, meaning `git diff --name-only <base>...HEAD` plus uncommitted
  and untracked changes, with `<base>` defaulting to `origin/main`;
- `--staged`.

For each path it answers in this order:

1. **Cross-cutting**: the path matches a cross-cutting rule, so the answer
   is "full suite", with the rule's reason.
2. **A spec file**: the answer is its owner, plus every slice that borrows
   it as a neighbour.
3. **Source**: the answer is every slice whose `paths` globs match.
4. **Uncovered**: the path is reported explicitly (FR-012).

At the end it prints one command that runs the union of named slices, or
says "run the full suite" if any path was cross-cutting. Exit codes:

- 0: every path is covered;
- 3: something is uncovered (still printing the rest);
- 2: usage error.

Documentation, specs and `.md` files match a built-in "no e2e needed" rule,
so a docs-only diff exits 0 and says so.

**Rationale**:

- An agent needs a machine-usable answer and a human needs a readable one.
  `--json` gives the former; the default gives the latter.
- Exit 3 lets a hook or an agent treat "uncovered" as a question, not a
  failure of the lookup.

**Alternatives considered**:

- *Deriving coverage from import graphs or V8 coverage.* This is precise,
  but it needs a full run to build and goes stale. Declared `paths` globs
  are cheap and reviewable, and the check keeps them honest (R7).

## R6. Measured, first-run and GitHub lanes inside a slice

**Decision**: The lanes are left alone. The runner assigns lanes per spec
file, so they already work on any subset.

- A slice that contains a measured-lane spec runs it in the measured lane,
  with the release engine build, serially.
- A slice that contains `instance-setup` runs it in the first-run lane.
- A slice that contains `github-apps` or `lore-repository-sync` runs it in
  the GitHub-apps lane.

Consequences:

- *canvas* includes `canvas-authoring`, which triggers a release build in
  that slice. Its measured duration will show whether that fits.
- The two engine slices are measured-only. The runner's existing warning,
  "`--only` matched only measured specs", is kept and reworded for
  `--slice`. The check does not fail on a measured-only slice; the listing
  labels it.

**Rationale**: Lanes exist because those specs cannot share a stack with
the others. A slice that bypassed lanes would be a different test.

**Alternatives considered**:

- *Moving `canvas-authoring` into an engine slice.* It owns canvas
  behaviour, and a canvas change must run it. Its cost is a measurement to
  take, not a reason to misfile it.

## R7. The coverage check

**Decision**: `scripts/check-e2e-slices.mjs` runs as verify step `e2e-slices`
and is added to the pre-commit ids. It fails on:

1. a spec under `apps/web/e2e` (the set `allSpecFiles()` walks, so
   including `torture/` and excluding `journeys/`) that no slice owns;
2. an `own` exact name or neighbour that names no existing spec;
3. an `own` prefix that matches no spec;
4. a `paths` or cross-cutting glob that matches no tracked file (from
   `git ls-files`);
5. an ownership tie (R2);
6. a missing, extra or non-canonical `e2e:<slice>*` script (R4);
7. a slice with a `standalone` entry whose package script does not exist;
8. a slice name that is not kebab-case, or that collides with a reserved
   script (`e2e:slices`, `e2e:which`);
9. a `durations` record for a slice that no longer exists.

Each failure names the file and the fix. It runs in well under a second.

**Rationale**: This is FR-013 and FR-014. It belongs in pre-commit because
its cost is flat, which is the criterion `verify.mjs` documents for that
group.

**Alternatives considered**:

- *Running the check only in `pnpm verify`.* A new spec committed without
  a slice would then survive until the next push, and the pre-push hook
  runs all steps anyway. Pre-commit costs under a second.

## R8. The slice grouping

**Decision**: 27 slices. The estimates below are sums of recorded per-spec
durations from `.e2e-shards-durations.local.json`. They are **labelled
estimates**: they exclude stack start and engine build, which only the
proof measures (FR-016).

| Slice | Own specs | Neighbours (seam) | Est. spec time |
|---|---|---|---|
| hero-builder | `hero-builder-` (3) | actor-art (the actor imagery the builder writes) | 2.8 min |
| actors | `actor-` (6), npc-visibility, players-section, `play-field-` (2) | hero-builder-saved (stored look), token-links (actor↔token link) | 5.1 min |
| combat | `combat-` (9), dice-roll, roll-check | token-attributes (HP on tokens), status-display (conditions) | 5.3 min |
| tokens | `token-` minus walls (6), look-at-and-follow, item-pickup-race | token-movement-walls (walls), world-session-tokens (session) | 6.3 min |
| canvas | canvas-authoring†, `canvas-` (2), map-editor-tooling, drawn-walls-block, token-movement-walls, board-loading, screenshot | scene-management (scene switch) | 6.3 min |
| scenes | `scene-` (5), player-active-scene, gm-staging-page, world-staging-route | play-pause (scene launch under pause) | 4.0 min |
| lighting | carried-light, darkvision-range, light-reach, scene-lighting, interactive-lighting | drawn-walls-block (occlusion) | 2.3 min |
| interactive | `interactive-` minus lighting (7) | token-movement-walls (doors), scene-exploration (reveal) | 1.8 min |
| status | `status-` (8) | combat-hit-points (HP state) | 1.9 min |
| play-pause | `play-pause` (5) | world-event-catchup (catch-up), live-sync (broadcast) | 7.2 min |
| world-cache-core | world-cache-isolated, -offline, -peer, -multitab | — | 9.1 min |
| world-cache | world-cache, -budget, -diagnostics, -permissions, -prefetch, -repair, -storage-ui, scene-preload | — | 4.7 min |
| compendium | `abilities-` (3), world-compendium | combat-attack (abilities used in play) | 2.7 min |
| collections | content-collections, content-origin, `collection-` (2), `library-` (4), account-library, sharing-attestation, singleton-share-anonymous-access, access-links, publishing-gate | — | 3.6 min |
| book-import | `book-import-` (2), pdf-reader | library-book-list (the library) | 0.6 min |
| lore | lore-wiki, lore-repository-sync‡ | — | 1.2 min |
| accounts | `two-factor` (5), oauth-provider, auth-providers, onboarding-flow, anonymous-shell, concurrent-sessions, user-data-export, account-standing, invite-membership | — | 5.4 min |
| instance | `instance-` (6, incl. instance-setup§), admin-instance-panels, mail-delivery, github-apps‡ | — | 0.7 min + setup |
| moderation | `dmca-` (3), takedown-reach, legal-enquiries | collection-moderation (reports on collections) | 2.3 min |
| feedback | `feedback` (6) | — | 3.1 min |
| companion | `companion-` (3), chat-panel, session-notes | live-sync (broadcast) | 1.1 min |
| genie | `genie-` (14) | system-panel-slots (system panel host) | 2.5 min |
| game-systems | `system-` (3) | status-systems (per-system status) | 1.4 min |
| worlds | worlds-list, world-appearance, world-dashboard-statistics, world-event-catchup, world-session-tokens, live-sync, demo-first-session | — | 1.9 min |
| engine-limits | engine-limits†, engine-status-limits† | — | 9.9 min |
| engine-other | engine-interaction-limits†, engine-lighting-limits†, engine-loading†, engine-monitor | — | 3.4 min |
| torture | `torture/` (7) | — | 0.7 min |

† measured lane · ‡ GitHub-apps lane · § first-run lane

Notes:

- **Every spec has one owner.** The draft grouping was run as a script
  against the 181 files. It left one orphan (`canvas-authoring`, now in
  *canvas*) and three prefix overlaps (`scene-lighting`, `scene-preload`,
  `interactive-lighting`). Each overlap is settled by an exact-name entry
  under R2.
- **Unmeasured specs.** Seven specs have no recorded duration (board-loading,
  drawn-walls-block, look-at-and-follow, genie-play-dock-session,
  genie-sheet-layout, worlds-list, world-dashboard-statistics) and
  instance-setup. The listing says "not measured" for these until the
  proof records them.
- **Splits.**
  - *world-cache* is split because world-cache-isolated alone is about
    5.8 minutes. The four specs that exercise isolation, offline, peer and
    multitab behaviour together make the "core" slice.
  - *engine* is split so the two largest perf specs (about 5 minutes each)
    get a slice to themselves.
- **Neighbours are few by design.** Principle VI asks for the smallest set
  that crosses each seam. A neighbour is added only where the slice's own
  code writes to a surface that another spec asserts. Examples: the builder
  writes actor imagery; combat writes HP that tokens and status show.
- **Tiny slices are kept.** *book-import*, *instance* and *torture* are
  small, and their wall time is dominated by stack start. Merging them
  would make "which slice covers my change" less precise. That precision
  is worth more than a minute of startup.

**Alternatives considered**:

- *One slice per spec directory from `specs/`.* The 59 spec directories
  do not map one to one onto e2e files. Many directories have no e2e, and
  some e2e files span several directories.
- *Balancing slices to equal duration.* This optimises the wrong thing: a
  slice is chosen by what a change touches, not by how long it takes.

## R9. Unit tests for the tooling

**Decision**: Use `node --test` in `scripts/e2e/__tests__/slices.test.mjs`
for the resolver, specificity, lookup ordering, the check's failure cases
and the `--only` regression. It runs as part of the `e2e-slices` verify step
(`node --test` finishes in about 100 ms), so no new step is needed.

**Rationale**: `scripts/` has no package and no test runner. `node:test` is
built in and needs no install, and a harness script should not depend on
the web app's vitest.

**Alternatives considered**:

- *vitest from `apps/web`.* This couples root tooling to one app's
  devDependencies.
- *No unit tests, relying on the check running against the real list.*
  The failure paths would then never run.

## R10. Recording measured durations

**Decision**:

- When `--slice=<name> --record-durations` completes, the runner writes an
  entry to `scripts/e2e/slice-durations.json` keyed by slice name:
  `{ wallSeconds, specs, passed, failed, flaky, skipped, measuredAt (ISO date), commit (short sha) }`.
- It uses the whole run's wall clock, from lock acquired to report written,
  so stack start and engine build are included. That is what a contributor
  waits for.
- A failed run is recorded with its result, so a red slice is visible
  rather than silently keeping an old green time.
- `--record-durations` keeps its existing per-spec behaviour as well.
- `e2e-slice.mjs list` prints the measured time and date. Where nothing is
  recorded it prints the per-spec sum marked `est.`. Over 10 minutes is
  flagged `over target`, and over 12 is flagged `over limit`.
- The proof runs every slice as
  `pnpm e2e:<slice>:integration -- --record-durations`, one at a time. The
  memory note about e2e with the rate-limit bypass and `--workers=1` is
  already handled by the runner.

**Rationale**:

- FR-016 asks for measured durations with a date.
- A tracked file makes the numbers reviewable in the commit that changes
  them.
- This is a separate file from `.e2e-shards-durations.json`, so feature
  commits never drag in shard rebalancing (the standing rule).

**Alternatives considered**:

- *Deriving slice times from the per-spec file.* This misses stack start,
  which dominates small slices, and the per-spec file is the one that must
  not be committed with feature work.
