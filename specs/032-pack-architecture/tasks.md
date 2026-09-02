---

description: "Task list for Pack Architecture — interfaces shaped by their system"
---

# Tasks: Pack Architecture — Interfaces Shaped By Their System

**Input**: Design documents from `/specs/032-pack-architecture/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Supersedes**: the first task list, written before the five clarifications. It
scoped User Story 1 as colours and spacing. This one is roughly twice its size,
which is the honest consequence of "the system computes, the interface
arranges" and not a change of ambition.

**Scope**: **User Story 1**, plus the interface-pack half of User Story 3.
User Story 2 and spec 031's T076 stay gated on ADR-029. See
[Deferred](#deferred).

**Tests**: Included, and not optional. FR-003, FR-012a and FR-026 all say
rejection is by automated validation rather than reviewer judgement, which
makes the validator's tests part of the requirement. `derive` being pure is
likewise a property, not a preference — two viewers of one character seeing
two sheets is the failure it prevents.

**Organization**: Four increments from plan.md. A and B are prerequisites with
nothing a Game Master can see; that is stated rather than disguised. C is the
MVP checkpoint. D is where it gets hard.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to

## Path Conventions

`crates/thunderforge-canvas-core/` (the contract, natively tested),
`crates/pack_system_spec/` (manifest and validation), `src/server/` (resolution,
routes, mutation), `packs/systems/*/` (each system's rules),
`packs/interface/` (the packs), `apps/web/` (React chrome + Playwright).

---

## Phase 1: Setup

- [X] T001 [P] Write ADR-059 — "an interface pack is data, not a module" — in `docs/adrs/`, recording why the format has nowhere to put code and how that keeps this increment independent of ADR-029, and add its row to `docs/adrs/README.md` (Constitution IV, research §1)
- [X] T002 [P] Write ADR-060 — the system contract: one declaration, declared values rather than a fixed struct, and why it lives in `thunderforge-canvas-core` — citing the two divergent traits it retires and the two fixed structs this codebase has already corrected. Index it in `docs/adrs/README.md` (research §8, §9)
- [X] T003 [P] Write ADR-061 in `docs/adrs/` — how a system pack's implementation is discovered rather than listed, recording that the mechanism is the least settled decision in this feature and what would change the answer. Index it in `docs/adrs/README.md` (research §12)
- [X] T004 Add `interface_packs_dir` to `Directories` in `src/server/src/config/mod.rs`, resolving to `<data>/packs/interface` exactly as `systems_dir` resolves to `<data>/packs/systems`
- [X] T005 [P] Create `packs/interface/` with a `README.md` stating that packs here are presentation-only, may not contribute behaviour, that the type is exclusive (FR-002), and that bundled packs are named `Forged <Metal>` with Forge as the base (FR-007b)

---

## Phase 2: Increment A — Declared values, end to end

**Purpose**: one contract, one resolution path, derived values that exist at all.

**⚠️ CRITICAL**: layout addresses declared identifiers. Nothing in Increment B
can be validated until identifiers resolve, so this phase blocks that one.

- [X] T006 Define `DeclaredValue`, `Origin` and the `SystemRules` trait in `crates/thunderforge-canvas-core/src/system_rules.rs` per `contracts/system-contract.md`, with `derived_declarations()` separate from `derive()` so a pack can be validated against a system without running it
- [X] T007 Document on the trait, in prose, that `derive` is pure — no I/O, no clock, no randomness — and why: a derived value is recomputed per read and never stored, so an impure one shows two viewers of the same character two different sheets
- [X] T008 [P] Add contract-level tests in `crates/thunderforge-canvas-core/` that a `SystemRules` implementation returning an identifier absent from `derived_declarations()` is rejected by the resolver rather than silently rendered
- [ ] T009 Delete `trait GameSystem`, `SkillDefinition` and `DerivedStats` from `src/engine/src/systems/core.rs`, whose only implementation is a stub and on which nothing depends (`src/server/src/attributes.rs` records this). Confirm with `cargo check --target wasm32-unknown-unknown -p thunderforge_engine`
- [ ] T010 Delete the re-declared `trait GameSystemTrait` from `packs/systems/dnd5e/engine/src/plugin.rs` — the one whose comment says it "should match" the deleted one and has drifted from it — and depend on `thunderforge_canvas_core` instead, which is the cross-package dependency its comment says it was avoiding
- [X] T011 Implement `SystemRules` for Genie in `packs/systems/genie/server/`, declaring and deriving what Genie actually needs, with unit tests. Genie first because `IMPLEMENTED_SYSTEM_IDS` in `apps/web/src/api/gameSystems.ts` contains only `genie`
- [ ] T012 Resolve stored and derived values into one `identifier → value` set in `src/server/src/attributes.rs` (or a sibling), merging the manifest-declared stored read that already exists with `derive`, tagging each with its `origin`
- [ ] T013 Expose the resolved set on the actor's GraphQL type, `origin` included, so a surface can tell which values a player may edit and which are computed
- [ ] T014 Replace the **seven** `register_*_system` functions in `src/server/src/systems.rs` and their `GAME_SYSTEMS` caller with discovery per ADR-061, removing the hand-wired validator lists and the **seven** per-system `[dependencies.*_server]` blocks in `src/server/Cargo.toml`. Adding a system currently requires editing both files, and `GAME_SYSTEMS` already carries a `// In future phases: register_coc7e_system(...)` comment — the eighth line somebody was going to have to write (FR-029, SC-004)
- [ ] T015 [P] Add `scripts/check-system-registry.mjs`, modelled on `scripts/check-interaction-seam.mjs`, failing the build if a hand-maintained list of system identifiers reappears in shared server code — and add it as a step in `scripts/verify.mjs`
- [ ] T016 [P] Server tests in `src/server/src/attributes.rs`'s test module: an actor in a Genie world reports stored and derived values through one path, and the same stored input always yields the same derived output

**Checkpoint**: declared values resolve end to end for Genie. Nothing looks different yet.

---

## Phase 3: Increment B — The pack format, its validator, and Forge

**Purpose**: the format is the safety property. Settle and test it before anything reads it.

- [ ] T017 Define `InterfaceManifest` in `crates/pack_system_spec/src/interface.rs` — identity, `compatibility`, `legal` (reusing `SystemManifestLegal`), `light`, `dark`, optional `canvas`, `targets`, optional `layout` — with `#[serde(deny_unknown_fields)]` at every level, per `contracts/interface-pack-manifest.md`
- [ ] T018 Define the token map in the same file: every key from the contract's token table, all optional, camelCase mapping one-for-one onto the `--kebab-case` custom properties in `apps/web/src/styles/globals.css`
- [ ] T019 Define the layout vocabulary in `crates/pack_system_spec/src/layout.rs`: containers (`section`, `column`, `row`), generic constructs addressing a declaration set by kind and order, and specific constructs naming an identifier (FR-025a). The type MUST have no place for an expression, a conditional, a threshold, or a label the system did not declare
- [ ] T020 [P] Implement WCAG relative luminance and contrast ratio in `crates/pack_system_spec/src/contrast.rs`, with a doc comment stating explicitly that this is **not** `thunderforge_canvas_core::resource_display::luma` (Rec. 709) and why the two must not be confused (research §6)
- [ ] T021 [P] Unit-test T020 in `crates/pack_system_spec/src/contrast.rs` against published WCAG worked examples — black on white is 21:1, and a pair either side of both 4.5:1 and 3:1
- [ ] T022 Implement `validate_interface_manifest` running the seven checks in the contract's Validation section in order: structural, colour parse, legibility floor, legal, id-matches-directory, targeting, and Forge conformance. Every failure names the offending value
- [ ] T023 Implement the targeting check (FR-026): for each id in `targets`, read that system's manifest and its `derived_declarations`, and reject a layout naming an identifier that system does not declare — naming the identifier **and** the system. Validate per target independently, never against their union
- [ ] T024 Implement the empty-`targets` rule: a pack declaring `targets: []` is rejected if its layout names any identifier at all, because naming an identifier is naming a system whatever the list says
- [ ] T025 Validator tests, one rejection per contract row: unknown key, `"type": "system"`, id/directory mismatch, unparseable colour, missing `legal`, a contrast failure **in light only** whose message names the mode, an identifier the target does not declare, and an untargeted pack that names one (FR-002, FR-003, FR-012a, FR-026, SC-003, SC-003a, SC-003b)
- [ ] T026 Author `packs/interface/forge/interface.json` — tokens transcribed verbatim from the current `:root` and `.dark` in `apps/web/src/styles/globals.css` so landing this changes nothing about how the product looks, `targets: []`, and a generic-only layout (FR-007, FR-025b)
- [ ] T027 Implement the Forge conformance test in `crates/pack_system_spec/src/interface.rs` (FR-007a): every construct the format offers appears somewhere in `packs/interface/forge/interface.json`, and Forge names no identifier. This is what catches a construct nothing can actually build, and it is why the format's authority is the schema rather than Forge
- [ ] T028 [P] Add a test asserting Forge's tokens still reproduce `apps/web/src/styles/globals.css`, so the base pack and the stylesheet cannot drift apart silently — the drift MVP.md's own header records this repo being bitten by once

**Checkpoint**: `cargo test -p pack_system_spec` passes. A malformed, illegible or over-claiming pack is refused by name. Still nothing looks different.

---

## Phase 4: Increment C — User Story 1 (Priority: P1) 🎯 MVP

**Goal**: a Game Master picks the world's look; everyone at that table sees it without reloading; an actor renders in that pack's arrangement; nothing else changes.

**Independent Test**: switch a Genie world between two packs as its GM and confirm every screen re-skins for every participant, the sheet takes the pack's arrangement, and every available action, permission and displayed value stays identical.

### Server

- [ ] T029 [US1] Create `src/server/src/interface_packs.rs` with `GET /` (list: id, title, version, description, targets, sorted by title, no special position for Forge) and `GET /{id}/manifest.json` (validate before serving, failing closed as `get_system_manifest` does), reading from `state.directories.interface_packs_dir`
- [ ] T030 [US1] Mount it in `src/server/src/main.rs` as `api_router.nest("/interface-packs", …)`, mirroring the `/systems` mount
- [ ] T031 [P] [US1] Add `EVENT_CODE_WORLD_APPEARANCE_CHANGED: i32 = 23` to `src/server/src/world_events.rs` with the doc-comment convention its siblings use
- [ ] T032 [US1] Add `UpdateWorldInterfacePackInput` to `src/server/src/graphql/input_types.rs` with a **nullable** `interfacePackId` — clearing the binding is a real thing a GM may do
- [ ] T033 [US1] Implement `update_world_interface_pack_impl` in `src/server/src/graphql.rs`: authorize with `is_dm_of_world`, refusing with *"Only the DM (Owner or GM) may change a world's interface pack"*; reject a pack that does not exist, does not validate, **or does not target this world's system**; write the column; record the T031 event (FR-010, Constitution III)
- [ ] T034 [US1] Expose it as the `updateWorldInterfacePack` mutation on the mutation root
- [ ] T035 [US1] Server tests: a GM succeeds, a player is refused, an unknown pack is refused, a pack not targeting the world's system is refused, `null` clears the binding, and the event is recorded on each success

### Web — resolution and rendering

- [ ] T036 [P] [US1] Add `apps/web/src/api/interfacePacks.ts` — list, and fetch one manifest — mirroring `apps/web/src/api/gameSystems.ts`
- [ ] T037 [US1] Create `apps/web/src/appearance/appearance-context.ts` holding the context, the `ResolvedAppearance` type from data-model.md, and the `useAppearance` hook. Context and hook in their own module from the start: a module exporting a provider *and* a hook cannot fast-refresh, and this repo now enforces that at `--max-warnings=0`
- [ ] T038 [US1] Create `apps/web/src/appearance/AppearanceProvider.tsx` — resolve Forge as base, overlay the chosen pack, apply the token map for the reader's light/dark selection onto `document.documentElement` as custom properties. No stylesheet fetch, no reload
- [ ] T039 [US1] Make it re-apply when the reader toggles light/dark, reading `useTheme` from `apps/web/src/hooks/theme-context` — the reader keeps their brightness, the world keeps its pack (research §5)
- [ ] T040 [US1] Send the manifest's `canvas` block to the engine as `{ type: "set_display_appearance", appearance }` on resolve and on change, using the command already typed in `apps/web/src/engine/sdk/commands.ts`. **No engine change**: this is that command's first caller
- [ ] T041 [US1] Build the layout renderer in `apps/web/src/layout/`: walk a `LayoutDeclaration`, resolve generic constructs against the system's declarations in order, resolve specific constructs by identifier, and render nothing — not an empty frame — for a set the system declares as empty
- [ ] T042 [US1] Make derived values non-editable in the renderer, keyed on `origin`, and stored values editable. A text box over a computed number invites the two to disagree, which is the failure `origin` exists to prevent
- [ ] T043 [US1] Subscribe to `EVENT_CODE_WORLD_APPEARANCE_CHANGED` where the world's other event handlers live in `apps/web/src/pages/world/WorldPage.tsx`, re-resolving on receipt so participants see the change without reloading (SC-001)
- [ ] T044 [P] [US1] Mount `AppearanceProvider` inside the world layout — not at the app root — because the binding is per world and a user with two worlds open must not see one leak into the other

### Web — the picker

- [ ] T045 [US1] Create `apps/web/src/pages/world/settings/WorldAppearanceSettingsCard.tsx`: list packs that target this world's system (plus every untargeted pack) in title order with no badge or pinned position for Forge, preview one without committing, commit through T034 (FR-008, US1 scenarios 1, 6, 7)
- [ ] T046 [US1] Show the active pack **by name** in `apps/web/src/pages/world/settings/WorldAppearanceSettingsCard.tsx` — "Forge" for a world that has never chosen one, never an empty select and never a placeholder (FR-023, US1 scenario 3)
- [ ] T047 [US1] Gate the control on `useWorldRole` so a player sees it read-only, surfacing the server's refusal rather than a silent no-op (FR-010)
- [ ] T048 [US1] Add the card to `apps/web/src/pages/world/settings/WorldSystemSettingsPage.tsx` beside the existing system and grants cards

### Tests

- [ ] T049 [P] [US1] Vitest for resolution: a pack declaring one token inherits the rest from Forge; an absent pack resolves to Forge with `missing` set; light/dark picks the right map; a generic construct over an empty declaration set renders nothing
- [ ] T050 [US1] E2E `apps/web/e2e/world-appearance.spec.ts` — a GM changes the pack and a second browser context in the same world sees it **without reloading** (SC-001); both see identical content, actions and values (US1 scenario 2); a player is refused (FR-010); Forge appears among its peers (US1 scenario 6); a Genie actor renders through generic layout with no empty skills heading (US1 scenario 8)

**Checkpoint**: User Story 1 is demonstrable. A GM dresses the table, the table sees it, and an actor renders in the system's own shape.

---

## Phase 5: Increment D — A targeted pack, and one 5e implementation

**Goal**: prove specific addressing and compatibility validation with a second pack, and end the state where 5e exists twice.

**⚠️ This is the phase that will slip.** It touches live UI covered by e2e specs. Spec 031's history is explicit: when a control moves, grep for its placeholder and its accessible name, not just its testid — and check whether any absence assertion just became vacuous.

- [ ] T051 [US1] Implement `SystemRules` for 5e in `packs/systems/dnd5e/server/`, deriving ability modifiers, save and skill totals, passive perception, proficiency bonus by level, spell save DC and spell attack bonus, with unit tests. The logic exists twice already — in `packs/systems/dnd5e/web/src/derived-data.ts`, which nothing loads, and inline in `apps/web/src/components/game-systems/dnd5e/CharacterSheet.tsx`, which does. Port from the former, verify against the latter, keep neither
- [ ] T052 [US1] Author `packs/interface/forged-steel/interface.json` targeting 5e, with a layout using specific addressing — an ability block with modifier badges, a skill list with proficiency marks, a slot grid, a death-save tracker — and an original ThunderForge design. **FR-003b**: the published sheet told us what 5e tracks; it does not tell us how ours looks, and its arrangement, wording and ornament are not ours to reproduce
- [ ] T053 [US1] Confirm the layout vocabulary in `crates/pack_system_spec/src/layout.rs` can actually express `packs/interface/forged-steel/interface.json` without a conditional or an expression. If it cannot, that is a finding about the format and belongs back in T019 — not a new construct bolted on to unblock one pack
- [ ] T054 [US1] Delete `apps/web/src/systems/dnd5e/` and `apps/web/src/components/game-systems/dnd5e/`, repointing consumers at the pack (FR-030, SC-004). `dnd5e` is the only system with a module in shared app code; after this, none is
- [ ] T055 [US1] Before T054 lands, grep `apps/web/e2e/` for the placeholders and accessible names of every control being removed, not only their testids, and check whether any absence assertion has become vacuous
- [ ] T056 [P] [US3] Fall back to Forge when the world's pack is absent or fails validation, setting `missing` on the resolved appearance (FR-018)
- [ ] T057 [US3] Tell the participant **once** — not once per navigation — naming the missing pack, and block nothing, from `apps/web/src/appearance/AppearanceProvider.tsx` (FR-018)
- [ ] T058 [P] [US3] Replace `"Unbound placeholder"` in `apps/web/src/pages/world/components/WorldCard.tsx` with the active pack's title, which is "Forge" when unset (FR-022, FR-023, SC-008)
- [ ] T059 [P] [US3] Replace `"Not yet assigned"` for the interface pack in `apps/web/src/pages/world/WorldDashboardPage.tsx` with the same. Leave the **`gameSystemId`** labels on both screens alone — there the unset state is real, and it belongs to User Story 2
- [ ] T060 [US1] E2E in `apps/web/e2e/world-appearance.spec.ts`: two worlds on two systems, each under a pack targeting it, render visibly and structurally different sheets with the shared application unchanged between them (SC-005)
- [ ] T061 [US3] E2E in `apps/web/e2e/world-appearance.spec.ts`: a world bound to a pack that is not installed opens in Forge, says so once, blocks nothing, and returns when the pack is restored with no re-binding step; and both labels read the same true thing (SC-008)

**Checkpoint**: two packs, two shapes, and `apps/web/src/systems/` is gone.

---

## Phase 6: Polish & Cross-Cutting Concerns

- [ ] T062 Run `quickstart.md` by hand, end to end — including §1's "open a dialog", §3 step 4's light/dark check, and §6 step 4's derived-value editability check. Constitution V
- [ ] T063 Do the SC-002 pass against a running `node scripts/dev.mjs`: walk the product under both packs and confirm 100% of available actions, permissions and displayed values are identical and only presentation differs
- [ ] T064 [P] Update `MVP.md` and `docs/adrs/README.md` where either describes the interface-pack field as unused, or 5e's presentation as living in the app
- [ ] T065 Run `pnpm verify` and fix what it reports **in the code this feature added**. Keep it to that; wide passes get their own commit

---

## Deferred

- **User Story 2 — system packs mounting their own surfaces (FR-004, FR-005, FR-013 to FR-016, SC-004's mounting half, SC-009, SC-010).** Gated on ADR-029. Note the distinction this whole increment rests on: ADR-029 governs loading **third-party** code at runtime. `packs/systems/*/server` are Cargo workspace members compiled into the product, which is why Increment A is not gated.
- **Spec 031 T076 — system-supplied turn structure.** Inside that gate. Spec 031 cannot close on this increment.
- **The system-pack half of User Story 3 (FR-019 to FR-021).** Degrading a world is a different problem from degrading a look.
- **Third-party system packs**, per FR-017's interim restriction.
- **An `interface_packs` table and an upload flow.** Bundled packs only.
- **Web fonts in a pack.** A real want, a separate decision, and one involving a fetch this format deliberately does not make.
- **Removing `apps/web/src/styles/tokens.scss`.** Zero importers; unrelated tidying.

---

## Dependencies & Execution Order

### Phase dependencies

- **Setup (1)**: none
- **Increment A (2)**: needs T004; **blocks Increment B** — layout validation reads declared identifiers, which do not exist until A resolves them
- **Increment B (3)**: needs A; **blocks Increment C**
- **Increment C (4)**: needs B. MVP checkpoint
- **Increment D (5)**: needs C. T051 blocks T052 blocks T053. T055 precedes T054
- **Polish (6)**: needs C and D

### Within Increment A

T006 blocks T007, T008, T011, T012. T009 and T010 are independent of the rest
and of each other. T012 blocks T013 and T016. T014 blocks T015.

### Within Increment C

The server half (T029–T035) and the web half (T036–T048) meet only at
`contracts/graphql-appearance.md` and can run in parallel. T037 blocks T038,
which blocks T039, T040, T043 and T056. T041 blocks T042. T045 needs T036 and T034.

### Parallel opportunities

- T001, T002, T003, T005 across Setup
- T008, T015, T016 within A; T009 and T010 alongside anything
- T020/T021, T028 within B
- T031, T036, T044, T049 within C
- T056, T058, T059 within D
- The whole server half and the whole web half of Increment C, with two people

---

## Implementation Strategy

### MVP

Setup, A, B, C. At T050 a Game Master can dress the table and an actor renders
in its system's shape. Stop and validate before D.

### Why A and B come first, with nothing to show

Increment A exists because layout addresses declared identifiers and there are
no derived identifiers today. Increment B exists because the format is the
safety property: if C starts against a format that is still moving, the
pressure to add "just one construct that carries a conditional" arrives from a
real rendering problem rather than from a proposal anyone would refuse. That is
how FR-003 stops being a fact about the schema and becomes a rule someone has
to keep enforcing.

Both phases are prerequisites with nothing a Game Master can see, and that is
worth saying out loud to whoever is asked how it is going in week one.

### Notes

- [P] = different files, no dependencies. Commit after each task or logical group
- The engine crate is modified only by T009 and T010, both deletions. If any
  other task appears to require an engine change, that is a signal a pack is
  being asked to contribute behaviour — stop and re-read FR-003 before writing it
- Verify per crate against its real target: `cargo check --target
  wasm32-unknown-unknown` for the engine, native `cargo check` for the server
  and pack crates, `tsc` for the web app (Constitution V)
