# Implementation Plan: An Open Ability Vocabulary and a Guarded System Switch

**Branch**: `033-abilities-vocabulary` | **Date**: 2026-09-03 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/033-abilities-vocabulary/spec.md`

## Summary

Two halves sharing one seam. A game system supplies the words for abilities —
the umbrella term and the name of each type — and the compendium breaks out
into a tab per type in the system's own vocabulary. Separately, changing a
world's system stops being a one-click action on a world full of content and
becomes a counted, red, twice-confirmed operation guarded at the data boundary.
They meet at the question neither half can answer alone: what happens to an
Enchantment when the world stops being 5e.

**The spec was written on 2026-09-01. Spec 032 landed on 2026-09-03 and moved
the ground under it — favourably.** A pack can now own tables, contribute
GraphQL and contribute behaviour, and the server is a library packs depend on.
"Contributed, not central" is no longer a shape to invent; it is the shape the
codebase already has, three times over.

## Technical Context

**Language/Version**: Rust 1.98 (server library, pack crates), TypeScript 5.x /
React 19 (web)

**Primary Dependencies**: `thunderforge-server` as the library packs depend on
(spec 032); the manifest-reading pattern of `attributes.rs`, `sheet.rs`,
`turn_structure.rs` and `status_display.rs`; `inventory` for contribution;
async-graphql + Diesel

**Storage**: PostgreSQL. **One migration is required and it is a removal** —
see [The crux](#the-crux-a-check-constraint-is-a-central-list) below. Two
further columns for User Story 4; no data migration and no re-typing anywhere.

**Testing**: `cargo test -p thunderforge-server`, `cargo test -p <pack>-server`,
`vitest`, Playwright per system

**Target Platform**: Chromium-only, per the constitution

**Project Type**: Web application — Rust backend, React frontend, pack crates

**Performance Goals**: The content inventory behind the warning is one query
per world, run when a GM opens a confirmation dialog. Not a hot path.

**Constraints**: No re-typing of authored content, ever, by anything other than
a GM's deliberate act. No blank labels under any pack failure. The tab set is
identical for GMs and players.

**Scale/Scope**: Four bundled systems already exercised end-to-end must each
render their own tab set (SC-004).

### What the survey found, that the spec could not know

Three findings change the shape of the work, all verified in the code rather
than assumed.

**1. The concept is called `classification`, not "type".** `world_abilities.classification`,
`AbilityClassification`, `resolveAbilityLabel`, `abilityFacets`. The spec says
"ability type" throughout. These are the same thing, and the plan keeps the
code's word in the code and the spec's word in prose rather than renaming 40
call sites for a vocabulary feature. **Where a user can see it, the word is the
system's anyway** — which is the whole point of User Story 1.

**2. There is already a lossy fallback that FR-034 forbids.**
`src/server/src/graphql/types.rs:1210` and
`mutations_ability_shares.rs:114` both read an unknown stored classification as
`unwrap_or(AbilityClassification::Spell)`. Its comment is reasonable — "a row
written by a newer version must not break an older reader" — and the effect is
exactly what FR-034 rules out: an ability of an unrecognised type is silently
presented as a Spell. Nobody has hit it because nothing can currently write a
fifth value. User Story 3 makes it writable, so this is a live bug the moment
that lands, and it is fixed in the same increment rather than after.

**3. Ability labels are web-only, and read separately by six components.**
`WorldCompendiumPage`, `AbilityCompendiumTab`, `AbilityPreviewPanel`,
`AbilityDetailPage`, `ActorAbilitiesPanel` and `SharedAbilityPage` each fetch
the manifest and cast `abilityFacets` themselves. The server knows nothing
about ability vocabulary. That is six chances to disagree, and FR-006 requires
that none of them do.

### The crux: a CHECK constraint is a central list

```sql
classification VARCHAR(16) NOT NULL
    CHECK (classification IN ('spell', 'feat', 'power', 'talent'))
```

That is `src/server/migrations/2026-08-25-120000-0000_create_world_abilities/up.sql:19-20`,
and it is the same shape of problem spec 032 spent six increments removing: a
closed list, in shared code, that every new system must edit.

FR-012 and SC-003 require that adding a type for one system changes **zero**
files shared with other systems. A constraint enumerating valid types fails
that by construction — a fifth type means a migration, and a migration is as
shared as a file gets. There is no arrangement in which both survive.

**So the constraint goes.** Validity stops being a database enumeration and
becomes what the assembled vocabulary says, enforced where every other
authoring rule in this codebase is enforced: server-side, at the mutation. That
is Constitution Principle III's position, and it is also the only one that can
express "valid *in this world*, because of the system it is running" — which a
table-wide constraint cannot say at all.

Two closed lists sit behind it and go the same way: `AbilityClassification` as
a GraphQL enum (`types.rs:1046`) and `AbilityClassification` as a TypeScript
union (`apps/web/src/types/ability.ts:14`). A GraphQL enum cannot carry a value
a pack invented, so the wire type becomes a string identifier with the
vocabulary describing it — the same move `DeclaredValue` made in spec 032, for
the same reason.

**This is the increment's main risk and its main cost.** It is not a large
diff; it is a change to what "valid" means, and everything downstream that
assumed four values has to be found. The survey found the two `unwrap_or` sites;
the plan assumes there are more.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design.*

| Principle | Assessment |
|---|---|
| **I. ECS owns simulation, React owns chrome** | **Pass, not engaged.** Abilities are compendium content. Nothing here draws on the canvas, and no ability is an entity. |
| **II. Plugin-modular engine architecture** | **Pass, and this is the principle the feature serves.** A system's vocabulary is contributed by the system, and the application holds no per-system knowledge — the same property FR-029 established for system packs and ADR-054 for interaction effects. The engine itself is untouched. |
| **III. Ownership & authorization at the data boundary** | **Pass, and load-bearing twice.** FR-028's acknowledgement and FR-019's binding refusal are both explicitly required to hold against a direct call, not just in the dialog. Removing the CHECK constraint moves validation *to* this boundary rather than away from any. New columns carry existing provenance conventions. |
| **IV. Real ADRs and specs before divergent implementation** | **Pass, two ADRs required.** One recording that ability vocabulary is contributed and that the CHECK constraint is therefore removed in favour of boundary validation. One recording that a destructive-looking, non-destructive operation is guarded by counted acknowledgement. The spec's own constitution note already anticipates the first. |
| **V. Verify before claiming done** | **Pass.** Server natively, web via `tsc` and vitest, and per-system Playwright — SC-004 names four systems, which makes "exercised in a running instance" a suite rather than a habit. |

**DMCA / content-moderation guardrail**: not triggered, and FR-039 exists to
keep it that way — nothing here makes one world's content visible from another.
Existing per-ability share links are unchanged in scope.

**Result: PASS.** Complexity Tracking is empty. See [Risks](#risks).

## Project Structure

### Documentation (this feature)

```text
specs/033-abilities-vocabulary/
├── plan.md              # This file
├── research.md          # Phase 0
├── data-model.md        # Phase 1
├── quickstart.md        # Phase 1
├── contracts/
│   ├── ability-vocabulary.md    # what a system declares
│   └── system-change-guard.md   # the counted, acknowledged mutation
├── checklists/requirements.md
└── tasks.md
```

### Source Code (repository root)

```text
src/server/src/
├── ability_vocabulary.rs        # NEW — assemble built-ins + the system's declarations
├── graphql/mutations_abilities.rs      # validate against the vocabulary, not an enum
├── graphql/mutations_actor_abilities.rs # binding facet enforcement
├── graphql/queries/world_content.rs    # NEW — the content inventory (FR-025)
├── graphql/types.rs             # AbilityClassification enum retired
└── graphql.rs                   # update_world_game_system_impl gains acknowledgement

src/server/migrations/
└── <new>_open_ability_classification/  # drop the CHECK; add grade + binding columns

packs/systems/*/system.json      # gain an `abilityVocabulary` block

apps/web/src/
├── abilities/vocabulary.ts      # NEW — one reader, replacing six
├── pages/world/compendium/AbilityCompendiumTab.tsx   # tab set per type
├── pages/world/settings/WorldSystemSettingsPage.tsx  # the red double confirmation
└── utils/abilityFacets.ts       # folded into the above

scripts/check-ability-vocabulary.mjs   # NEW — SC-003, modelled on check-system-registry
```

**Structure decision**: the vocabulary is assembled **server-side**, in
`ability_vocabulary.rs`, following `attributes.rs` exactly — a
`for_system(systems_dir, system_id)` reader over a filesystem-free
`from_manifest(&Value)` half, total, defaulting rather than erroring. The web
consumes the assembled answer instead of casting the manifest itself.

That is a change from today's arrangement and it is deliberate. Six components
each casting `abilityFacets` is six chances to disagree about what FR-006 says
they must agree on; and the server needs the vocabulary regardless, because
FR-013, FR-019 and FR-023 are all refusals it has to make. Assembling it twice,
in two languages, to two possibly different answers, is the thing to avoid.

## Increments

Six, each a checkpoint. User Story 2 is deliberately early and out of
dependency order, because it is independently deliverable, it is the half a GM
can lose data-confidence over today, and it needs none of the vocabulary work.

### A — The vocabulary has one home (foundational)

`ability_vocabulary.rs`: built-in types, a manifest block a system may declare,
and the assembled answer exposed on the world. The six web call sites collapse
to one reader. No new types yet, no tabs yet — the same four classifications,
the same labels, arriving by one path instead of six.

*Checkpoint*: genie still shows Scrolls and Knacks, from the server. Nothing
looks different, and `abilityFacets` has exactly one reader.

### B — User Story 1: the compendium breaks out by type 🎯 MVP

A tab per recognised type, in the system's order, with the system's plural
label and a count that matches its rows; create-in-tab pre-selects; empty tabs
say so. The umbrella term replaces "Abilities" on the outer tab, the heading
and the create control.

*Checkpoint*: **demonstrable.** A 5e GM sees Spells and Feats; a Genie GM sees
Scrolls and Knacks; a Blades GM sees the built-in words, complete and correct.

### C — User Story 2: the guarded switch

The content inventory query, the red dialog with real counts, two distinct
confirmations, and the server-side acknowledgement without which the mutation
refuses. Independent of A and B.

*Checkpoint*: a world with content cannot change system in fewer than two
deliberate actions, including from a direct GraphQL call; a world without
content still switches in one.

### D — User Story 3: a system names its own types

The CHECK constraint goes; validity becomes the assembled vocabulary, enforced
at the mutation. A system may declare a type of its own, re-label a built-in,
and be refused for a collision at assembly rather than at authoring. The
automated check (SC-003) lands here, and so does the `unwrap_or(Spell)` fix.

*Checkpoint*: `dnd5e` declares Enchantment and it appears as a tab, with zero
changes to any file another system shares.

### E — User Story 4: binding and grading

A type declares what it attaches to and whether it is graded. The item
counterpart of the character attachment is added; both are constrained by the
declared facet, server-side. A grade records a value, displayed in the system's
word for it, refused outside its range at authoring and retained when a range
later narrows.

*Checkpoint*: a 5e Spell has a Level and binds to a character; an Enchantment
binds to an item and appears on that item beside its effects.

### F — Where the halves meet: unrecognised types

An ability whose type the active system does not recognise stays listed,
editable and labelled with the identity it was authored under, in a marked
section. Switching back restores it to its own tab. Re-typing is available and
never automatic. The FR-025 counts include what will become unrecognised.

*Checkpoint*: switch a 5e world with Enchantments to Genie and back; nothing is
renamed, re-typed or lost, and the count in the warning said so beforehand.

## Clarifications applied (2026-09-03)

`/speckit-clarify` ran after this plan rather than before it, and five answers
landed. Three changed the design; all five are in `spec.md` § Clarifications.

- **A built-in type is shown when the system uses it, or when the world holds
  one** (FR-011a, new). The literal union in FR-011 would have given every 5e
  world permanently empty "Powers" and "Talents" tabs. Presence is now a
  function of the world as well as the system, which is why the vocabulary is
  assembled per world and cannot be cached per system.
- **"No content" means actors, abilities and items** (FR-029, tightened).
  Scenes and lore are excluded because every world is created with a default
  scene, so counting them would make FR-029's one-click path unreachable and
  put the red warning in front of a GM on a world a minute old.
- **A type binds to exactly one subject** (FR-018, tightened). FR-019's refusal
  is a comparison, not a set membership test.
- **An unrecognised type is labelled with its stored identity**, plainly
  (FR-035). No other system's manifest is read, and no label is copied onto the
  ability.
- **The unrecognised group is a final tab in the same row** (FR-035a, new),
  present only while such abilities exist, offering no creation.

## Risks

- **The CHECK constraint's removal is the part that can break quietly.** The
  four values are assumed in a GraphQL enum, a TypeScript union, two
  `unwrap_or` fallbacks, and an unknown number of places the survey did not
  reach. The guard is a grep for each of the four literals across server and
  web before D is called done — not a reading of the type.
- **Assembling the vocabulary server-side is a change of ownership**, and the
  six web call sites are live UI covered by e2e. Spec 031's history says a
  control that moves breaks tests reaching it by placeholder and accessible
  name rather than testid; `abilities-compendium.spec.ts` asserts against
  `ability-catalog-table` and will need reworking for a tab set regardless.
- **US4 is the only half needing new tables**, and no pack has ever shipped a
  migration — `find packs -type d -name migrations` returns nothing. The item
  attachment is *application* content, not pack content, so it belongs in the
  server's migrations; that is worth stating before someone tries to make it a
  pack's, on the reasonable-sounding grounds that the facet was declared by one.
- **"Umbrella term" reaches further than it looks.** FR-003 lists tabs,
  headings, creation controls, empty states and confirmation text; SC-002
  measures *zero* occurrences of the built-in word on any ability surface. The
  cheap version relabels the tab and leaves "Add ability" alone, and SC-002 is
  written to catch exactly that.

## Deferred

- **Usage mechanics** — slots, charges, cooldowns, prepared/known. A grade is a
  recorded property, not a resource (spec, and spec 025 before it).
- **Resolution or adjudication of ability effects.**
- **Canvas representation of abilities.**
- **GM-authored ability types outside a pack.** The vocabulary is the system's.
- **Automatic translation of content between systems.** FR-034's refusal is the
  whole point.

## Complexity Tracking

No constitution violations to justify. The CHECK constraint's removal *reduces*
the number of places that decide what an ability may be, from three to one.
