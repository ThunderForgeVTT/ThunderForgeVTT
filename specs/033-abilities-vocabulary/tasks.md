---

description: "Task list for An Open Ability Vocabulary and a Guarded System Switch"
---

# Tasks: An Open Ability Vocabulary and a Guarded System Switch

**Input**: Design documents from `/specs/033-abilities-vocabulary/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: Included, and not optional. SC-003 says the contribution property is
proved "by an automated repository check"; SC-004 says four systems render
their own tab set, "verified by end-to-end tests that run per system"; SC-007
says the two-confirmation rule holds "including attempts that call the
operation directly without the interface". Those are requirements that name
their own verification, so the tests are part of the feature rather than
follow-up.

**Organization**: Six increments from plan.md. A is a prerequisite with nothing
a GM can see. B is the MVP checkpoint. C is independently deliverable and is
placed third deliberately — it needs none of the vocabulary work.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to

## Path Conventions

`src/server/src/` (the server **library** — spec 032 made it one), `src/app/`
(the binary that composes it), `packs/systems/*/system.json` (manifests),
`apps/web/` (React chrome + Playwright), `scripts/` (the gates).

**A note on naming.** The code calls this concept `classification`, not "type"
— `world_abilities.classification`, `AbilityClassification`,
`resolveAbilityLabel`. The spec says "type" throughout. They are the same
thing. Tasks below use the code's word for code and the spec's word for
behaviour, and deliberately do not rename forty call sites for a feature about
vocabulary.

---

## Phase 1: Setup

- [X] T001 [P] Write the ADR for contributed ability vocabulary in `docs/adrs/` — that a system owns the words for abilities, that the set of types is the union of built-ins and declarations, and that **`world_abilities`' CHECK constraint is therefore removed** in favour of validation at the mutation boundary. Record why both cannot survive (FR-012/SC-003 forbid a migration per new type) and why a table-wide constraint could not express the actual rule anyway (FR-013 is a per-world question). Index it in `docs/adrs/README.md` (Constitution IV, research §2)
- [X] T002 [P] Write the ADR for the guarded system switch in `docs/adrs/` — that an operation which *looks* destructive and is not is guarded by a counted acknowledgement rather than a boolean, that the acknowledgement is a digest over the counts so "I agree" means "I agree to **these** numbers", and that the guard is enforced server-side because a guard that exists only in the dialog is not a guard (Constitution III). Index it in `docs/adrs/README.md` (research §5)

---

## Phase 2: Increment A — The vocabulary has one home (foundational)

**Goal**: one assembled answer to "what does this world call its abilities",
replacing six independent readers. Nothing looks different.

- [X] T003 Create `src/server/src/ability_vocabulary.rs` with the four built-ins (`spell`, `feat`, `power`, `talent`), `AbilityTypeDeclaration` and `AbilityVocabulary` per `data-model.md`, and a `from_manifest(&serde_json::Value)` half that is filesystem-free. Follow `src/server/src/attributes.rs:32-80` exactly — it is the established shape for reading a manifest block
- [X] T004 Add `for_system(systems_dir, system_id)` to the same file, reading `<systems_dir>/<id>/system.json`, returning the built-in vocabulary on any read or parse failure rather than erroring (FR-016, SC-013). Register the module in `src/server/src/lib.rs`
- [X] T005 Implement the assembly rules from `data-model.md`: a declaration matching a built-in id **re-labels** it and produces one type not two (FR-014); a malformed entry is skipped while the rest of that system's vocabulary survives (FR-016); a missing label falls back to the id and never to blank (FR-016); ordering is the declared `order`, then declaration order, with built-ins first
- [X] T006 Implement **FR-011a** — a built-in is *present* when the active system declares or re-labels it, **or** when the world holds at least one ability of that type. This is why the vocabulary is assembled per world rather than cached per system; take the set of classifications in use as an argument rather than querying inside the assembler, so the pure half stays testable
- [X] T007 Read the legacy `abilityFacets` block as a labels-only `types` list, so `packs/systems/genie/system.json:42-51` keeps working untouched. Where both blocks are present, `abilityVocabulary` wins for the ids it covers (contract § *Backwards compatibility*)
- [X] T008 [P] Unit-test assembly in `src/server/src/ability_vocabulary.rs`: re-labelling produces one type; a malformed entry loses only itself; a blank label falls back to the id; FR-011a's presence rule in all four combinations (declared/not × held/not). **Mutation-test each** — break the rule, watch the right test fail with the right message
- [X] T009 Expose the assembled vocabulary on the world's GraphQL type in `src/server/src/graphql/`, so one request gets the world's abilities and the words for them together
- [X] T010 Create `apps/web/src/abilities/vocabulary.ts` as the single web reader of the assembled vocabulary, with the total, never-throwing lookup behaviour `apps/web/src/utils/abilityFacets.ts:78-124` already implements
- [X] T011 Replace the six independent manifest readers with it: `apps/web/src/pages/world/compendium/WorldCompendiumPage.tsx:100-123`, `compendium/AbilityCompendiumTab.tsx:21`, `compendium/AbilityPreviewPanel.tsx:9`, `apps/web/src/pages/world/ability/AbilityDetailPage.tsx:39`, `apps/web/src/pages/world/actor/ActorAbilitiesPanel.tsx:18`, and the shared-ability page. FR-006 requires all six to agree; six readers is six chances not to
- [X] T012 Delete `apps/web/src/utils/abilityFacets.ts` and move its tests to the new reader's, keeping every fallback case they cover
- [X] T013 [P] Vitest for `apps/web/src/abilities/vocabulary.ts`: label and plural resolution, id fallback, and an absent vocabulary producing the built-in words

**Checkpoint**: genie still shows Scrolls and Knacks — now from the server —
and `abilityFacets` has exactly one reader.

---

## Phase 3: Increment B — User Story 1 (Priority: P1) 🎯 MVP

**Goal**: the compendium breaks out by type, in the system's own words.

**Independent test**: seed a world with abilities of two types; set its system
to one supplying vocabulary and one supplying none; confirm both produce a
tabbed, fully-labelled set.

- [X] T014 [US1] Replace the flat table in `apps/web/src/pages/world/compendium/AbilityCompendiumTab.tsx` with a nested tab set — one tab per present type, in the vocabulary's order, each carrying the system's plural label. Reuse `apps/web/src/components/ui/tabs/Tabs.tsx`; it takes a `TabsItem[]` and supports a controlled value
- [X] T015 [US1] Show a count on each tab equal to the rows that tab lists (FR-007). The count is of what *this viewer* may see, so it is computed from the same filtered set the table renders, never from a separate total
- [X] T016 [US1] Drop the `<th>Type</th>` column and its cell (`AbilityCompendiumTab.tsx:205,235-238`) — the tab is the type now, and keeping both says it twice
- [X] T017 [US1] Creating from within a tab defaults the new ability to that type without asking (FR-008), replacing the `useState("SPELL")` default at `AbilityCompendiumTab.tsx:66-67`
- [X] T018 [US1] An empty tab states it is empty and offers creation, and does **not** fall back to listing other types (FR-009)
- [X] T019 [US1] Apply the umbrella term wherever the concept is named: the outer compendium tab (`WorldCompendiumPage.tsx:225-226`, currently the literal `"Abilities"`), the page heading, the create control, empty states and confirmation text (FR-003, FR-006). SC-002 measures **zero** built-in words on any ability surface, which is written to catch relabelling the tab and leaving "Add ability" alone
- [X] T020 [US1] Confirm the tab set and its labels are identical for GMs and players, only the abilities within differing (FR-010)
- [X] T021 [P] [US1] Update `apps/web/e2e/abilities-compendium.spec.ts`, which asserts against `ability-catalog-table` and a flat list and will not survive the tab set. Keep every behaviour it covers
- [X] T022 [US1] Add `apps/web/e2e/abilities-vocabulary.spec.ts` running **per system** across genie, dnd5e, pathfinder2e and blades_in_the_dark (SC-004), asserting each renders its own tab set from its own declarations — and that `blades_in_the_dark`, which declares nothing, still gets a complete, fully-labelled built-in set with no blank tab and no error (SC-013). That last case is the one most likely to break under a change only ever tested against genie

**Checkpoint**: **demonstrable.** A 5e GM sees Spells and Feats; a Genie GM
sees Scrolls and Knacks; a Blades GM sees the built-in words, complete.

---

## Phase 4: Increment C — User Story 2 (Priority: P2)

**Goal**: changing a world's system tells the truth in numbers and asks twice.
Independent of A and B.

**Independent test**: a world with content cannot switch in fewer than two
deliberate actions, including from a direct GraphQL call; a world without
content still switches in one.

- [X] T023 [US2] Add `worldContentInventory(worldId, targetSystemId)` in a new `src/server/src/graphql/queries/world_content.rs` per `contracts/system-change-guard.md`, returning per-kind counts with the system each was authored under, `isEmpty`, and a digest. **DM-only** — the counts describe content a player may not be able to see
- [X] T024 [US2] Count **actors, abilities and items only** (FR-029 as clarified). Scenes and lore are excluded and the code says why: every world is created with a default scene (spec 010), so counting scenes would make no world ever empty, FR-029 unreachable, and the red warning appear on a world a GM made a minute ago
- [X] T025 [US2] Compute the digest over the counts, stable across ordering, so the acknowledgement means "I acknowledge **these** numbers" rather than "I clicked something"
- [X] T026 [US2] Add `acknowledgedDigest` to `UpdateWorldGameSystemInput` (`src/server/src/graphql/input_types.rs`) and enforce it in `update_world_game_system_impl` (`src/server/src/graphql.rs:2009-2066`) in the contract's order: authorization (FR-031) → no-op check (FR-030) → recompute inventory → require a matching digest when the world has content (FR-028) → apply. Absent **or stale** is refused
- [X] T027 [US2] Record a world event on a successful change, which this mutation does not do today — unlike its sibling `update_world_interface_pack_impl` at `graphql.rs:1945-2007`. A world's system changing is at least as worth announcing as its palette
- [X] T028 [US2] Build the red warning panel in `apps/web/src/pages/world/settings/WorldSystemSettingsPage.tsx`: counts by kind, systems named by **display name** not id, the target system named, that content becomes **hidden not destroyed** and switching back restores it, and what will be presented differently rather than hidden (FR-025)
- [X] T029 [US2] Word it so it never overstates (FR-026). It must not say "delete", "lose" or "destroy", because none of those is what happens — and a false warning teaches GMs to distrust every warning the product shows them
- [X] T030 [US2] Require a second, distinct confirmation naming the target system (FR-027). **Do not reuse** the existing single confirmation at `WorldSystemSettingsPage.tsx:324-339` — it exists for spec 016's legal notice, and making one control mean both "I read the licence" and "I accept this data consequence" weakens both
- [X] T031 [US2] A world with no content keeps the one-step path, with no red panel and no second confirmation (FR-029); selecting the active system is a silent no-op (FR-030); cancelling at either step leaves system and content unchanged (FR-032)
- [X] T032 [US2] After applying, state what became hidden and how to restore it (FR-033)
- [X] T033 [P] [US2] Server tests in `src/server/src/graphql.rs`: absent digest refused; **stale** digest refused (take one, add an actor, send it); empty world switches without one; no-op returns unchanged; a non-DM is refused regardless of acknowledgement (FR-031). Mutation-test the digest check by making it always pass
- [X] T034 [P] [US2] Add `apps/web/e2e/system-change-guard.spec.ts`: counts match seeded content exactly (SC-006); two confirmations required (SC-007); cancel leaves everything unchanged; content counts identical before and after (SC-005); switch away and back restores visibility with nothing renamed (SC-008)

**Checkpoint**: a GM cannot lose track of what a system change costs, and a
script cannot skip the question.

---

## Phase 5: Increment D — User Story 3 (Priority: P3)

**Goal**: a system names its own types. This is where the closed lists go.

**Independent test**: declare a new type in one system's pack; confirm it
appears as a tab in that system's worlds, is not offered in another system's,
and that no file shared with other systems was modified.

- [X] T035 [US3] Write the migration in `src/server/migrations/` dropping `world_abilities_classification_check`. **No data migration** — every existing row holds a built-in, which stays built in (FR-017, SC-012). The down migration restores the constraint **only if** every row still holds a built-in, and otherwise fails loudly rather than deleting somebody's abilities to make a constraint fit (data-model § 1)
- [X] T036 [US3] Retire the `AbilityClassification` GraphQL enum (`src/server/src/graphql/types.rs:1046-1077`); the wire type becomes a string identifier described by the vocabulary the same request can fetch. A GraphQL enum publishes a closed set and a client validating against it would reject a pack's own type
- [X] T037 [US3] **Fix the lossy fallback.** `src/server/src/graphql/types.rs:1210` and `src/server/src/graphql/mutations_ability_shares.rs:114` read an unknown stored value as `unwrap_or(AbilityClassification::Spell)`, silently presenting an unrecognised ability as a Spell — which FR-034 forbids in as many words. Nothing can write a fifth value today; T035 makes it possible, so this is fixed in the same increment that creates the possibility
- [X] T038 [US3] Replace the TypeScript union `apps/web/src/types/ability.ts:14` with a string identifier, and the nullable actor-side variant in `apps/web/src/types/actorAbility.ts` with it
- [X] T039 [US3] Validate `classification` against the **world's assembled vocabulary** in `src/server/src/graphql/mutations_abilities.rs` (create at `:27,:276`; update at `:43,:337-340`). This is where "valid" now lives, and it is the only place that can answer the per-world question the dropped constraint never could
- [X] T040 [US3] Refuse authoring a type the active system does not recognise (FR-013), while leaving existing abilities of that type readable and editable — the two are different questions and only the first is refused
- [X] T041 [US3] Report an irreconcilable identity collision **when the vocabulary is assembled** (FR-015), not when a GM first authors one of the colliding types
- [X] T042 [US3] Add `scripts/check-ability-vocabulary.mjs` and wire it into `scripts/verify.mjs` as a new step. Model it on `scripts/check-system-registry.mjs`, which derives the ids it forbids **from `packs/systems/` itself** rather than a list in the script, so it cannot go stale: collect every type identity any pack declares, subtract the built-ins, and fail if one appears as a literal in shared server or web code (SC-003, FR-012)
- [X] T043 [US3] Declare `enchantment` in `packs/systems/dnd5e/system.json` — an item-bound type 5e actually has — as the worked example SC-003 is measured against, and the thing that proves the property rather than asserting it
- [X] T044 [US3] **Audit for the four literals.** Grep `"spell"`, `"feat"`, `"power"`, `"talent"` (and `SPELL`/`FEAT`/`POWER`/`TALENT`) across `src/server/src`, `src/app/src` and `apps/web/src`. The survey found two `unwrap_or` sites; the plan assumes more. This is the guard against the CHECK constraint's removal breaking something quietly, and it is a grep rather than a reading of the types
- [X] T045 [P] [US3] Tests: a declared type is accepted in its own system's world and refused in another's (FR-013); a colliding declaration is reported at assembly; an unknown stored value is **not** rendered as a Spell (the T037 regression); the built-ins remain authorable everywhere (FR-017)

**Checkpoint**: `dnd5e` declares Enchantment, it appears as a tab, and
`check-ability-vocabulary.mjs` passes with zero changes to any shared file.

---

## Phase 6: Increment E — User Story 4 (Priority: P4)

**Goal**: types declare what they bind to and how they are graded.

**Independent test**: declare a character-bound graded type and an item-bound
ungraded type; confirm the grade records and displays in the system's word, and
that each binding refuses the other subject.

- [X] T046 [US4] Write the migration adding `world_abilities.grade INTEGER NULL` and creating `world_item_abilities` per `data-model.md` § 3 — the item peer of `world_actor_abilities` (`src/server/src/schema.rs:489-497`), mirroring its nullable `ability_id` plus name snapshot so a deleted ability leaves a tombstone rather than vanishing. `created_by`/`updated_by` per Constitution III
- [X] T047 [US4] Parse `binds` in `ability_vocabulary.rs` as **exactly one** of `character`, `item` or `nothing`, never a list (FR-018 as clarified), defaulting to `character` when absent
- [X] T048 [US4] Enforce the binding at the data boundary (FR-019, SC-011) in `src/server/src/graphql/mutations_actor_abilities.rs` and the new item-attachment mutations. Because a type binds to exactly one subject, this is a comparison rather than a set membership test
- [X] T049 [US4] Add the item attachment mutations and query beside the existing actor ones, following `mutations_actor_abilities.rs`'s shape (`attach_*_impl`, `detach_*_impl`, resolvers) rather than inventing a second one
- [X] T050 [US4] Parse `grade` as `{ label, min, max }`, refuse an out-of-range value at authoring (FR-023), and **retain** a stored value that falls outside a *newly narrowed* range — never clamp or discard it. A system narrowing a range does not get to edit content authored under the old one
- [X] T051 [US4] Display the grade in the system's word for it on every surface showing the ability, and show **no** grade for ungraded types (FR-022, SC-010)
- [ ] T052 **Server half done** (`itemAbilities` query, `attachAbilityToItem`); the item page does not render them yet. [US4] Show item-bound abilities on the item, listed with that item's existing mechanical effects, each identified as what it is and not duplicated (FR-020). `world_item_effects` is **not** merged — an effect is a rule the resolution layer consumes, an ability is named, permissioned, shareable content, and they are reconciled in presentation only
- [X] T053 [P] [US4] Tests: grade refused outside range; out-of-range retained after a range narrows; item binding refused for a character and vice versa, **through the API rather than the interface** (SC-011)

**Checkpoint**: a 5e Spell has a Level and binds to a character; an Enchantment
binds to an item and appears on that item beside its effects.

---

## Phase 7: Where the halves meet — unrecognised types

**Goal**: an Enchantment survives the world ceasing to be 5e. Needs D (types
can be unrecognised at all) and C (the counts must include them).

**This phase spans two stories, which is why the spec says neither half can
answer it alone.** Tasks are labelled by the story each serves: presenting an
unrecognised type is User Story 3's vocabulary continuing to hold after the
world changed; counting what will become unrecognised is User Story 2's
warning telling the truth.

- [X] T054 [US3] Detect abilities whose stored classification is absent from the world's assembled vocabulary. A **presentation state** computed per request, never a stored flag — nothing about the row changes when it enters or leaves it (FR-034)
- [X] T055 [US3] Present them as a **final tab** in the same tab row (FR-035a), present only while such abilities exist, clearly marked, carrying a count like any other tab
- [X] T056 [US3] Label each with the **stored identity itself**, plainly (FR-035 as clarified). Do not read another system's manifest to prettify it — that consults a system this world is not running — and do not copy a label onto the ability, which duplicates the manifest into content where it goes stale
- [X] T057 [US3] Offer **no creation** in that tab (FR-035a), since FR-013 forbids authoring a type the active system does not recognise. Keep every other action: open, edit, delete (FR-034)
- [X] T058 [US3] Restore such abilities to their own tab, with the system's labels, when a system recognising them is active again (FR-036, SC-008)
- [X] T059 [US3] Let a GM re-type one deliberately to a recognised type (FR-038), and never do it for them — the alternative is a silent, lossy, irreversible edit to authored content performed by a dialog somebody clicked through
- [X] T060 [US2] Include abilities that will **become** unrecognised in the FR-025 counts (FR-037). This is the part of the inventory that needs the *target* system's vocabulary, and the reason `worldContentInventory` takes a `targetSystemId`
- [X] T061 [P] [US2] Add the round trip to `apps/web/e2e/system-change-guard.spec.ts`: author an Enchantment in a 5e world, switch to genie acknowledging the warning, confirm it is still listed under the unrecognised tab labelled `enchantment` and **not** shown as a Spell, then switch back and confirm it returns to its own tab unchanged (SC-008, SC-009)

**Checkpoint**: switching away and back is lossless, and the warning said so
beforehand.

---

## Phase 8: Polish & Cross-Cutting Concerns

- [ ] T062 Run `quickstart.md` by hand end to end — including §1 step 7 (a system that declares nothing) and step 8 (FR-011a's presence rule), which no suite answers. Constitution V
- [ ] T063 Run `pnpm verify` and fix what it reports **in the code this feature added**. Keep it to that; wide passes get their own commit
- [ ] T064 [P] Update `MVP.md` where it describes the abilities compendium as a flat list with a Type column

---

## Dependencies & Execution Order

### Phase dependencies

- **Setup (1)**: none. Both ADRs can be written while A is built
- **Increment A (2)**: blocks B, D and F — everything that needs to know what a world calls its abilities
- **Increment B (3)**: needs A. MVP checkpoint
- **Increment C (4)**: **needs nothing.** Independently deliverable, and placed third because it is the half a GM can lose confidence over today. It gains T060 from F afterwards
- **Increment D (5)**: needs A
- **Increment E (6)**: needs D — a facet is declared on a type, and D is where a system may declare one
- **Phase 7**: needs D and C
- **Polish (8)**: needs everything

### Within Increment A

T003 blocks T004, T005, T006, T007. T005 and T006 block T008. T009 needs T005.
T010 blocks T011 and T012. T013 needs T010.

### Within Increment C

The server half (T023–T027, T033) and the web half (T028–T032, T034) meet only
at `contracts/system-change-guard.md` and can run in parallel. T023 blocks T025
blocks T026. T028 blocks T029 and T030.

### Within Increment D

T035 blocks T039. T036 blocks T038. T037 is independent and should land early —
it is a bug fix, not a feature. T042 and T043 block each other's meaning: the
check is only proved by a pack that exercises it.

### Parallel opportunities

- T001 and T002 across Setup, alongside anything
- T008 and T013 within A
- T021 and T022 within B
- The whole server half and the whole web half of Increment C, with two people
- T033, T034, T045, T053, T061, T064 alongside their phases

---

## Implementation Strategy

### MVP

Setup, A, B. At T022 a GM opens the compendium and finds their rulebook's
sections in their rulebook's words. Stop and validate before C.

### Why C is third and not last

User Story 2 is P2 and depends on none of the vocabulary work. It is also the
only half of this feature that addresses something a GM can be hurt by today: a
world's system changes on one click, with no warning and no count, and the
operation looks destructive even though it is not. Shipping it early buys
confidence that the rest of the feature spends.

### Why A has nothing to show

Increment A moves six independent manifest readers behind one assembled answer
and changes no pixels. It exists because FR-006 requires every ability surface
to agree, and because the server needs the vocabulary regardless — FR-013,
FR-019 and FR-023 are all refusals only it can make. Doing B first would mean
building the tab set against a vocabulary that then moves.

### Notes

- [P] = different files, no dependencies. Commit after each task or logical group
- **The riskiest task in the feature is T044**, and it is a grep rather than a
  design. Removing the CHECK constraint means four literals stop being the only
  possible values, and every place that assumed otherwise has to be found before
  D is called done
- Verify per crate against its real target: native `cargo test -p thunderforge-server`
  for the server library, `cargo test -p <pack>-server` for a pack, `tsc` and
  vitest for web (Constitution V). Note `cargo test -p thunderforge` now runs
  the **binary's** tests only — spec 032 split the crate
