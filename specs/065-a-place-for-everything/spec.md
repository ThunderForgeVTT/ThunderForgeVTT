# Feature Specification: A Place for Everything

**Feature Branch**: `065-a-place-for-everything`

**Created**: 2026-10-04

**Status**: Draft

**Input**: Project owner, 2026-10-04, after asking how closely the repository
follows their own global design rules: "lets chase down the repo structure i
think thats the biggest thing that makes this repo go from a pet project to a
professional one and worthy of a full spec." The rules in question: entry
points live in `./apps/*` and are thin; reusable Rust libraries live in
`./crates/*`; shared web code and core business logic live in `./packages/*`;
replacing one app with another should need minimal rewriting.

## Context

Measured on 2026-10-04, at commit `7ba41b3b`.

The repository already follows the rule in most places. `crates/` holds 18
library crates. `apps/` holds three web entry points. `packages/` exists. The
workspace files (`Cargo.toml`, `pnpm-workspace.yaml`) are in place.

Two things do not follow it.

**Four Rust crates live under `src/`, outside both homes.** They are the four
that matter most:

| Today        | What it is                                    | Size                |
| ------------ | --------------------------------------------- | ------------------- |
| `src/app`    | The server binary (`thunderforge`) and a schema printer | 7 files, 1,847 lines |
| `src/server` | A library: every resolver, model and migration | 460 files, 171,181 lines |
| `src/core`   | A library shared by server and engine         | 12 files, 1,394 lines |
| `src/engine` | A library compiled to WebAssembly             | 95 files, 25,205 lines |

So the largest library in the project is not in `crates/`, and the only
server binary is not in `apps/`. A newcomer who has read the rule cannot find
the server by following it.

**`apps/web` is not thin.** It holds 653 TypeScript files and 109,275 lines.
`packages/` holds two packages with 45 files between them. The API client,
the world store and its sync, the design system and the system-panel registry
all live inside the app, so a second entry point (the hero builder, the
engine sandbox, a future companion app) reaches into `apps/web` or copies
from it.

The first is a move. The second is an extraction, and a much larger one.
This spec covers both, in that order, and each story stands alone.

The old paths are written down in many places. Outside the crates themselves,
147 files name `src/server`, 82 name `src/engine`, 17 name `src/app` and 5
name `src/core`. That includes the build (`Makefile`, `Dockerfile`,
`diesel.toml`, `.cargo/config.toml`), 16 scripts, and dozens of browser specs
that cite a server file in a comment. The specs and ADRs under `specs/` and
`docs/adrs/` name the old paths several hundred times more.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Find any Rust code by the rule alone (Priority: P1)

A contributor who knows only the rule — binaries in `apps/`, libraries in
`crates/`, game systems in `packs/` — looks for the server binary, the server
library, the shared models and the engine. Each is where the rule says. No
Rust crate lives anywhere else.

**Why this priority**: This is the gap the owner named. It is also the cheap
half: nothing about how the code behaves changes, only where it sits.

**Independent Test**: List every workspace member. Each path starts with
`apps/`, `crates/` or `packs/`. Build, lint and test the workspace from a
clean checkout and get the same results as before the move.

**Acceptance Scenarios**:

1. **Given** a clean checkout after the move, **When** a contributor lists the
   workspace members, **Then** none is under `src/`, and `src/` no longer
   exists at the repository root.
2. **Given** the same checkout, **When** they run the documented build, lint
   and test commands, **Then** each passes with the same counts as on the
   commit before the move.
3. **Given** a file that moved, **When** they ask version control for its
   history, **Then** the history before the move is still attached to it.
4. **Given** a database from before the move, **When** the server starts,
   **Then** it runs no migration again and reports the same schema version.

---

### User Story 2 - The layout cannot quietly drift back (Priority: P1)

Someone adds a crate, or a web package, in the wrong place. The check that
already runs before every commit refuses it and says where it belongs.

**Why this priority**: The move is worth little if the next crate lands under
`src/` again. Every structural rule this project has kept is one a check
enforces (file length, e2e slices, the system registry).

**Independent Test**: Add a throwaway workspace member outside the allowed
roots and try to commit. The commit is refused with a message naming the
member and the allowed roots.

**Acceptance Scenarios**:

1. **Given** a new Rust crate added outside `apps/`, `crates/` and `packs/`,
   **When** a commit is attempted, **Then** it is refused and the message
   names the crate and where it may live.
2. **Given** a crate under `apps/` that declares no binary, or a crate under
   `crates/` that declares one, **When** a commit is attempted, **Then** it is
   refused with the reason. A crate that needs an exception is listed by name
   in the check, with the reason beside it.
3. **Given** a tree that follows the rule, **When** the check runs, **Then**
   it passes and reports how many members it looked at.

---

### User Story 3 - Living documents point at paths that exist (Priority: P2)

A contributor or agent follows a path from `AGENTS.md`, the constitution, the
contributing guide or a code comment, and the file is there.

**Why this priority**: Agents work from `AGENTS.md` and the constitution on
every session. A wrong path there costs time on every task. Past specs and
ADRs are different: they record what was true when they were written.

**Independent Test**: Search the living documents and the source for the four
old paths and find none. Search `specs/` and `docs/adrs/` and find them
unchanged, with one note at the top of each index saying how to translate.

**Acceptance Scenarios**:

1. **Given** the move has landed, **When** the living documents, build files,
   scripts, source comments and browser specs are searched for the old paths,
   **Then** none is found.
2. **Given** a past spec or ADR, **When** it is read, **Then** its text is
   unchanged, and the index it belongs to carries one note mapping old paths
   to new.

---

### User Story 4 - A second web entry point borrows nothing from the first (Priority: P2)

A contributor builds a new entry point — or maintains the hero builder or the
engine sandbox — and gets the API client, the world store, the design system
and the game-system panels from `packages/`. Nothing under `apps/` imports
from another app.

**Why this priority**: This is what "thin apps" means for the web, and it is
the larger piece of work. It comes after the Rust move because it changes far
more files and benefits from the check in Story 2.

**Independent Test**: Search every app for an import that reaches into
another app and find none. Build each app alone and pass its own slice.

**Acceptance Scenarios**:

1. **Given** the extraction has landed, **When** any file under one app is
   searched for imports from another app, **Then** none is found, and the
   pre-commit check refuses a new one.
2. **Given** an extracted package, **When** it is built and tested with no app
   present, **Then** it passes on its own.
3. **Given** `apps/web` after the extraction, **When** its remaining source is
   measured, **Then** it holds routing, pages and composition, and its line
   count is recorded beside the 109,275 it started from.

---

### Edge Cases

- **Work in flight.** A branch made before the move that touches a moved file
  must be merged first or rebased across it. On 2026-10-04 the four other
  worktrees (`063-claim-grants-editor`, `token-viewport-culling`,
  `scene-levels`, `rfs-journey`) held nothing that `main` did not, so the
  move lands as one commit of pure renames plus the path updates.
- **Migrations.** The migrations directory moves with the server library. The
  recorded migration versions must not change, or a live database would be
  asked to run them again.
- **Generated output.** Paths baked into generated files (the printed GraphQL
  schema, generated TypeScript types, the engine build under `dist/`) are
  regenerated, not hand-edited.
- **Package names.** Crate and binary names stay as they are. Only directories
  move, so no `use` statement, import or installed command changes.
- **Game systems.** `packs/` stays where it is. A system pack is content with
  a server crate, an engine crate and a web package inside it; it is neither
  an app nor a general library, and the rule gains it as a third named home.
- **A half-finished move.** If a crate has moved and the build files have not,
  the tree does not build. The moves and their path updates land together.

## Requirements *(mandatory)*

### Functional Requirements

**The Rust move**

- **FR-001**: Every Rust workspace member MUST live under `apps/`, `crates/`
  or `packs/`. The `src/` directory at the repository root MUST no longer
  exist.
- **FR-002**: The four crates MUST move as follows, keeping their package and
  binary names:

  | From         | To                          |
  | ------------ | --------------------------- |
  | `src/app`    | `apps/server`               |
  | `src/server` | `crates/thunderforge-server` |
  | `src/core`   | `crates/thunderforge-core`  |
  | `src/engine` | `crates/thunderforge-engine` |

- **FR-003**: The move MUST preserve each file's version-control history.
- **FR-004**: The move MUST change no behaviour. No resolver, schema, route,
  migration or rendered output may differ.
- **FR-005**: Database migrations MUST move with the server library and MUST
  keep their recorded versions.
- **FR-006**: Every build file, script, hook and container definition that
  names an old path MUST be updated in the same change as the crate it names.

**The guard**

- **FR-007**: A check that runs before every commit MUST refuse a workspace
  member outside the allowed roots, and MUST name the member and the roots.
- **FR-008**: The same check MUST refuse a crate under `apps/` with no binary
  and a crate under `crates/` with one, except those listed by name with a
  written reason.
- **FR-009**: The check MUST refuse an import from one app into another.

**The documents**

- **FR-010**: `AGENTS.md`, the constitution, the contributing guide, the
  README, source comments and browser specs MUST name only paths that exist.
- **FR-011**: Past specs and ADRs MUST NOT be rewritten. Their indexes MUST
  carry one note mapping the old paths to the new.
- **FR-012**: The constitution MUST state the layout rule, including `packs/`
  as the home for game systems, so the rule binds future work.

**The web extraction**

- **FR-013**: Code that more than one entry point needs MUST live in
  `packages/`: at least the API client, the world store and its sync, the
  design system, and the game-system panel registry.
- **FR-014**: Each extracted package MUST build and pass its own tests with no
  app present.
- **FR-015**: No app may import from another app.
- **FR-016**: The extraction MUST change no behaviour visible in a browser.

### Key Entities

- **Entry point**: something a person runs — a server binary, a web app. It
  lives in `apps/` and composes libraries.
- **Library**: reusable code with no entry point of its own. Rust libraries
  live in `crates/`, web libraries in `packages/`.
- **Game system**: one ruleset's server, engine and web parts together, under
  `packs/systems/<id>/`.
- **Living document**: a document read to decide what to do now. Kept current.
- **Record**: a past spec or ADR. Left as written.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Zero Rust workspace members live outside `apps/`, `crates/` and
  `packs/`, down from four.
- **SC-002**: The Rust test count and the full browser suite's pass count are
  the same on the commit before the Rust move and the commit after it.
- **SC-003**: Zero files outside `specs/` and `docs/adrs/` name any of the
  four old paths, down from 147, 82, 17 and 5 files for `src/server`,
  `src/engine`, `src/app` and `src/core`.
- **SC-004**: A crate or package added in the wrong place is refused before it
  can be committed, shown by a test of the check itself.
- **SC-005**: Zero imports cross from one app into another.
- **SC-006**: After the extraction, each app builds alone and each extracted
  package passes its tests alone.
- **SC-007**: The line count of `apps/web` after the extraction is recorded
  beside the 109,275 it started from. No target is set in advance; the number
  is measured, not promised.

## Assumptions

- The project is unreleased, so nothing outside this repository depends on
  the old paths.
- Crate and binary names do not change. Renaming the `thunderforge` binary or
  any library is out of scope.
- The licence (AGPL-3.0-or-later, chosen 2026-08-25) is a deliberate decision
  and is not revisited here, although the owner's general default is a
  permissive licence.
- Stories 1 to 3 land before the first field test only if they can be done
  without holding up play-critical work; Story 4 is expected to land after
  it. The stories are ordered so stopping after any one leaves the repository
  better than it was.
- The constitution asks for an ADR before architecturally significant work.
  This restructuring needs one, written during planning, recording the three
  homes and the reason `packs/` is its own. It is
  [ADR-111](../../docs/adrs/20261004-111-four_homes_and_why_packs_is_its_own.md),
  which also fixes Story 4 after the first field test.
- No feature flag applies: a directory layout cannot be switched at runtime.
- Code splitting of the web bundle and the feature-flag practice are separate
  pieces of work and are not part of this spec.
