# Feature Specification: One Way to Be a Pack

**Feature Branch**: `066-one-way-to-be-a-pack`

**Created**: 2026-10-04

**Status**: Draft

**Input**: Project owner, 2026-10-04, after an audit of the system packs:
"lets chase that down by adding it into a packs overhaul spec to ensure things
are all in line and we have a rock solid foundation then task and implement
it." The audit was prompted by their question: is there a core pack each pack
is based on, and an API extensible enough for fully custom packs?

## Context

Measured on 2026-10-04, at commit `4a45504d`.

There is no core pack, and there should not be one. The platform is the base:
`packs/systems/README.md` is the author's contract,
`crates/pack_system_spec` validates a manifest, and the base interface pack
draws a sheet from `system.json` alone. A pack that is only a manifest is a
working system. Spec 032 built that, and ADR-029 and ADR-062 decided its
edges: a pack from outside the product is data, a bundled pack may also carry
server and web code, and no pack extends the engine with code.

That foundation is sound. What sits on it is not all in line with it. Eight
systems were written over five months, the early ones by copying the one
before, and the copies kept parts the product stopped reading.

| What | Where | Size | Who reads it |
| ---- | ----- | ---- | ------------ |
| An engine crate | 7 packs (all but Roll for Shoes) | 1,091 lines, 7 workspace members | Nothing. No crate depends on one, so none reaches the browser. ADR-062 says none should. |
| A web package with no discovered entry | Blades, Cypher, Fate, Pathfinder, Year Zero | 1,804 lines, 5 pnpm packages | Nothing. The host finds `ActorSheet.tsx`, `StatBlocks.ts` and `panels/*.tsx`; these ship `components/CharacterSheet.tsx` and an `index.ts`. Their sheets are drawn from the manifest. |
| Web files no discovered entry imports | 5e | 989 lines: `index.ts`, `schema.ts` (an RxDB schema), four components | Nothing. 5e's `ActorSheet.tsx` is the sheet and imports none of them. Found while implementing; the audit had assumed they were reached. |
| `server/src/loader.rs` | 7 packs | a no-op `register_mutations()` | Nothing calls it. Its comment cites a `register_system()` that spec 032 deleted. The two tests in the file are real and stay. |
| `esmodules`, `styles`, `packages` in `system.json` | every pack | 3 keys each | Nothing loads from them. They name `web/dist/index.js`, which no build produces. |
| The template, `basic-game-system` | 1 pack | `module/main.mjs`, a rollup config, a stylesheet | Nothing. The contract says to copy it, and it has the shape of a different product's modules. |

Roll for Shoes, the newest pack, has none of this. It is `system.json`,
`server/`, `web/` and a README, and every file in it is read. It is the shape
the others should have.

Two smaller things are out of line with the contract itself:

- The contract says a pack with a server crate needs "one line outside your
  directory". A pack with its own GraphQL or tables needs more: an entry in
  `apps/server/src/schema_roots.rs`, a migration under
  `crates/thunderforge-server/migrations/`, and its table in two
  `diesel.toml` files. Genie and Roll for Shoes both do all of it.
- `scripts/check-system-registry.mjs` keeps system names out of shared
  server code and out of `apps/web/src`. The other apps, `packages/` and the
  engine crates name none today, and nothing keeps it so. (The audit said the
  check covered the server only. It was wrong about `apps/web/src`.)

## What this spec does not do

It adds no extension point. The audit found real limits for a fully custom
system: four panel slots, no generic per-world settings (ADR-108 deferred
it), no pack-contributed rules beyond derived values and sheet checks, and no
engine hook (ADR-062, by decision). Each is a feature with its own design,
and each should be asked for by a pack that needs it. This spec makes the
ground level so those can be built on it.

It does not let one pack inherit from another. Nothing needs it today.

It does not move a pack's migrations into the pack. ADR-063 covers that, and
the contract will say what is true until it changes.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Everything in a pack is read by something (Priority: P1)

A developer opens any bundled pack to learn how packs work. Every directory
and file in it is one the product reads. Nothing has to be ruled out before
the real mechanism can be seen.

**Why this priority**: the dead parts are what the next pack will be copied
from. Five packs already copied them.

**Independent Test**: delete the parts the table lists; build, lint and test
the workspace; run the system e2e slices. Every system still offers a sheet,
and no test that asserts product behaviour is lost.

**Acceptance Scenarios**:

1. **Given** any bundled pack, **When** its directory is listed, **Then** it
   holds `system.json` and at most `server/`, `web/`, `seed-content/`, a
   README and data files its own crate reads. It holds no `engine/`.
2. **Given** a pack with a `web/` directory, **When** the host's discovery
   paths are checked, **Then** at least one of `src/ActorSheet.tsx`,
   `src/StatBlocks.ts` or `src/panels/*.tsx` exists in it.
3. **Given** Blades, Cypher, Fate, Pathfinder or Year Zero, **When** a
   character of that system is opened, **Then** the sheet is drawn from the
   manifest exactly as before.
4. **Given** a pack's server crate, **When** it is read, **Then** it has no
   function that nothing calls, and its registration tests remain.

---

### User Story 2 - The shape is refused, not just removed (Priority: P1)

A developer adds an `engine/` crate to a pack, or a `web/` directory the host
cannot find anything in. The commit is refused, and the message names the
pack and the rule.

**Why this priority**: Story 1 without this is a cleanup that lasts until the
next pack. Every structural rule this project has kept is one a check
enforces.

**Independent Test**: a script test builds a pack tree with each fault in
turn and expects the check to name it; the check passes on the real tree.

**Acceptance Scenarios**:

1. **Given** a pack directory with an entry the contract does not list,
   **When** the check runs, **Then** it fails and names the pack and entry.
2. **Given** a pack whose `web/` has no discovered entry, **When** the check
   runs, **Then** it fails and names the three paths the host looks for.
3. **Given** a pack with a `server/` crate, **When** the check runs, **Then**
   it fails unless `apps/server/src/system_packs.rs` links that crate.
4. **Given** shared web or engine source that names a system identifier,
   **When** the registry check runs, **Then** it fails as it already does for
   shared server source.

---

### User Story 3 - The template is the smallest real pack (Priority: P2)

Somebody starting a new system copies `basic-game-system`. What they copy is
a manifest that validates and renders a sheet, in the shape the contract
describes, and nothing else.

**Independent Test**: the template's manifest passes
`validate_system_manifest`; its directory passes the Story 2 check; it is
still not offered as a system a world can be bound to.

**Acceptance Scenarios**:

1. **Given** the template, **When** its directory is listed, **Then** it
   holds `system.json` and nothing the product does not read.
2. **Given** the template's manifest, **When** it is validated by the same
   test that walks every bundled pack, **Then** it passes.

---

### User Story 4 - The manifest declares only what is read (Priority: P2)

A pack author reads a bundled `system.json`. Every key in it is either
described by the contract or read by that pack's own crate. None points at a
file that does not exist.

**Independent Test**: no bundled manifest carries `esmodules`, `styles` or
`packages`; a manifest that omits them validates; one that still carries them
(an installed pack written against the old contract) is not refused.

**Acceptance Scenarios**:

1. **Given** any bundled manifest, **When** it is read, **Then** it has no
   `esmodules`, `styles` or `packages` key.
2. **Given** a manifest without those keys, **When** it is validated,
   **Then** it passes.
3. **Given** an older manifest that carries them, **When** it is installed,
   **Then** it is accepted and the keys are ignored.

---

### User Story 5 - The contract states its real cost (Priority: P3)

A pack author who wants their own GraphQL fields or tables reads the
contract and learns every file outside their directory they must touch, and
why each one cannot be discovered.

**Independent Test**: the contract's list matches what Genie and Roll for
Shoes touch, file for file; `check-pack-docs.mjs` still passes.

**Acceptance Scenarios**:

1. **Given** the contract, **When** "the one line outside your directory" is
   read, **Then** it lists the linkage line and dependency for any server
   crate, and separately the schema-roots entry, the migration and the two
   `diesel.toml` entries for a pack with its own GraphQL or tables.
2. **Given** the contract's pack shape, **When** it is read, **Then** it has
   no `engine/` entry, and says why with a pointer to ADR-062.

### Edge Cases

- **A test that only existed in an engine crate.** Each engine crate's tests
  assert that its empty plugin builds, or exercise code that ships nowhere.
  They go with the crate, and the Rust test count drops by that number. The
  count before and after is recorded in the tasks.
- **5e's engine `dice.rs`.** It rolls a d20 with advantage, which
  `thunderforge-dice` already does for the product. If anything in it is not
  covered there, it moves to the dice crate's tests rather than vanishing.
- **Genie's `index.ts`.** `apps/web` imports it by alias, so it is read and
  stays. Only an `index.ts` nothing imports is removed.
- **Genie keeps `components/CharacterSheet.tsx`** because its
  `ActorSheet.tsx` imports it. 5e's is imported by nothing and goes. The rule
  is reachability from a discovered entry, not a file name.
- **The lockfile.** Removing five pnpm packages changes `pnpm-lock.yaml`.
  That is one regenerated file in the same commit as the removal.
- **The wasm lint** names `dnd5e-engine`. It is updated in the commit that
  removes the crate.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: No system pack MUST contain an `engine/` directory, and the
  Cargo workspace MUST list no pack engine crate.
- **FR-002**: A pack's `web/` directory MUST contain at least one entry the
  host discovers. A pack with none MUST have no `web/` directory.
- **FR-003**: A pack's server crate MUST NOT keep a function nothing calls.
  The tests that prove its registration MUST be kept.
- **FR-004**: Every system's character sheet, token bars and checks MUST
  behave after this change as they did before it.
- **FR-005**: A check MUST run before every commit and refuse: a pack entry
  the contract does not list; a `web/` with no discovered entry; a `server/`
  crate that the application, or the server library's test binary, does
  not link; a system directory with no `system.json`.
- **FR-006**: The registry check MUST refuse a system identifier in shared
  web source (`apps/*/src`, `packages/*/src`, where it covered `apps/web/src`
  alone) and shared engine source
  (`crates/thunderforge-engine`, `crates/thunderforge-core`,
  `crates/thunderforge-canvas-core`), with the same exemptions for tests and
  fixtures it already allows in server source.
- **FR-007**: `basic-game-system` MUST be a manifest-only pack that passes
  manifest validation and is not offered to a world.
- **FR-008**: `esmodules`, `styles` and `packages` MUST be optional in the
  manifest schema, absent from every bundled manifest, and ignored when an
  installed manifest carries them.
- **FR-008a**: Every bundled manifest MUST pass the validation an installed
  manifest must. Found while implementing FR-008: the schema also required
  `authors` and `packs`, the contract documents `author`, and so a pack
  written to the contract was refused on install while no bundled manifest
  was ever held to the schema. Both are optional now, and a test walks every
  bundled manifest through the install validator.
- **FR-009**: The contract MUST list every file outside a pack's directory
  that a bundled pack touches, split by what the pack contributes.
- **FR-010**: The contract MUST describe the pack shape as it is: no
  `engine/`, and `web/` only with a discovered entry.

## Success Criteria *(mandatory)*

- **SC-001**: The Cargo workspace has 7 fewer members (39 to 32), and
  `cargo check --all-targets`, `make lint` and `make test-rust` pass.
- **SC-002**: The pnpm workspace has 5 fewer packages, and
  `pnpm --filter @thunderforge/web typecheck` and the web unit tests pass.
- **SC-003**: The system e2e slices pass from the main checkout after the
  change.
- **SC-004**: The new check passes on the tree and fails on each fault in
  FR-005, shown by its own tests.
- **SC-005**: Every bundled pack directory lists only entries the contract
  names.
- **SC-006**: A grep for `esmodules`, `"styles"` and `"packages"` across
  `packs/systems/*/system.json` returns nothing.

## Assumptions

- ADR-029 and ADR-062 stand. This spec enforces them; it reopens neither.
- Roll for Shoes is the reference shape. Where another pack differs from it
  without a reason, the other pack changes.
- No installed pack outside this repository relies on `esmodules` or
  `styles` being loaded, because nothing has ever loaded them.
- 5e and Roll for Shoes are the systems that must be field-test ready. The
  other six must keep working, and their manifest-drawn sheets are the
  product's answer for them.
