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

**Scope**: **User Story 1**, plus the interface-pack half of User Story 3 —
and, since 2026-09-03, **User Story 2** as Increment F. See
[Phase 5d](#phase-5d-increment-f--user-story-2-priority-p2).

**ADR-029 is now written (2026-09-03), and the gate has moved.** It ratifies
what three ADRs had already established in practice: packs from outside the
product are data, and executable extension is bundled-only. So a *bundled*
pack contributing behaviour is permitted, and User Story 2 is no longer
blocked on a security decision — only on being built. What stays forbidden is
third-party executable packs, which is FR-017 as a decision rather than an
interim measure. See [Deferred](#deferred).

**Tests**: Included, and not optional. FR-003, FR-012a and FR-026 all say
rejection is by automated validation rather than reviewer judgement, which
makes the validator's tests part of the requirement. `derive` being pure is
likewise a property, not a preference — two viewers of one character seeing
two sheets is the failure it prevents.

**Organization**: Six increments from plan.md. A and B are prerequisites with
nothing a Game Master can see; that is stated rather than disguised. C is the
MVP checkpoint. D is where it gets hard, E grew the vocabulary to cover a whole
sheet, and F is User Story 2 — three of whose six scenarios A–E already
satisfied.

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
- [X] T009 **Rescoped 2026-09-02 — the premise was wrong.** `trait GameSystem` in `src/engine/src/systems/core.rs` is *not* unused: `src/engine/src/systems/builtin/basic.rs` implements it for `BasicSystem`, `src/engine/src/plugins/system_registration.rs` builds a `SystemRegistry` around it, and that plugin is added to the running app at `src/engine/src/lib.rs:1078`. Two tests in `src/engine/src/integration_tests.rs` use it too. `src/server/src/attributes.rs`'s claim that "nothing depends on it" is half right — the single implementation is `BasicSystem`, but live code depends on the registry holding it. So this is a migration, not a deletion: decide whether `BasicSystem` becomes a `SystemRules` implementation or goes away with the registry, then remove `systems/core.rs`, `systems/builtin/`, `plugins/system_registration.rs`, its `mod`/`pub use` lines, the `.add_plugins` call and the two tests together. Confirm with `cargo check --target wasm32-unknown-unknown -p thunderforge_engine`
- [X] T009a **Two name collisions to avoid while doing T009.** `DerivedStats` exists twice: the one in `src/engine/src/systems/core.rs` is dead, the one in `src/engine/src/components.rs` is load-bearing (used by `derived_data.rs` and by token spawn in `lib.rs`) — and `src/engine/src/systems/mod.rs:22` re-exports the core one under the same name, so a careless delete takes the wrong type. `SkillDefinition` likewise collides with the live `packs/systems/dnd5e/server/src/srd.rs::SkillDefinition`
- [X] T010 Delete the re-declared `trait GameSystemTrait` from `packs/systems/dnd5e/engine/src/plugin.rs` — the one whose comment says it "should match" the deleted one and has drifted from it — and depend on `thunderforge_canvas_core` instead, which is the cross-package dependency its comment says it was avoiding
- [X] T010a The re-declared `GameSystemTrait` T010 removed from the 5e engine pack exists in **six more** pack engine crates — `pathfinder2e`, `year_zero_engine`, `fate_core`, `blades_in_the_dark`, `cypher_system`, `genie`. They are stubs with no default bodies, so nothing is at risk of being lost, but they are six more copies of the contract FR-027 says must be stated once. Remove them the same way
- [X] T011 Implement `SystemRules` for Genie in `packs/systems/genie/server/`, declaring and deriving what Genie actually needs, with unit tests. Genie first because `IMPLEMENTED_SYSTEM_IDS` in `apps/web/src/api/gameSystems.ts` contains only `genie`
- [X] T012 Resolve stored and derived values into one `identifier → value` set in `src/server/src/attributes.rs` (or a sibling), merging the manifest-declared stored read that already exists with `derive`, tagging each with its `origin`
- [X] T013 Expose the resolved set on the actor's GraphQL type, `origin` included, so a surface can tell which values a player may edit and which are computed
- [X] T014 Replace the **seven** `register_*_system` functions in `src/server/src/systems.rs` and their `GAME_SYSTEMS` caller with discovery per ADR-061, removing the hand-wired validator lists and the **seven** per-system `[dependencies.*_server]` blocks in `src/server/Cargo.toml`. Adding a system currently requires editing both files, and `GAME_SYSTEMS` already carries a `// In future phases: register_coc7e_system(...)` comment — the eighth line somebody was going to have to write (FR-029, SC-004)
- [X] T015 [P] Add `scripts/check-system-registry.mjs`, modelled on `scripts/check-interaction-seam.mjs`, failing the build if a hand-maintained list of system identifiers reappears in shared server code — and add it as a step in `scripts/verify.mjs`
- [X] T014a **Half done.** Of the two violations the checker found, the default system id is fixed: `prepare_world_input` now takes it as an argument and it lives in the config manifest beside the other realm defaults, where an operator already looks. A product default is not supposed to grow a branch per pack, and the way to keep that true is for that layer to know no pack's name. `None` is a real answer — a world with no system is a state the product handles.
- [X] T014a3 Realm seed values — `DEFAULT_GAME_SYSTEM_ID` and its siblings in `src/server/src/admin.rs` — live in Rust and are written into the config manifest at bootstrap. They belong in a shipped, version-controlled config file that `default_manifest()` reads, so seeding a realm stops being a thing shared code knows. Note `src/server/data` is gitignored, so the file cannot go there; blanking the constant without a replacement makes every new world systemless on every install
- [X] T014a2 **Closed 2026-09-03.** World creation no longer branches on a system id; the pack contributes a world-creation hook and the server runs whatever is linked. See Increment F2.
- [X] T014b Every pack still declares `pub const SYSTEM_ID: &str = "..."` alongside the id it now passes to `SystemContribution::new`. Two places inside one pack naming the same thing, with nothing checking they agree. Harmless today and invisible to `check-system-registry.mjs`, which only polices shared code — but it is the same drift in miniature
- [X] T016 [P] Server tests in `src/server/src/attributes.rs`'s test module: an actor in a Genie world reports stored and derived values through one path, and the same stored input always yields the same derived output

**Checkpoint**: declared values resolve end to end for Genie. Nothing looks different yet.

---

## Phase 3: Increment B — The pack format, its validator, and Forge

**Purpose**: the format is the safety property. Settle and test it before anything reads it.

- [X] T017 Define `InterfaceManifest` in `crates/pack_system_spec/src/interface.rs` — identity, `compatibility`, `legal` (reusing `SystemManifestLegal`), `light`, `dark`, optional `canvas`, `targets`, optional `layout` — with `#[serde(deny_unknown_fields)]` at every level, per `contracts/interface-pack-manifest.md`
- [X] T018 Define the token map in the same file: every key from the contract's token table, all optional, camelCase mapping one-for-one onto the `--kebab-case` custom properties in `apps/web/src/styles/globals.css`
- [X] T019 Define the layout vocabulary in `crates/pack_system_spec/src/layout.rs`: containers (`section`, `column`, `row`), generic constructs addressing a declaration set by kind and order, and specific constructs naming an identifier (FR-025a). The type MUST have no place for an expression, a conditional, a threshold, or a label the system did not declare
- [X] T020 [P] Implement WCAG relative luminance and contrast ratio in `crates/pack_system_spec/src/contrast.rs`, with a doc comment stating explicitly that this is **not** `thunderforge_canvas_core::resource_display::luma` (Rec. 709) and why the two must not be confused (research §6)
- [X] T021 [P] Unit-test T020 in `crates/pack_system_spec/src/contrast.rs` against published WCAG worked examples — black on white is 21:1, and a pair either side of both 4.5:1 and 3:1
- [X] T022 Implement `validate_interface_manifest` running the seven checks in the contract's Validation section in order: structural, colour parse, legibility floor, legal, id-matches-directory, targeting, and Forge conformance. Every failure names the offending value
- [X] T023 Implement the targeting check (FR-026): for each id in `targets`, read that system's manifest and its `derived_declarations`, and reject a layout naming an identifier that system does not declare — naming the identifier **and** the system. Validate per target independently, never against their union
- [X] T024 Implement the empty-`targets` rule: a pack declaring `targets: []` is rejected if its layout names any identifier at all, because naming an identifier is naming a system whatever the list says
- [X] T025 Validator tests, one rejection per contract row: unknown key, `"type": "system"`, id/directory mismatch, unparseable colour, missing `legal`, a contrast failure **in light only** whose message names the mode, an identifier the target does not declare, and an untargeted pack that names one (FR-002, FR-003, FR-012a, FR-026, SC-003, SC-003a, SC-003b)
- [X] T026 Author `packs/interface/forge/interface.json` — tokens transcribed verbatim from the current `:root` and `.dark` in `apps/web/src/styles/globals.css` so landing this changes nothing about how the product looks, `targets: []`, and a generic-only layout (FR-007, FR-025b)
- [X] T027 Implement the Forge conformance test in `crates/pack_system_spec/src/interface.rs` (FR-007a): every construct the format offers appears somewhere in `packs/interface/forge/interface.json`, and Forge names no identifier. This is what catches a construct nothing can actually build, and it is why the format's authority is the schema rather than Forge
- [X] T028 [P] Add a test asserting Forge's tokens still reproduce `apps/web/src/styles/globals.css`, so the base pack and the stylesheet cannot drift apart silently — the drift MVP.md's own header records this repo being bitten by once

**Checkpoint**: `cargo test -p pack_system_spec` passes. A malformed, illegible or over-claiming pack is refused by name. Still nothing looks different.

---

## Phase 4: Increment C — User Story 1 (Priority: P1) 🎯 MVP

**Goal**: a Game Master picks the world's look; everyone at that table sees it without reloading; an actor renders in that pack's arrangement; nothing else changes.

**Independent Test**: switch a Genie world between two packs as its GM and confirm every screen re-skins for every participant, the sheet takes the pack's arrangement, and every available action, permission and displayed value stays identical.

### Server

- [X] T029 [US1] Create `src/server/src/interface_packs.rs` with `GET /` (list: id, title, version, description, targets, sorted by title, no special position for Forge) and `GET /{id}/manifest.json` (validate before serving, failing closed as `get_system_manifest` does), reading from `state.directories.interface_packs_dir`
- [X] T030 [US1] Mount it in `src/server/src/main.rs` as `api_router.nest("/interface-packs", …)`, mirroring the `/systems` mount
- [X] T031 [P] [US1] Add `EVENT_CODE_WORLD_APPEARANCE_CHANGED: i32 = 23` to `src/server/src/world_events.rs` with the doc-comment convention its siblings use
- [X] T032 [US1] Add `UpdateWorldInterfacePackInput` to `src/server/src/graphql/input_types.rs` with a **nullable** `interfacePackId` — clearing the binding is a real thing a GM may do
- [X] T033 [US1] Implement `update_world_interface_pack_impl` in `src/server/src/graphql.rs`: authorize with `is_dm_of_world`, refusing with *"Only the DM (Owner or GM) may change a world's interface pack"*; reject a pack that does not exist, does not validate, **or does not target this world's system**; write the column; record the T031 event (FR-010, Constitution III)
- [X] T034 [US1] Expose it as the `updateWorldInterfacePack` mutation on the mutation root
- [X] T035 [US1] Server tests: a GM succeeds, a player is refused, an unknown pack is refused, a pack not targeting the world's system is refused, `null` clears the binding, and the event is recorded on each success

### Web — resolution and rendering

- [X] T036 [P] [US1] Add `apps/web/src/api/interfacePacks.ts` — list, and fetch one manifest — mirroring `apps/web/src/api/gameSystems.ts`
- [X] T037 [US1] Create `apps/web/src/appearance/appearance-context.ts` holding the context, the `ResolvedAppearance` type from data-model.md, and the `useAppearance` hook. Context and hook in their own module from the start: a module exporting a provider *and* a hook cannot fast-refresh, and this repo now enforces that at `--max-warnings=0`
- [X] T038 [US1] Create `apps/web/src/appearance/AppearanceProvider.tsx` — resolve Forge as base, overlay the chosen pack, apply the token map for the reader's light/dark selection onto `document.documentElement` as custom properties. No stylesheet fetch, no reload
- [X] T039 [US1] Make it re-apply when the reader toggles light/dark, reading `useTheme` from `apps/web/src/hooks/theme-context` — the reader keeps their brightness, the world keeps its pack (research §5)
- [X] T040 [US1] Send the manifest's `canvas` block to the engine as `{ type: "set_display_appearance", appearance }` on resolve and on change, using the command already typed in `apps/web/src/engine/sdk/commands.ts`. **No engine change**: this is that command's first caller
- [X] T041 [US1] Build the layout renderer in `apps/web/src/layout/`: walk a `LayoutDeclaration`, resolve generic constructs against the system's declarations in order, resolve specific constructs by identifier, and render nothing — not an empty frame — for a set the system declares as empty
- [X] T042 [US1] Make derived values non-editable in the renderer, keyed on `origin`, and stored values editable. A text box over a computed number invites the two to disagree, which is the failure `origin` exists to prevent
- [X] T043 [US1] Subscribe to `EVENT_CODE_WORLD_APPEARANCE_CHANGED` where the world's other event handlers live in `apps/web/src/pages/world/WorldPage.tsx`, re-resolving on receipt so participants see the change without reloading (SC-001)
- [X] T044 [P] [US1] Mount `AppearanceProvider` inside the world layout — not at the app root — because the binding is per world and a user with two worlds open must not see one leak into the other

### Web — the picker

- [X] T045 [US1] Create `apps/web/src/pages/world/settings/WorldAppearanceSettingsCard.tsx`: list packs that target this world's system (plus every untargeted pack) in title order with no badge or pinned position for Forge, preview one without committing, commit through T034 (FR-008, US1 scenarios 1, 6, 7)
- [X] T046 [US1] Show the active pack **by name** in `apps/web/src/pages/world/settings/WorldAppearanceSettingsCard.tsx` — "Forge" for a world that has never chosen one, never an empty select and never a placeholder (FR-023, US1 scenario 3)
- [X] T047 [US1] Gate the control on `useWorldRole` so a player sees it read-only, surfacing the server's refusal rather than a silent no-op (FR-010)
- [X] T048 [US1] Add the card to `apps/web/src/pages/world/settings/WorldSystemSettingsPage.tsx` beside the existing system and grants cards

### Tests

- [X] T049 [P] [US1] Vitest for resolution: a pack declaring one token inherits the rest from Forge; an absent pack resolves to Forge with `missing` set; light/dark picks the right map; a generic construct over an empty declaration set renders nothing
- [X] T019a **(2) fixed; (1) and (3) outstanding.** The layout format is insufficient in three places, found by building the renderer against it.** In descending severity: (1) `slotGrid` carries one `id` and a `levels` count, but each level has *two* numbers (total and spent) and the wire carries one value per identifier — nothing in the format says how level three's total is named, so it is unrenderable without an out-of-band convention the renderer had to invent; (2) `barStack` has no maximum to draw a bar against, because `GraphQLDeclaredValue.value` is one pre-rendered string — the renderer recovers a proportion by *parsing* `"4 / 7"`, which is precisely the "branching on what a value means" that the wire type's own comment says it exists to prevent, and a system writing `"4 of 7"` silently loses its bar; (3) `tracker` has no notion of what a box is, so a system storing death saves as a list shows nothing sensible. (2) is the one to fix first and the fix is probably that resources reach the client in the `current`/`max` shape the canvas already uses, rather than flattened to text
- [X] T019c **The layout format has no construct for free text, and two of three systems are mostly free text.** Cypher's 55 fields agree with Fate's: `background`, `descriptor`, `focus`, `type`, `special abilities`, `attacks`, `notes`, seven blank `might skills` slots, and repeating `cyphers`/`equipment` lists — none of which the format can express. — **Closed by T072 (`text`, `paragraph`, `list` constructs) and T076.**
- [X] T019c1 **The layout format has no construct for free text, and Fate Core is mostly free text.** Field inventory taken from Evil Hat's form-fillable Fate Core sheet (CC-BY system; read for scope only, per FR-003b): 51 fields against 5e's 336, and the difference is not size but kind. Fate's sheet is **3 Aspects**, **4 Consequences each paired with a Consequence Aspect**, **26 blank Skill slots** (a pyramid the player fills, not a fixed list like 5e's 18), **8 Stress boxes**, `Current Fate`, `Refresh`, and one `Stunts and Extras` block. Of those, the layout format can express only the two numbers. There is no construct for a free-text field, none for a repeating list of them, and none for a label-and-text pair. A Fate pack today would render two numbers and nothing else — **Closed by T072 and T075 — Fate declares aspects, consequences, 26 player-named slots and a stunts block.**
- [X] T019d **`tracker` is wrong for two of the three systems that have one.** Three sheets, three shapes, and `boxes` + `rows` fits only the first: 5e's death saves are **two separate 3-box runs** meaning opposite things (successes, failures); Fate's stress is **one flat 8-box track**; Cypher's damage track is **named ordered states** — `impaired`, `debilitated`, dead — with no boxes at all. A construct shaped by the first ruleset that needed it, which is the `DerivedStats` mistake in miniature. Whatever replaces it has to admit a run of boxes *and* an ordered set of named states — **Closed by T071 — `track` and `state` are separate kinds, a run of marks and an ordered ladder being different things.**
- [X] T019f **Cypher's stat is a triple, and nothing in the format expresses it.** Each of `might`, `speed` and `intellect` is three fields on the sheet — a current value, a **pool** (its maximum), and an **edge** (a modifier applied to expenditure). So a Cypher stat is simultaneously an attribute, a bar, and a modifier, where 5e's is a score with a derived modifier and Fate has no attributes at all. `badgeGrid of attributes` shows one third of it and `barStack of resources` another third, and nothing shows the relationship — **Closed by T067 and T019g — a Cypher stat is a group whose pool is a resource, whose edge is a number, and which now names itself.**
- [X] T019e **The manifests are incomplete for the systems that are not 5e or Genie.** `fate_core` and `cypher_system` declare no top-level `resources` block at all, so they publish no pools and `barStack` renders nothing for them — even though `fate_core`'s `resource_data` has `fate_points`/`refresh` and `cypher_system`'s has `might_pool`/`speed_pool`/`intellect_pool`. For Cypher those pools *are* the core mechanic. Fate also stores its aspects and consequences nowhere: its `trait_data` is a single `notes` field — **Closed by T075/T076 — both manifests declare their pools; Fate's fatePoints/refresh and Cypher's three stat pools.**
- [X] T019b `section.collapsed` is carried to the DOM as `data-collapsed` but did not collapse anything. **Decided: in MVP 1, and it needs no state.** The format already defined the semantics — starts collapsed, a reader who opens it stays opened, nothing can force it shut — and that sentence is `<details>`, natively, with keyboard and screen-reader behaviour already right. Only a titled section collapses; the title is the summary, and a collapsed section with nothing to click is one a reader cannot open
- [X] T050 [US1] E2E `apps/web/e2e/world-appearance.spec.ts` — a GM changes the pack and a second browser context in the same world sees it **without reloading** (SC-001); both see identical content, actions and values (US1 scenario 2); a player is refused (FR-010); Forge appears among its peers (US1 scenario 6); a Genie actor renders through generic layout with no empty skills heading (US1 scenario 8)

**Checkpoint**: User Story 1 is demonstrable. A GM dresses the table, the table sees it, and an actor renders in the system's own shape.

---

## Phase 5: Increment D — A targeted pack, and one 5e implementation

**Goal**: prove specific addressing and compatibility validation with a second pack, and end the state where 5e exists twice.

**⚠️ This is the phase that will slip.** It touches live UI covered by e2e specs. Spec 031's history is explicit: when a control moves, grep for its placeholder and its accessible name, not just its testid — and check whether any absence assertion just became vacuous.

- [X] T051 [US1] Implement `SystemRules` for 5e in `packs/systems/dnd5e/server/`, deriving ability modifiers, save and skill totals, passive perception, proficiency bonus by level, spell save DC and spell attack bonus, with unit tests. The logic exists twice already — in `packs/systems/dnd5e/web/src/derived-data.ts`, which nothing loads, and inline in `apps/web/src/components/game-systems/dnd5e/CharacterSheet.tsx`, which does. Port from the former, verify against the latter, keep neither
- [X] T052 [US1] Author `packs/interface/forged-steel/interface.json` targeting 5e, with a layout using specific addressing — an ability block with modifier badges, a skill list with proficiency marks, a slot grid, a death-save tracker — and an original ThunderForge design. **FR-003b**: the published sheet told us what 5e tracks; it does not tell us how ours looks, and its arrangement, wording and ornament are not ours to reproduce
- [X] T053 [US1] Confirm the layout vocabulary in `crates/pack_system_spec/src/layout.rs` can actually express `packs/interface/forged-steel/interface.json` without a conditional or an expression. If it cannot, that is a finding about the format and belongs back in T019 — not a new construct bolted on to unblock one pack
- [X] T054 [US1] Delete `apps/web/src/systems/dnd5e/` and `apps/web/src/components/game-systems/dnd5e/`, repointing consumers at the pack (FR-030, SC-004). `dnd5e` is the only system with a module in shared app code; after this, none is
- [X] T055 [US1] Before T054 lands, grep `apps/web/e2e/` for the placeholders and accessible names of every control being removed, not only their testids, and check whether any absence assertion has become vacuous
- [X] T056 [P] [US3] Fall back to Forge when the world's pack is absent or fails validation, setting `missing` on the resolved appearance (FR-018)
- [X] T057 [US3] Tell the participant **once** — not once per navigation — naming the missing pack, and block nothing, from `apps/web/src/appearance/AppearanceProvider.tsx` (FR-018)
- [X] T058 [P] [US3] Replace `"Unbound placeholder"` in `apps/web/src/pages/world/components/WorldCard.tsx` with the active pack's title, which is "Forge" when unset (FR-022, FR-023, SC-008)
- [X] T059 [P] [US3] Replace `"Not yet assigned"` for the interface pack in `apps/web/src/pages/world/WorldDashboardPage.tsx` with the same. Leave the **`gameSystemId`** labels on both screens alone — there the unset state is real, and it belongs to User Story 2
- [X] T060 [US1] E2E in `apps/web/e2e/world-appearance.spec.ts`: two worlds on two systems, each under a pack targeting it, render visibly and structurally different sheets with the shared application unchanged between them (SC-005)
- [X] T061 [US3] E2E in `apps/web/e2e/world-appearance.spec.ts`: a world bound to a pack that is not installed opens in Forge, says so once, blocks nothing, and returns when the pack is restored with no re-binding step; and both labels read the same true thing (SC-008)

**Checkpoint**: two packs, two shapes, and `apps/web/src/systems/` is gone.

---

## Phase 5b: Increment E — Every shipping system has a usable sheet (US4)

**Goal**: bind a world to any bundled system, open an actor under the base pack
alone, and find a sheet worth reading — without a pack having been written for
it.

**Independent Test**: for each bundled system in turn, everything its manifest
declares is present on the sheet and nothing it declares is absent.

**Supersedes T019a/T019c/T019d/T019f**, which recorded these gaps one at a
time as each was found. They stay in the list as the evidence trail; the work
is here.

### The vocabulary

- [X] T066 [US4] Extend `DeclaredValueKind` in `crates/thunderforge-canvas-core/src/system_rules.rs` to the set FR-031 requires: the existing `Integer`/`Number`/`Text`/`Boolean`/`List`/`Fraction`, plus a **`Track`** (a bounded run of marks, with how many are filled) and a **`State`** (an ordered set of named states with one current). Each new kind must be a *shape of value* and never a rule about one — the moment a variant carries a threshold or a condition, the format is a language and FR-003 is gone
- [X] T067 [US4] Add a **group** to the declaration model (FR-033), so a Fate consequence arrives as its severity and the aspect written into it, and a Cypher stat as its current value, pool and edge — one thing with parts, not three unrelated identifiers. Decide deliberately whether this nests in a value or is a declaration that names its members; the second keeps `DeclaredValue` flat, which everything downstream currently relies on
- [X] T068 [US4] Add **player-named slots** (FR-032) to the attribute/skill declarations in `crates/thunderforge-canvas-core/src/attributes.rs`: a declaration that says "there are twenty-six of these and the player names them" rather than naming them. A format modelling only fixed lists turns Fate's twenty-six blanks into eighteen wrong labels
- [X] T069 [P] [US4] Unit-test each new kind in `crates/thunderforge-canvas-core/src/system_rules_tests.rs` against the real shapes: 5e's two 3-mark runs, Fate's flat 8-mark track, Cypher's impaired/debilitated/dead. A track and a state set are different things and a test that treats them alike proves nothing
- [X] T070 [US4] Render unknown kinds as labelled text (FR-035, SC-014) wherever values are resolved, so a system declaring something this build does not know loses nothing. Absence is indistinguishable from the character not having it, which is why this degrades rather than drops

### The layout constructs

- [X] T071 [US4] Replace `tracker` in `crates/pack_system_spec/src/layout.rs`. It carries `boxes` + `rows` and fits one of the three systems that has a track: 5e's death saves are two separate 3-mark runs meaning opposite things, Fate's stress is one flat 8-mark track, Cypher's damage track has **no marks at all**. Whatever replaces it must admit a run of marks and an ordered set of named states as different things
- [X] T072 [US4] Add text constructs to the layout format: a labelled text field, a paragraph block, and a list a player adds to. Two of the three sheets read are *mostly* these — Fate's aspects, consequences and stunts; Cypher's background, descriptor, focus, type, special abilities, attacks and notes — and the format can currently express none of them
- [X] T073 [US4] Fix `slotGrid` (T019a's remaining gap): it carries one identifier and a level count, but each level has a total and an expended count and nothing says how level three's total is named. Either the construct addresses its members or the declaration groups them — T067's answer probably settles this one too
- [X] T074 [US4] Extend Forge to render every kind FR-031 admits (FR-034), and extend `validate_conformance` to require it. A kind the format admits and the base pack cannot draw is a kind nobody has shown can be presented

### The manifests

- [X] T075 [P] [US4] Declare `fate_core`'s sheet in `packs/systems/fate_core/system.json`: fate points and refresh as resources (it currently declares **no** top-level `resources` block at all), three aspects, four consequences each paired with an aspect, twenty-six player-named skill slots, a flat eight-mark stress track, and a stunts block
- [X] T076 [P] [US4] Declare `cypher_system`'s sheet in `packs/systems/cypher_system/system.json`: might, speed and intellect each as a group of current value, pool and edge; effort, tier, xp, recovery bonus, armor and limit; the impaired/debilitated/dead damage track as named ordered states; seven player-named skill slots; and the free-text identity and equipment fields
- [X] T019g **A group has no label and no headline member**, found by rendering one. A group falls back to its first member's label, which is a guess: a Fate consequence group reads better as "Mild Consequence" than as whatever its first part is called, and nothing says which member is the one to show when there is room for one. Both are things a sheet wants and the format cannot say
- [X] T019h **The named declaration sets and `all` can drift.** `other` is a complement, so the renderer needs the whole published set as well as the five named ones — and nothing checks the two agree. If the server sent each value's set membership beside its `group`, the client would stop carrying six lists that can disagree with each other
- [X] T019i **A state's null current and its rendered text disagree.** `State { current: None }` renders to an empty string in the `value` field, so a system that legitimately named a rung `""` would read as no-rung. Only the structured field is trustworthy — the same lesson as T019a, in a smaller place
- [X] T019j **A `Track` cannot express a segmented run** — Fate's style where only boxes 1, 2 and 4 exist has no representation. **Decided: not in MVP 1, deliberately.** Nothing shipping needs it, and the shape is genuinely unsettled — those numbers could be labels on marks, capacities each mark absorbs, or values a mark is worth, and the three want different constructs. Shaping it now, against no consumer, is precisely how `tracker` and `DerivedStats` were shaped by their first ruleset and found wrong by their second. The first system that actually needs it brings the shape with it, and `Track` stays a bounded run of interchangeable marks — which is what every shipping system's track is
- [X] T077 [P] [US4] Audit the remaining bundled manifests — `blades_in_the_dark`, `pathfinder2e`, `year_zero_engine`, `basic-game-system` — against the same question, and declare what each actually tracks
- [X] T078 [US4] Write a Fate-shaped and a Cypher-shaped interface pack under `packs/interface/`. This is the acceptance test for the whole increment (SC-013): if either needs a format change to be written, the format is not finished, and finding that out by writing a pack is what worked every previous time

### Proof

- [X] T079 [US4] Test in `crates/pack_system_spec/` that for every bundled system, everything its manifest declares is renderable by Forge (SC-012). Read the systems from the directory rather than listing them, so a pack added later is covered without anyone remembering to add it
- [X] T080 [US4] E2E in `apps/web/e2e/world-appearance.spec.ts`: a world on each of at least three structurally different systems, opened under the base pack alone, renders that system's own sheet — and the three are visibly different from each other

**Checkpoint**: no bundled system renders two numbers and a heading.

---

## Phase 5c: The engine's test suite has never run

Found by the 2026-09-02 mutation audit, which could not break the engine's
tests because it could not build them. See `docs/test-audit-2026-09-02.md`.

- [X] T081 `bevy_winit` and `webgl2` were unconditional features, and winit does not compile for a plain Linux host. Both are browser-only and are now confined to the wasm32 target section, which is the pattern this manifest already praises for `thunderforge_cache_browser`. Cargo unions features across target sections, so the shipping build is unchanged
- [X] T082 **The crate is wasm-only far more deeply than winit.** `network`, `optimistic`, `sync`, `conflict_visualization`, `token_sync_d2`, `mutation_sender`, `presence`, `event_dispatcher` and both websocket modules carry `#![cfg(target_arch = "wasm32")]`; `movement.rs` and `plugins/websocket_plugin.rs` import from them **without** being gated, so they compile nowhere else and the gate was implicit. Gating them cascades into `systems/token_move.rs`, `systems/mod.rs`'s re-exports and `lib.rs`. This is a module-graph refactor and wants deciding, not forcing — an attempt was reverted rather than pushed through
- [X] T083 **Recommendation: (a), and the gate is at the wrong granularity — this is smaller than it looks.** Six of the modules carrying `#![cfg(target_arch = "wasm32")]` reference **nothing browser-specific**: `presence`, `conflict_visualization`, `mutation_sender`, `token_sync_d2`, `optimistic` and `sync` contain zero `web_sys`, zero `wasm_bindgen`, zero `js_sys` and zero `crate::network`. They are gated because they sit *beside* the network code, not because they need a browser — and they are what 45 of the tests exercise (`tests_f1_unit.rs` uses five of them and never constructs an `App`). Only `network/*` genuinely needs wasm. So: ungate the six, gate what actually touches `network` (`movement.rs`, `websocket_plugin.rs`, `lib.rs`'s `start`), and move portable rules into `thunderforge-canvas-core` where this repo already puts rules "where tests execute". A trial run got as far as 22 remaining errors across 8 files — stale tests referencing `TokenAbilities` and `DerivedStats::armor_class`, types deleted in earlier specs, which have not compiled since. That is the true size: not a rewrite, but not an afternoon either. Recommend **not** (b): `wasm-bindgen-test` under headless Chromium would test browser-bound versions of logic that is not browser-bound, and make CI slower for it. Keep (b) in reserve for `network/*`, which is the only part that earns it
- [X] T083a Decide **where** the engine's tests should run. Two honest options. (a) Gate the wasm-only half consistently and move genuinely portable logic into `thunderforge-canvas-core`, which is the pattern this repo already uses and the reason that crate exists — `attributes.rs` says outright that rules live there "where tests execute". (b) Run them under wasm32 with `wasm-bindgen-test` and headless Chromium, which tests them where they actually run. (a) shrinks what the engine crate contains; (b) keeps it and makes the suite real. Doing neither leaves ~45 tests that cannot fail
- [X] T084 Whichever wins, `tests_f2_f4_integration.rs::scenario_mutation_timeout_and_rollback` has **zero assertions** — it binds `check_timeouts(6.0)`, never reads it, and unconditionally prints "✅ Scenario 4 passed". And both `test_suite_coverage` functions are pure `eprintln!`, one claiming "50+ unit tests implemented" where the file holds 33

---

## Phase 5d: Increment F — User Story 2 (Priority: P2)

Planned 2026-09-03, after ADR-029 closed the gate. **Three of User Story 2's
six acceptance scenarios already pass** — a system's sheet mounts with no
shared branch naming it, two worlds render structurally different sheets, and
a system declaring no sheet gets Forge's generic one. Increments A–E did that,
and saying so is what keeps this increment honest about its actual size.

**Independent test**: bind two worlds to two bundled systems, add a third
system by dropping a directory under `packs/systems/` and changing nothing
else, and confirm all three are offered, mount, and fail independently.

**Story goal**: the shared application stops knowing which systems exist, a
pack contributes behaviour rather than only declarations, a failing pack
surface is contained and named, and the contract exists for authors.

### F1 — The application stops knowing which systems exist (FR-005, SC-004)

- [X] T085 [US2] Replace the `game_systems` table query in `list_systems` (`src/server/src/systems.rs:31`) with a directory listing over `state.directories.systems_dir`, modelled line for line on `interface_packs::list_installed` — read each `<id>/system.json`, project `id`, `title`, `description` and `version`, sort by title, and omit a pack whose manifest fails to parse rather than listing something that cannot be chosen. The route path `/api/systems` and its response shape stay put so the client change is separable
- [X] T086 [US2] Decide what `packs/systems/basic-game-system/` is, because a directory listing offers **eight** systems where `BUNDLED_SYSTEM_IDS` named seven. It has a complete manifest — `abilities`, `resources`, `skills`, `sheet`, `turnStructure`, a compliant `legal` — so under Increment E's rules it renders a real sheet, but its own description calls it "a blank-slate template ... meant to be forked". Either it is genuinely selectable and every picker gains a row, or the manifest declares itself a template and `list_installed` honours that **declaration** — never a name in shared code. Record the answer where a pack author will find it (T104)
- [X] T087 [US2] Give `load_game_systems` (`src/server/src/graphql/helpers.rs:218`) the same directory source, or delete the `gameSystems` GraphQL field outright if `src/server/src/graphql.rs:277`'s export is its only reader. Two code paths answering "which systems exist" from two stores is the asymmetry this increment is closing, and fixing one of them leaves the bug with a second address
- [X] T088 [US2] Add `listGameSystems(): Promise<GameSystemSummary[]>` to `apps/web/src/api/gameSystems.ts` fetching `/api/systems`, mirroring `listInterfacePacks()` including its `readJson` error handling, and **delete `BUNDLED_SYSTEM_IDS`, `BUNDLED_SYSTEM_LABELS` and the comment that explains why they exist**. That comment is the deleted requirement: "there is no reconciled installed system packs catalog yet"
- [X] T089 [US2] Move the three consumers onto the fetched list: the picker in `apps/web/src/pages/world/CreateWorldPage.tsx:141`, the picker in `apps/web/src/pages/world/settings/WorldSystemSettingsPage.tsx:281`, and `apps/web/src/pages/world/settings/WorldAppearanceSettingsCard.tsx:53`, which maps a pack's `targets` through `BUNDLED_SYSTEM_LABELS` for display. The card needs an id→title map from the same fetch; note it renders a *label for a pack's target*, so a system the deployment does not have must still read as something rather than vanish
- [X] T090 [US2] `CreateWorldPage`'s `useState("genie")` default is a system name in shared web code by another route. The realm's `default_game_system_id` already lives in the config manifest (T014a3) and is the operator's answer; take the default from what the server reports rather than from a literal, and let "no system" stay a real answer
- [X] T091 [US2] [P] Fix the two references that outlive the constants: `apps/web/src/api/interfacePacks.ts:9`, whose header cites `BUNDLED_SYSTEM_IDS` as the asymmetry it is unlike, and `apps/web/e2e/onboarding-flow.spec.ts:165`, which names it in a comment. Both become descriptions of a thing that no longer exists
- [X] T092 [US2] [P] Add a server test beside `get_system_manifest_serves_a_manifest_with_valid_legal` in `src/server/src/systems.rs` proving `list_systems` reports a system written into a temp `systems_dir` and never seeded into `game_systems`, and omits a directory whose `system.json` is malformed. Mutation-test it: break the sort, break the omission, watch each fail with the right message
- [X] T093 [US2] Write ADR-028 — `docs/adrs/` currently holds it as an empty stub, and T085 makes it a live question: what is the `game_systems` **table** for, if `packs/systems/` is the row of record? Record the answer research §F-1 expects (the directory is authoritative; the table is premature, since a row per installed system earns its place only when a system can be installed at runtime, which ADR-029 says it cannot), whether the table is dropped or reserved, and what would change the answer. Index it in `docs/adrs/README.md`

### F2 — A pack contributes behaviour, not only declarations (FR-004)

**Spiked 2026-09-03. The answer is recorded in ADR-063, and the work is
larger than this increment.** The spike is what the plan asked for and its
result is not what the plan expected, so the tasks below are rewritten rather
than ticked.

What was expected: choose a shape for a world-creation hook, add it to
`SystemContribution`, implement it in `genie-server`, delete twenty lines from
`graphql.rs`.

What measuring found: **Genie's session domain already lives in the shared
server, and it is 2,763 production lines** — `graphql/mutations_genie_session.rs`
(2,385 lines, thirteen mutations: Wish Pool, Doom Clock, Puzzle Clocks,
Session Resources, shop listings, two-party trades) and
`graphql/queries/genie_session.rs` (378), reaching six `world_genie_*` tables
through the server's generated `schema.rs`, plus fourteen models and one event
code. `check-system-registry.mjs` passes over all of it honestly: those files
quote `"genie"` only inside `#[cfg(test)]`, which is correctly exempt. The
check measures what it says it measures; this is a larger thing standing
beside it.

So the world-creation insert is twenty lines of a 2,763-line problem, and the
shape that resolves it properly — a pack owning its tables, with
`print_schema`'s `except_tables` keeping them out of the server's schema
(verified available in `diesel_cli` 2.3.12) — **cannot be applied to one table
for one hook**: excluding those six tables breaks the 2,763 lines on the next
build. Shape 3 is now closed rather than open: the server would have to write
to a table with bespoke columns it does not know, which is either
pack-supplied SQL or a generic key-value store, and either way it addresses
only the insert.

- [X] T094 [US2] **Done — the spike ran and answered.** Three shapes evaluated against the code rather than in the abstract; `diesel_cli` 2.3.12 confirmed to support `filter = { except_tables = [...] }`; shape 3 shown to be unable to express the row; and the 2,763-line footprint measured, which is the finding that changes the plan
- [X] T095 [US2] **Done — ADR-063 (`docs/adrs/20260903-063-a_pack_owns_the_tables_it_writes.md`), indexed in `docs/adrs/README.md`.** Records the destination (a pack owns the tables it writes), the two rejected shapes with why, the rejected-outright dodge (moving the branch into `system_packs.rs`, the one file the checker exempts), the sizing, and what would change the answer
- [X] T099 [US2] **Done, differently than written.** The `graphql.rs` entry stays in `KNOWN` — removing it was conditional on the hook landing — but its stated reason was false: it said User Story 2 is "gated on ADR-029", which ADR-029 settled. It now cites ADR-063 and says what the block actually is
- [X] T096 [US2] **Done.** The hook is `WorldCreatedHook` in `src/server/src/world_hooks.rs`, not a field on `SystemContribution` in canvas-core as planned — it takes a `&mut PgConnection`, and canvas-core is compiled to wasm, so putting it beside the other contributions would have dragged Diesel into the browser. It runs inside the world-creation transaction, so a system that cannot set itself up rolls the world back rather than leaving half of one
- [X] T097 [US2] **Done.** `packs/systems/genie/server/src/session/` — six `table!` declarations, eleven models, thirteen mutations and the queries beside them, and the hook that writes the session row with the `doom_clock_max: 6` that was that ruleset's number sitting in shared code. `diesel.toml` excludes the six tables from `print-schema`, so there is one declaration of each and regenerating cannot quietly add a second
- [X] T098 [US2] **Done.** `is_genie_world` and its branch are gone from `src/server/src/graphql.rs`, along with `NewGenieSession` and `world_genie_sessions` from its imports. Full `cargo test` per crate, not a filtered subset
- [X] T100 [US2] **Done, and it moved.** Selection is tested in the library; **discovery is tested in `src/app`**, because `inventory` collects into one compiled crate instance and `cargo test` builds the library a second time under `cfg(test)` while the dev-dependency packs were built against the first. Both mutation-tested: removing the pack's `submit!` fails the discovery test, and pointing the hook at a system that does not exist fails the second by name
- [X] T014a2 **Closed.** The last game system named in shared server code is gone, and `check-system-registry.mjs` reports zero violations with nothing exempted

### F2a — what the move actually required (recorded 2026-09-03)

Two things the F-5 spike had not found, both discovered by doing the work
rather than planning it:

- **`allow_tables_to_appear_in_same_query!` cannot span crates.** It emits an
  impl in each direction, so the reverse one lands on a foreign type and the
  orphan rule refuses it. This would have been a wall had the code needed
  cross-crate joins; it does not, which was established by removing them and
  watching it compile.
- **A dev-dependency cycle gives you two instances of the library.** The packs
  linked against the normal build; the test harness compiles a second copy
  under `cfg(test)`. Anything collected through `inventory` in the crate under
  test is therefore invisible to its own tests. `SystemContribution` escapes
  this only because it collects in canvas-core, which is compiled once.

### F3 — A pack's failure is contained and named (FR-016, SC-009)

`apps/web` has no error boundary at all — no `componentDidCatch`, no
`getDerivedStateFromError`, nothing (verified 2026-09-03). `PackActorSheet`
catches a *fetch* rejection; a surface that throws while rendering takes the
page and names nothing.

- [X] T101 [US2] Add `apps/web/src/appearance/PackSurfaceBoundary.tsx` — a class component with `getDerivedStateFromError`, taking the pack id it wraps and rendering a named message in its place. Not a boundary at the app root: that satisfies "does not crash" and fails "the rest of the session remains usable", because the whole page is the thing replaced. `MissingPackNotice` is the tonal precedent — name the pack, block nothing
- [X] T102 [US2] Wrap each mounted pack surface in it, starting with `PackActorSheet` in `apps/web/src/pages/world/actor/`, passing the pack id the appearance context resolved rather than the one the world *names* — a world falling back to Forge should say Forge failed, because Forge is what rendered
- [X] T103 [US2] [P] Add a vitest that a throwing child renders the boundary's message with the pack named, and a Playwright spec that injects a throwing surface and confirms the surrounding session stays usable — navigation, the world nav, and a second surface all still work. SC-009 measures those two things **separately**, so assert them separately

### F4 — The contract exists as a document (FR-015, SC-010)

- [X] T104 [US2] Write `packs/systems/README.md`, modelled on `packs/interface/README.md`, describing every declaration block a system pack may carry — `abilities`, `abilityFacets`, `resources`, `skills`, `movement`, `sheet`, `groups`, `conditions`, `sizeCategories`, `turnStructure`, `data_types`, `legal` — and every hook it may contribute, including T096's. Written for an author who has not read shared application source, which is what SC-010 measures. Note per block which systems actually use it: `groups` is Fate's, `movement` is absent from Fate entirely, and absence is a fact about the ruleset rather than an omission
- [X] T105 [US2] [P] Document the two things a pack has that are not declarations — the `server/` crate and its `SystemContribution` submission, and the `use <pack> as _;` line in `src/server/src/system_packs.rs` that keeps the linker from discarding it. That line is the one thing adding a system touches outside its own directory, and SC-004 is measured against the change set; say so plainly rather than letting an author find it by having their pack silently do nothing
- [X] T106 [US2] [P] Add `scripts/check-pack-docs.mjs`, modelled on `check-system-registry.mjs`, failing if `packs/systems/README.md` or `packs/interface/README.md` links a path that does not exist — SC-010's "zero references to documents that do not exist" is testable rather than merely assertable. Add it as a step in `scripts/verify.mjs`

### F5 — the browser half of the thing ADR-029 ruled out (FR-017, FR-028)

Found 2026-09-03 while auditing US2 scenario 4 ("installation is validated
against the recorded security terms"), which the plan called open and trivial.

- [X] T107 [US2] Delete `apps/web/src/hooks/useSystemHooks.ts`, `apps/web/src/providers/SystemHooksProvider.tsx` and `apps/web/src/providers/system-hooks-context.ts`. The provider did `await import("/api/systems/<id>/<path>")` — **dynamically importing and executing a system pack's JavaScript in a participant's browser**, which is what ADR-029 forbids and what `system_hooks.rs` was deleted for last session. This is the same machinery on the other side of the wire. Its `SystemHooksContract` carried `armorClass` and `initiative`, and its `BaseTokenStats` hardcoded 5e's six ability scores into a supposedly system-agnostic contract — FR-028's prohibition stated exactly. Nothing mounted the provider, the three files imported only each other, and the `web/dist/index.js` the bundled manifests point at does not exist in any pack, so this was dead code that would have become a live violation the moment somebody wired it up

**Note for whoever revisits `POST /api/systems/install`.** That route is
admin-gated and unpacks an uploaded archive into the systems directory. With
the loader gone, an installed pack is data — read for its manifest and served
as static files — which is what ADR-029 permits. The route was never the
violation; the `import()` was.

**Left alone deliberately**: every bundled manifest still declares
`esmodules: ["web/dist/index.js"]` and a `styles` entry, and nothing reads
either now. Those files do not exist in any pack. Removing the keys means
touching eight manifests and `SystemManifest`'s required fields in
`pack_system_spec`, which is a schema change and wants its own change set —
`packs/systems/README.md` deliberately does not document either key.

### Checkpoint

Amended twice on 2026-09-03. It first read "zero outstanding violations",
which F2's spike found cost more than the increment held; it was rewritten to
say so, and then the work was done. Both amendments are left visible because
the second only means anything beside the first.

**Delivered, all of it**: the shared application no longer knows which systems
exist — a system is added by creating a directory, proved end to end against a
running stack; a pack contributes **behaviour**, with Genie's six tables,
eleven models and 2,763 lines of GraphQL living in its own crate and a
world-creation hook the server runs without knowing whose it is; a failing
pack surface is contained and names the pack; and `packs/systems/README.md`
describes what a pack may declare, with a check that its references exist.

**`check-system-registry.mjs` reports zero violations and nothing exempted.**
The `KNOWN` list is empty for the first time since it was written.

---

## Phase 6: Polish & Cross-Cutting Concerns

- [~] T062 **Deferred to the playtest pass (2026-09-03), not dropped.** Run `quickstart.md` by hand, end to end — §1's "open a dialog", §3 step 4's light/dark check, §6 step 4's derived-value editability check. Constitution V still wants a person here; the decision is that it happens once, across every spec, after the spec list is wrapped rather than as a gate on each one. See [Manual passes](#manual-passes-deferred-to-the-playtest).
- [~] T063 **Deferred to the playtest pass**, as T062. The SC-002 walk: under both packs, confirm 100% of available actions, permissions and displayed values are identical and only presentation differs.
- [X] T064 [P] Update `MVP.md` and `docs/adrs/README.md` where either describes the interface-pack field as unused, or 5e's presentation as living in the app
- [X] T065 Run `pnpm verify` and fix what it reports **in the code this feature added**. Keep it to that; wide passes get their own commit

---

## Manual passes, deferred to the playtest

Decided 2026-09-03. Constitution V says hand-verification is not optional, and
this does not change that — it changes **when**. These passes were gating each
spec's completion individually, which meant either stopping the spec list to
run them piecemeal or leaving specs open indefinitely. They are marked `[~]`:
not done, not dropped, and not blocking.

They happen together, once, as a playtest across the whole product after the
current spec list is wrapped. That is also the better test: SC-002's "only
presentation differs" and §1's "open a dialog" are judgements about a product
being *used*, and using it for an hour surfaces more than walking a checklist
per feature.

What is deferred here, and where it lives:

| Pass | Spec |
|---|---|
| `quickstart.md` end to end, including the light/dark and derived-value checks | 032 T062 |
| The SC-002 walk under both packs | 032 T063 |
| The playability quickstart | 031 T077 |
| Wall passability, torch placement, left-click behaviour | 003 T007–T009 |
| The canvas-authoring e2e run by hand | 002 T040 |

Nothing mechanical is deferred with them. Every one of these specs' automated
checks passes now, and the e2e suite covers what a suite can.

---

## Deferred

- **User Story 2 — no longer deferred.** It was deferred as scope and never gated: ADR-029 (2026-09-03) governs loading *third-party* code, `packs/systems/*/server` are Cargo workspace members compiled into the product, and the ADR states plainly that a bundled pack may contribute behaviour. It is now [Increment F](#phase-5d-increment-f--user-story-2-priority-p2). What remains deferred out of it is **FR-013 and FR-014** — a pack contributing an *editing* surface and an items/inventory presentation of its own. F contributes one behaviour and contains one surface; the surface catalogue is a separate increment.
- **Spec 031 T076 — system-supplied turn structure.** **Never actually inside the gate.** FR-031 says turn structure is "determined by the active game system", and SC-011 that a system without rounds shows no round counter — which is a manifest *declaration* plus conditional rendering, in the shape `abilities`, `resources` and `movement` already use. No pack code is involved, and the mechanism it needed shipped with Increment A.
- **The system-pack half of User Story 3 (FR-019 to FR-021, and the success criteria that measure it — SC-006 and SC-007).** Degrading a world is a different problem from degrading a look. Named with its criteria because an audit of SC coverage otherwise finds two with no tasks and no reason, which reads as an oversight rather than a decision.
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
- **Increment E (5b)**: needs C
- **Increment F (5d)**: needs A (the contribution mechanism) and C (a mounted surface to contain). Independent of D and E
- **Polish (6)**: needs C and D

### Within Increment A

T006 blocks T007, T008, T011, T012. T009 and T010 are independent of the rest
and of each other. T012 blocks T013 and T016. T014 blocks T015.

### Within Increment C

The server half (T029–T035) and the web half (T036–T048) meet only at
`contracts/graphql-appearance.md` and can run in parallel. T037 blocks T038,
which blocks T039, T040, T043 and T056. T041 blocks T042. T045 needs T036 and T034.

### Within Increment F

Its four parts are independent of each other and can run in any order or at
once. **F1**: T085 blocks T088, which blocks T089, T090 and T091; T086 blocks
T085's final shape; T087 and T092 are independent; T093 wants T085 and T086
decided. **F2** is strictly serial — T094 blocks T095 blocks T096 blocks T097
blocks T098 blocks T099 — and T100 needs T096. **F3**: T101 blocks T102 blocks
T103. **F4**: T104 blocks T106; T105 is independent, and T104 wants T086 and
T096 settled so it documents what is true.

### Parallel opportunities

- T001, T002, T003, T005 across Setup
- T008, T015, T016 within A; T009 and T010 alongside anything
- T020/T021, T028 within B
- T031, T036, T044, T049 within C
- T056, T058, T059 within D
- F1, F2, F3 and F4 alongside each other — four people, four parts, one merge
- T091, T092, T100, T103, T105, T106 within F
- The whole server half and the whole web half of Increment C, with two people

---

## Implementation Strategy

### MVP

Setup, A, B, C. At T050 a Game Master can dress the table and an actor renders
in its system's shape. Stop and validate before D.

### Where to start in Increment F

**F1 first.** It is the smallest of the four, it closes SC-004's measurable
half, and it deletes the last hardcoded list of systems in the product. F3 and
F4 are independent of everything and can be picked up by anyone at any time.

**F2 last, and deliberately.** It is the only part that needs a decision rather
than typing, its cheap answer produces exactly the schema duplication five
increments have been spent removing, and it is the one part where a rushed
choice is worse than no choice. T094 is a spike whose output is evidence; do
not skip it to T096.

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
