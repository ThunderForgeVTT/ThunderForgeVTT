# Feature Specification: A Slice for Every Feature

**Feature Branch**: `060-a-slice-for-every-feature`

**Created**: 2026-09-22

**Status**: Draft

**Input**: Project owner, 2026-09-22, after spec 044's builder suite ran on
its own in under four minutes: "I want to add to the project constitution
feature or feature sets should be independently testable. Ideally to prevent
30m to 1 h gauntlets and they should have optimal integration slices." That
became constitution Principle VI (v1.3.0). This spec applies it to the
features that already exist: "Give every existing feature area an isolated e2e
slice … Every existing e2e spec belongs to at least one slice, with a check
that fails when a spec belongs to none … A developer or agent can list slices
and find which slice(s) cover a given file they changed … Record each slice's
measured duration."

## Context

On 2026-09-22 the end-to-end suite has **174 spec files** in `apps/web/e2e`,
plus the standalone hero builder's own suite. The last recorded per-spec
durations cover 141 of them and add up to **about 69 minutes** of serial
browser time. Split across shards, a full run takes 30 minutes to an hour.

The only feature that can be proven without that run today is the hero
builder (spec 044). It has a stack-free standalone suite (7 tests, 41 s) and
an integration slice (16 tests, 2.9 min). Everywhere else, anyone who wants
confidence in a change has two choices: build an `--only` list by hand from
memory, or run the whole gauntlet. The hand-built lists have already gone
wrong. Spec 044's own task text named e2e specs that did not exist
(`actor-claims`, `world-actor-permissions`, `collections-copy`).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Prove a change to one feature in minutes (Priority: P1)

A contributor or agent has changed one feature area, such as combat. They run
that area's slice by name. It stands up its own stack, runs only the specs
that cross the area's seams, and reports a result they can trust, in about
ten minutes instead of an hour.

**Why this priority**: This is the whole point of Principle VI. Without named
slices for the existing areas, the principle only binds new features. Every
change to the 174 existing specs' surfaces would still need the gauntlet.

**Independent Test**: Pick any slice, run it by its name with nothing else
running, and confirm it finishes on one shard in about ten minutes (see
SC-002). It must run only its listed specs and report pass, fail and skip
counts the same way the full suite does.

**Acceptance Scenarios**:

1. **Given** a clean tree and no other e2e run, **When** a contributor runs
   the combat slice by name, **Then** only the combat specs and their
   declared neighbours run, on one shard with a stack of their own, and the
   result names every spec that ran.
2. **Given** a slice that includes neighbour specs, **When** it runs, **Then**
   each neighbour passes without state left behind by specs outside the
   slice.
3. **Given** the hero builder's existing scripts, **When** the slices are
   introduced, **Then** `pnpm e2e:hero-builder` and its two parts keep
   working unchanged and appear in the slice list as one slice among many.

---

### User Story 2 - Find which slice covers what I changed (Priority: P1)

A contributor or agent has changed some files. They do not know which e2e
specs exercise those files. They ask which slices cover each changed file,
or their whole working diff. They get back the slices to run, and why each
was chosen.

**Why this priority**: A slice is only useful if people can find the right
one. The mistakes in spec 044's task text happened because "which specs cover
this?" was answered from memory.

**Independent Test**: Give the lookup a file from a known feature area, such
as the combat panel component. Confirm it names the combat slice. Give it a
file only the full suite covers, such as the schema. Confirm it says so
instead of naming nothing.

**Acceptance Scenarios**:

1. **Given** a changed file owned by one feature area, **When** a contributor
   asks which slices cover it, **Then** they are told that area's slice and
   the reason (the area owns the file, or the file is a surface the slice
   crosses).
2. **Given** a changed file that several areas touch, such as the actor page,
   **When** they ask, **Then** every slice that crosses it is named.
3. **Given** a changed file on a cross-cutting surface (the schema, auth, the
   e2e harness, shared UI primitives), **When** they ask, **Then** they are
   told the full suite is the gate for this change.
4. **Given** a whole working diff, **When** they ask about it, **Then** they
   get the union of slices for the changed files, with each slice listed
   once.
5. **Given** a changed file no slice claims and no cross-cutting rule covers,
   **When** they ask, **Then** they are told plainly that nothing covers it.
   The answer is never an empty success.

---

### User Story 3 - No spec falls out of every slice (Priority: P1)

Someone adds a new e2e spec and forgets to put it in a slice. The project's
standard verification fails and names the orphan spec. Nobody has to notice
it by chance.

**Why this priority**: Slices are useful only while they stay complete. A
spec in no slice runs only in the gauntlet, which is the problem Principle VI
exists to end. Without a check, slices would decay as fast as the hand-built
lists did.

**Independent Test**: Add an empty spec file whose name matches no slice and
run the standard verification. It fails and names the file. Add the file to a
slice, and verification passes.

**Acceptance Scenarios**:

1. **Given** every existing spec is in at least one slice, **When** the
   standard verification runs, **Then** the coverage check passes.
2. **Given** a new spec file in no slice, **When** verification runs,
   **Then** it fails and names the file, and the failure says how to add it
   to a slice.
3. **Given** a slice that names a spec file that no longer exists, **When**
   verification runs, **Then** it fails and names the stale entry, so a
   renamed or deleted spec cannot quietly shrink a slice.
4. **Given** a slice selects specs by a shared file prefix, **When** a new
   spec with that prefix is added, **Then** it joins the slice with no edit,
   and the coverage check counts it as covered.

---

### User Story 4 - Each slice's cost is on record (Priority: P2)

A contributor chooses what to run. The slice list shows each slice's
measured duration, how many specs it holds, and when it was last measured.
Slices that have grown past the target are visible without anyone having to
run them.

**Why this priority**: The target of about ten minutes only means something
if the durations are measured and visible. It comes after the slices exist.

**Independent Test**: Run a slice with measuring turned on. Confirm the list
then shows that slice's new duration and date. Confirm a slice over the target
is flagged.

**Acceptance Scenarios**:

1. **Given** a slice has been measured, **When** the slice list is shown,
   **Then** it gives that slice's duration, spec count and measurement date.
2. **Given** a slice whose last measured duration is over the target,
   **When** the list is shown, **Then** that slice is flagged as over target,
   so someone can decide to split it.
3. **Given** a slice never measured, **When** the list is shown, **Then** it
   says "not measured" instead of an estimate presented as a measurement.

---

### User Story 5 - New features are born with a slice (Priority: P2)

A contributor starts a new feature through the spec workflow. Its plan asks
which existing specs cross the feature's seams. Its tasks end with a proof
task that names the feature's slice and records the slice's result.

**Why this priority**: The retrofit covers what exists today. The template
change keeps every later feature from needing a retrofit of its own.

**Independent Test**: Generate a plan and tasks for a throwaway feature.
Confirm the plan's Constitution Check asks about neighbour specs for
Principle VI. Confirm the tasks' final proof task names an `e2e:<feature>`
slice and has a place to record its result.

**Acceptance Scenarios**:

1. **Given** a new plan, **When** its Constitution Check is filled in,
   **Then** it contains a Principle VI line asking which neighbouring specs
   cross the feature's seams and what the feature's slice is named.
2. **Given** new tasks, **When** they are generated, **Then** the proof task
   names the feature's slice, and a task exists to create or extend that
   slice.

---

### User Story 6 - The stack-free half, wherever a harness exists (Priority: P3)

For a feature area with a standalone harness, part of the slice runs with no
database, bucket or server. Today that area is the hero builder. The half is
listed as its own runnable part, like `e2e:hero-builder:standalone`.

**Why this priority**: Only the hero builder has a stack-free Playwright
suite today. The engine sandbox has no tests yet, and the packages' own tests
are unit tests that already run in the standard verification. This story
fixes the convention so the next harness fits it. It does not create suites
that do not exist yet.

**Independent Test**: List the slices. Confirm the hero builder's standalone
half is listed and runs with no stack up. Confirm areas with no harness list
no standalone half, not an empty one.

**Acceptance Scenarios**:

1. **Given** an area with a standalone harness suite, **When** its combined
   slice runs, **Then** the standalone half runs first. A failure there stops
   the run before any stack is started.
2. **Given** an area with no harness, **When** its slice is listed, **Then**
   it shows no standalone half.

---

### Edge Cases

- **A spec in several slices.** This is allowed and expected for shared
  surfaces. The actor page's specs are neighbours to combat, the hero builder
  and collections. The coverage check requires *at least* one slice, never
  exactly one.
- **A slice over target that cannot be split without losing a seam.** The
  slice stays whole, is flagged over target, and has the reason on record.
  Splitting a seam to hit a number would make the slice prove less. On
  record from the T035 proof: `canvas` (15m 30s, over the limit) keeps its
  `canvas-authoring` spec, because splitting it off would move the cold
  release wasm build that makes up most of that time rather than remove it,
  and would cost the authoring→board-loading seam.
- **Specs that need a first-run stack.** `instance-setup` and similar specs
  need an unseeded instance. The e2e runner already puts them in their own
  lane. A slice containing them must still run them correctly on one shard.
- **Measured (serial) specs.** Some specs are timed and must run alone. The
  runner refuses an `--only` list made only of them. A slice made only of
  measured specs must either be accepted by the runner or flagged when it is
  defined, never fail at run time.
- **The playtest suite.** `pnpm playtest` is a separate suite of scripted
  play sessions. It is not part of the gauntlet this spec slices, and it is
  out of scope. The coverage check counts only the e2e suite.
- **A spec that is flaky only inside a slice.** Such a spec usually depends on
  state another spec left behind. That is a defect in the spec, not in the
  slice. It is fixed where it lives, never by adding specs to the slice until
  it passes.
- **Two slices run at once.** Two e2e runs never run in parallel on one
  machine. The runner's existing run lock applies to slices too, and a second
  slice waits or refuses with the same message a second full run gets today.

## Requirements *(mandatory)*

### Functional Requirements

**Defining slices**

- **FR-001**: The project MUST have one declared list of feature slices, kept
  in the repository, that every tool in this spec reads. It MUST NOT
  duplicate a slice's membership between the list and the scripts that run
  it.
- **FR-002**: Each slice MUST declare: a name; the feature area and the specs
  in `specs/` it serves; its own e2e specs, by explicit file or by shared
  file prefix; its neighbour specs, each with the seam it crosses; the source
  paths it owns or crosses, for the lookup in FR-010; and, where one exists,
  its standalone harness suite.
- **FR-003**: The slice list MUST be derived from the real specs and feature
  history, not from the example areas in the request. The expected areas
  include combat, content collections, the compendium, actors, maps and
  canvas authoring, tokens, lighting and vision, interactive objects, status
  displays, play and pause, the world cache, lore, the library and book
  import, accounts and two-factor, instance administration and moderation,
  feedback, companion, and the Genie system. The final grouping is decided in
  planning.
- **FR-004**: Each slice MUST be the smallest set of specs that crosses every
  seam its feature touches: its own specs plus the neighbour specs of each
  existing surface it changes. Every neighbour entry MUST record the seam
  that justifies it.
- **FR-005**: Each slice SHOULD finish in about ten minutes on one shard. A
  slice over that target MUST either be split along a seam or carry a
  recorded reason why it cannot be.

**Running slices**

- **FR-006**: Each slice MUST be runnable by name as root scripts following
  the spec 044 convention: `e2e:<slice>:integration` (one shard, its own
  stack), `e2e:<slice>:standalone` where a harness suite exists, and
  `e2e:<slice>` running both, standalone first.
- **FR-007**: A slice run MUST use the existing e2e runner and inherit its
  guarantees: the run lock, the first-run lane, the auth rate-limit bypass,
  per-spec durations, and the same pass, fail, flaky and skip totals and
  failure markers as a full run.
- **FR-008**: The hero builder's existing three scripts MUST keep their names
  and behaviour, and the hero builder MUST appear in the declared list.
- **FR-009**: A contributor MUST be able to list every slice with its spec
  count, its last measured duration and measurement date, and whether it is
  over target.

**Finding the right slice**

- **FR-010**: A contributor MUST be able to ask which slices cover a given
  file, or every file in the working diff against a named base. The answer
  MUST name each covering slice once, with the reason it was chosen. A
  changed e2e spec file is covered by every slice that lists it.
- **FR-011**: Changes to cross-cutting surfaces MUST be answered with "the
  full suite", never with a slice. These are the GraphQL schema and
  generated bindings, authentication and authorization, the e2e harness and
  its fixtures, database migrations, and shared UI primitives. The
  cross-cutting list is declared beside the slices.
- **FR-012**: A file that no slice and no cross-cutting rule covers MUST be
  reported as uncovered, never as a silent empty answer.

**Keeping slices complete**

- **FR-013**: The standard verification (`pnpm verify`) MUST include a
  coverage check that fails when any e2e spec file belongs to no slice, and
  names each such file.
- **FR-014**: The same check MUST fail when a slice names a spec file,
  prefix, owned path or standalone suite that matches nothing, and name
  that entry.
- **FR-015**: The coverage check MUST run without a stack and in well under
  a second's worth of the verify budget. It MUST read the declared list and
  the file tree, never run a browser.

**Recording cost**

- **FR-016**: Each slice's duration MUST be measured by running it, and the
  measurement MUST be recorded with its date, where the listing reads it.
  The e2e runner's per-spec durations MAY be summed to give an estimate, but
  an estimate MUST be labelled as one.
- **FR-017**: This feature's proof MUST run every slice at least once,
  sequentially with nothing else running, and record each measured duration
  and result.

**Templates**

- **FR-018**: The Spec Kit plan template's Constitution Check MUST include a
  Principle VI item asking what the feature's slice is called, which neighbour
  specs cross its seams, and whether the feature adds a surface an existing
  slice must now cross.
- **FR-019**: The Spec Kit tasks template MUST include a task to create or
  extend the feature's slice. Its final proof task MUST name the slice and
  record the slice's result, replacing hand-built `--only` lists.

**Scope**

- **FR-020**: The full suite MUST remain unchanged and runnable exactly as
  today. It stays the check before a release and for cross-cutting changes.
- **FR-021**: No existing e2e spec may be deleted, skipped or weakened to
  make a slice fit its target or pass in isolation. A spec that fails only
  in isolation is fixed in the spec.

### Key Entities

- **Slice**: a named, runnable subset of the e2e suite that proves one
  feature area. It has its own specs, neighbour specs with their seams, the
  source paths it owns or crosses, and an optional standalone harness suite.
  It belongs to one or more `specs/` features.
- **Seam**: an existing surface a feature area changes or depends on, such as
  the actor page, the compendium row or the scene canvas. Each seam is
  proven by a named neighbour spec.
- **Cross-cutting surface**: a path whose change is gated by the full suite,
  not a slice.
- **Slice measurement**: a slice's duration on one shard, its spec count, its
  date and its result.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of the e2e suite's spec files belong to at least one
  slice on the day this ships, and adding an orphan spec fails verification
  within the normal verify run.
- **SC-002**: Every slice finishes in 12 minutes or less on one shard, with
  nothing else running, except any slice with a recorded reason it cannot be
  split. At least 80% of slices finish in 10 minutes or less.
- **SC-003**: For a change confined to one feature area, a contributor gets
  a trustworthy e2e result in about a sixth of the time a full run takes
  (about ten minutes against 30 to 60).
- **SC-004**: For any changed file in the repository, a contributor learns
  which slices to run, or that the full suite is required, in one command
  and under five seconds.
- **SC-005**: Every slice passes when run on its own on the day this ships.
  No spec is skipped or removed to get there (FR-021).
- **SC-006**: The next feature specified after this one names its slice in
  its plan and its proof task without anyone prompting it.

## Assumptions

- **The e2e runner is kept, not replaced.** Slices are named `--only` lists
  run through `scripts/e2e-parallel.mjs`. The runner's lanes, lock, rate-limit
  bypass and durations are reused as they are. Any change the runner needs
  (for example, reading the declared list directly) is small and keeps
  `--only` working.
- **One shard is the unit of measure.** The target of about ten minutes is on
  one shard, with nothing else running, on the owner's development machine.
  That machine's per-spec durations are the only measurements that exist.
  There is no CI yet.
- **Standalone halves exist only where a harness suite exists.** Today that
  is the hero builder. Writing a Playwright suite for the engine sandbox is
  out of scope. When one exists, it joins through the declared list.
- **Package unit tests are not slices.** They already run in `pnpm verify`
  and are fast. This spec covers only the browser e2e suite.
- **The playtest suite is out of scope** (see Edge Cases).
- **Durations are measured, not assumed.** The per-spec record from
  2026-09-14 (141 specs, about 69 minutes serial) is used only to draft the
  groupings. Every recorded slice duration comes from running that slice.
- **Overlap is acceptable.** Summed over all slices, the time is expected to
  exceed a full run, because neighbours repeat. A slice is sized for the one
  change being proven, never for running every slice back to back.
