---

description: "Task list for Pack Architecture — interface-pack half"
---

# Tasks: Pack Architecture — Interface Packs Are Themes

**Input**: Design documents from `/specs/032-pack-architecture/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Scope**: **User Story 1 only**, plus the interface-pack half of User Story 3.
User Story 2 (system packs) is gated on ADR-029 and is not in this increment;
spec 031's T076 sits inside that gate and stays blocked. See
[Deferred](#deferred) — this is a decision, not an omission.

**Tests**: Included, and not optional here. FR-003 and FR-012a both say
"rejected by automated validation, not reviewer judgement", which makes the
validator's tests part of the requirement rather than a quality practice. The
e2e is what proves SC-001's "without reloading" and SC-002's "identical
actions and values", neither of which a unit test can observe.

**Organization**: Grouped by user story. Phase 2 is unusually load-bearing:
the manifest format is what makes the safety property structural, so it is
settled — and tested — before anything reads it.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to

## Path Conventions

Four surfaces, per plan.md: `crates/pack_system_spec/` (manifest and
validation, native), `src/server/` (Axum + async-graphql + Diesel),
`apps/web/` (React chrome + Playwright), `packs/interface/` (the packs
themselves). The engine crate is **not** touched — see T024.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: The decision of record, and the directory the whole feature hangs off.

- [ ] T001 Write ADR-046 — "an interface pack is data, not a module" — in `docs/adrs/`, recording why the format has nowhere to put code and how that makes this half independent of ADR-029, and add its row to `docs/adrs/README.md` (Constitution IV, research.md §1)
- [ ] T002 Add `interface_packs_dir` to `Directories` in `src/server/src/config/mod.rs`, resolving to `<data>/packs/interface` exactly as `systems_dir` resolves to `<data>/packs/systems`
- [ ] T003 [P] Create `packs/interface/` with a `README.md` stating that this directory holds presentation-only packs, that a pack here may not contribute behaviour, and that the type is exclusive (FR-002)

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: The manifest format, its validator, and the two packs that exercise it.

**⚠️ CRITICAL**: Everything in Phase 3 reads the type defined in T004. Nothing
in Phase 3 can begin until T004–T010 are done.

- [ ] T004 Define `InterfaceManifest` in `crates/pack_system_spec/src/interface.rs` — `id`, `type` (literal `"interface"`), `title`, `version`, `description`, `compatibility`, `legal` (reusing `SystemManifestLegal`, not redeclaring it), `light`, `dark`, optional `canvas` — with `#[serde(deny_unknown_fields)]` at every level, per `contracts/interface-pack-manifest.md`
- [ ] T005 Define the token map in the same file: every key from the contract's token table, all optional, camelCase mapping one-for-one onto the `--kebab-case` custom properties in `apps/web/src/styles/globals.css`
- [ ] T006 [P] Implement WCAG relative luminance and contrast ratio in `crates/pack_system_spec/src/contrast.rs`, with a doc comment stating explicitly that this is **not** `thunderforge_canvas_core::resource_display::luma` (Rec. 709) and why the two must not be confused (research.md §6)
- [ ] T007 [P] Add unit tests for T006 in the same file against published WCAG worked examples — black on white is 21:1, and at least one pair either side of both 4.5:1 and 3:1
- [ ] T008 Implement `validate_interface_manifest` in `crates/pack_system_spec/src/interface.rs`, running the five checks in the contract's Validation section in order: structural, colour parse, legibility floor, legal (reusing `validate_legal_content`), and `id`-matches-directory. Every failure names the offending value
- [ ] T009 Add validator tests in `crates/pack_system_spec/` covering one rejection per contract row: unknown key, `"type": "system"`, `id`/directory mismatch, unparseable colour, missing `legal`, and a contrast failure **in light only** whose message names the mode (FR-002, FR-003, FR-012a, SC-003, SC-003a)
- [ ] T010 Author `packs/interface/forge/interface.json` by transcribing the current `:root` and `.dark` values from `apps/web/src/styles/globals.css` verbatim, so that landing this feature changes nothing about how the product looks (FR-007)
- [ ] T011 [P] Author a second pack under `packs/interface/` that is *visibly* different from Forge and passes the legibility floor. It exists so SC-002 and SC-005 are testable at all; a second pack that merely differs by a hue is not evidence
- [ ] T012 [P] Add a test asserting Forge's manifest reproduces the values in `apps/web/src/styles/globals.css`, so the base pack and the stylesheet cannot drift apart silently — the drift this repo has already been bitten by once, per MVP.md's own header note

**Checkpoint**: `cargo test -p pack_system_spec` passes. A malformed pack is refused with a message that names what is wrong. Nothing yet reads any of it.

---

## Phase 3: User Story 1 — A Game Master dresses the table (Priority: P1) 🎯 MVP

**Goal**: A Game Master picks the world's look; everyone at that table sees it, without a reload, and nothing else about the product changes.

**Independent Test**: Install both packs, switch the world between them as its GM, and confirm every screen re-skins for every participant while every available action, permission, and displayed value stays identical.

### Server

- [ ] T013 [US1] Create `src/server/src/interface_packs.rs` with `router()` exposing `GET /` (list: id, title, version, description, sorted by title, no special position for Forge) and `GET /{id}/manifest.json` (validate before serving, failing closed the way `get_system_manifest` does), reading from `state.directories.interface_packs_dir`
- [ ] T014 [US1] Mount it in `src/server/src/main.rs` as `api_router.nest("/interface-packs", interface_packs::router())`, mirroring the `/systems` mount at line ~554
- [ ] T015 [P] [US1] Add `EVENT_CODE_WORLD_APPEARANCE_CHANGED: i32 = 23` to `src/server/src/world_events.rs` with the doc comment convention its siblings use (`contracts/graphql-appearance.md`)
- [ ] T016 [US1] Add `UpdateWorldInterfacePackInput` to `src/server/src/graphql/input_types.rs` with a **nullable** `interfacePackId` — clearing the binding is a real thing a GM may do, unlike `updateWorldGameSystem`
- [ ] T017 [US1] Implement `update_world_interface_pack_impl` in `src/server/src/graphql.rs`: authorize with `is_dm_of_world` and refuse with *"Only the DM (Owner or GM) may change a world's interface pack"*; reject an id naming a pack that does not exist or does not validate; write `worlds.interface_pack_id`; record the T015 event. Mirror `update_world_game_system_impl` (FR-010, Constitution III)
- [ ] T018 [US1] Expose it as the `updateWorldInterfacePack` mutation on the mutation root in `src/server/src/graphql.rs`
- [ ] T019 [US1] Add server tests in `src/server/src/graphql.rs`'s test module: a GM succeeds, a player is refused, an unknown pack id is refused, `null` clears the binding, and the world event is recorded in each success case

### Web — resolving and applying

- [ ] T020 [P] [US1] Add `apps/web/src/api/interfacePacks.ts` — list packs, fetch one manifest — mirroring `apps/web/src/api/gameSystems.ts`
- [ ] T021 [US1] Create `apps/web/src/appearance/appearance-context.ts` holding the context, the `ResolvedAppearance` type from data-model.md, and the `useAppearance` hook. Context and hook go in their own module from the start: a module exporting a provider *and* a hook cannot fast-refresh, and this repo now enforces that at `--max-warnings=0`
- [ ] T022 [US1] Create `apps/web/src/appearance/AppearanceProvider.tsx` — resolve the world's pack (Forge as base, chosen pack overlaid, per data-model.md's *Resolved Appearance*), and apply the token map for the reader's current light/dark selection onto `document.documentElement` as custom properties. No stylesheet fetch, no reload
- [ ] T023 [US1] Make the provider re-apply when the reader toggles light/dark, reading `useTheme` from `apps/web/src/hooks/theme-context` — the reader keeps their brightness, the world keeps its pack (research.md §5)
- [ ] T024 [US1] Send the manifest's `canvas` block to the engine as `{ type: "set_display_appearance", appearance }` when the appearance resolves and whenever it changes, using the command already typed in `apps/web/src/engine/sdk/commands.ts`. **No engine change**: this is that command's first caller
- [ ] T025 [US1] Subscribe to `EVENT_CODE_WORLD_APPEARANCE_CHANGED` where the world's other event handlers live in `apps/web/src/pages/world/WorldPage.tsx`, re-resolving the appearance on receipt so a participant sees the change without reloading (SC-001)
- [ ] T026 [P] [US1] Mount `AppearanceProvider` inside the world layout — not at the app root — because the binding is per world and a user with two worlds open must not see one world's look leak into the other

### Web — the picker

- [ ] T027 [US1] Create `apps/web/src/pages/world/settings/WorldAppearanceSettingsCard.tsx`: list the available packs in title order with no badge or pinned position for Forge, preview one without committing, and commit through the T018 mutation (FR-008, US1 scenarios 1 and 6)
- [ ] T028 [US1] Show the active pack **by name** — "Forge" for a world that has never chosen one, never an empty select and never a placeholder string (FR-023, US1 scenario 3)
- [ ] T029 [US1] Gate the control on `useWorldRole` so a player sees it read-only, and surface the server's refusal message rather than a silent no-op if it is somehow reached (FR-010)
- [ ] T030 [US1] Add the card to `apps/web/src/pages/world/settings/WorldSystemSettingsPage.tsx` alongside the existing system and grants cards

### Tests

- [ ] T031 [P] [US1] Vitest for the overlay in `apps/web/src/appearance/`: a pack declaring one token inherits the rest from Forge; an absent pack resolves to Forge with `missing` set; the light/dark selection picks the right map
- [ ] T032 [US1] E2E `apps/web/e2e/world-appearance.spec.ts` — a GM changes the world's pack and a second browser context in the same world sees it **without reloading** (SC-001, US1 scenario 1); both contexts see identical content, actions and values (US1 scenario 2); a player is refused (FR-010); the pack list shows Forge among its peers (US1 scenario 6)

**Checkpoint**: User Story 1 is complete and demonstrable. A GM can dress the table, the table sees it, and nothing else moved.

---

## Phase 4: User Story 3 — A world survives a pack that is not there (interface half only, Priority: P3)

**Goal**: A missing interface pack costs nothing, and the two screens that lie about the binding stop lying.

**Independent Test**: Remove a bound pack's directory, reload, and confirm the world opens in Forge with one notice naming what is missing and nothing blocked; restore it and confirm the world returns with no re-binding step.

**Note**: the system-pack half of this story (FR-019, FR-020, FR-021) belongs to User Story 2's gate and is not built here.

- [ ] T033 [US3] Fall back to Forge when the world's `interfacePackId` names a pack that is absent or fails validation, setting `missing` on the resolved appearance (FR-018, data-model.md)
- [ ] T034 [US3] Tell the participant **once** — not once per navigation — naming the missing pack, and block nothing (FR-018)
- [ ] T035 [P] [US3] Replace `"Unbound placeholder"` in `apps/web/src/pages/world/components/WorldCard.tsx` with the active pack's title, which is "Forge" when the binding is unset (FR-022, FR-023, SC-008)
- [ ] T036 [P] [US3] Replace `"Not yet assigned"` for the interface pack in `apps/web/src/pages/world/WorldDashboardPage.tsx` with the same. Leave the **`gameSystemId`** labels on both screens alone — there the unset state is real, and it belongs to User Story 2
- [ ] T037 [US3] Extend `apps/web/e2e/world-appearance.spec.ts`: a world bound to a pack that is not installed opens in Forge, says so once, blocks nothing, and returns to that pack when it is restored with no re-binding step; and both labels read the same true thing (SC-008)

**Checkpoint**: The feature no longer describes a state the product does not have.

---

## Phase 5: Polish & Cross-Cutting Concerns

- [ ] T038 Run `quickstart.md` by hand, end to end, including §1's "open a dialog" and §3 step 4's light/dark check. Constitution V — the suite proves the mechanical half, a person proves the rest
- [ ] T039 Do the SC-002 pass: walk the product under both packs and confirm 100% of available actions, permissions and displayed values are identical and only presentation differs
- [ ] T040 [P] Update `docs/adrs/README.md` and `MVP.md` if either describes the interface-pack field as unused — it no longer is
- [ ] T041 Run `pnpm verify` and fix what it reports **in the code this feature added**. Keep it to that; wide passes get their own commit (`pnpm verify:fix` rewrites what can be rewritten mechanically)

---

## Deferred

Not oversights. Recorded so a later reading does not have to reconstruct why.

- **User Story 2 — system packs (FR-004, FR-005, FR-013 to FR-017, SC-004, SC-005, SC-009, SC-010).** Gated on ADR-029, an empty stub. FR-017's interim restriction — no system packs from any source but the product itself — is already the de facto state and needs no code to hold.
- **Spec 031 T076 — system-supplied turn structure.** Inside that gate. Spec 031 cannot close on this increment.
- **The system-pack half of User Story 3 (FR-019, FR-020, FR-021).** Degrading a *world* is a different problem from degrading a *look*, and it depends on the mounting mechanism US2 has not built.
- **An `interface_packs` table and an upload/install flow.** Bundled packs only (research.md §3). One migration to add later; nothing about the manifest format changes when it is.
- **Removing `apps/web/src/styles/tokens.scss`.** Imported by zero files and a fossil of a previous design system, but deleting it is unrelated tidying.
- **Web fonts in a pack.** A real want, a separate decision, and one that involves a fetch this format deliberately does not make.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: no dependencies
- **Foundational (Phase 2)**: needs T002 for the directory; **blocks Phase 3 entirely**
- **User Story 1 (Phase 3)**: needs Phase 2 complete
- **User Story 3 (Phase 4)**: needs T022 — the resolver is where the fallback lives. Buildable immediately after it, and independently testable
- **Polish (Phase 5)**: needs Phases 3 and 4

### Within User Story 1

Server (T013–T019) and web (T020–T030) can proceed in parallel once Phase 2 is
done: they meet only at the contract in `contracts/graphql-appearance.md`.
Within the web half, T021 blocks T022, which blocks T023, T024, T025 and T033.
T027 needs T020 for the list and T018 for the commit.

### Parallel Opportunities

- T003, T006/T007, T011, T012 within Phase 2
- T015, T020, T026 within Phase 3
- T031 alongside any of T027–T030
- T035 and T036 are different files and independent of each other
- The whole server half and the whole web half of User Story 1, if two people

---

## Parallel Example: Phase 2

```bash
# The contrast rule and the second pack do not touch each other:
Task: "Implement WCAG contrast in crates/pack_system_spec/src/contrast.rs"
Task: "Author the second interface pack under packs/interface/"
Task: "Write packs/interface/README.md"
```

---

## Implementation Strategy

### MVP

Phases 1, 2 and 3. At T032 the feature is demonstrable: a Game Master dresses
the table and the table sees it. Stop there and validate before Phase 4.

### Why Phase 2 is worth finishing before anything reads it

The manifest format is the safety property. If Phase 3 starts against a format
that is still moving, the pressure to add "just one key that carries a bit of
logic" arrives from a real implementation problem rather than from a proposal
anyone would refuse. Settling and testing the format first is what makes FR-003
a fact about the schema instead of a rule someone has to keep enforcing.

### Notes

- [P] tasks = different files, no dependencies
- Commit after each task or logical group
- The engine crate is not modified by this feature. If a task appears to require
  it, that is a signal the pack is being asked to contribute behaviour — stop
  and re-read FR-003 before writing the change
