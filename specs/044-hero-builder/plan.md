# Implementation Plan: The Hero Builder

**Branch**: `044-hero-builder` | **Date**: 2026-09-15, re-planned 2026-09-16 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/044-hero-builder/spec.md`, decided
2026-09-14 and clarified 2026-09-16 (look only; full-screen dialog; "Build look"
on every NPC row; a race-aware dice roll whose race is pre-selected from the
sheet).

## Summary

`packages/heroes` draws a hero from a spec. Nothing drives it: the only ways to
make a hero today are to edit `presets.ts` by hand or to write code that calls
the factory (`packages/heroes/src/cli.ts`, `scripts/seed-demo-art.mjs:21`). This
plan builds the thing that drives it, in the order the owner meets it.

A **hero spec** stays the unit of currency. Phase (a) makes it editable by
looking at it, in a library of React components and a standalone app that needs
no server, with a dice roll that can be narrowed to a race. Phase (b) puts a
"Build look" control inside `ActorImageryPanel`, so it appears on every page
that mounts the panel, puts the same control on every row of the compendium's
NPC list, and adds Quick NPC beside the compendium's "New NPC" button. All three
open one full-screen dialog and save through one helper. Phase (c) gives the player who holds a character the
right to change its art — the first right in this product derived from a claim
rather than from a permission row — with a per-world setting and a per-character
lock as the two ways to withdraw it. Phase (d) stores the spec beside the image
it drew, so a saved hero re-opens as a hero.

Nothing new stores pixels. `uploadActorImage` and `storage/svg.rs` remain the
single path from a drawing to the map (FR-024), and phase (a) touches no server
at all.

| Phase | Who | What lands | Proven by |
|---|---|---|---|
| **(a)** | Us | Six additions to `packages/heroes` (labels, palettes, race looks, randomiser, minimal spec, preset source); `packages/hero-builder`; `apps/hero-builder` | `apps/hero-builder/e2e/builder.spec.ts`, no backend; `pnpm -F @thunderforge/heroes check` |
| **(b)** | A Game Master | "Build look" in `ActorImageryPanel` and on each NPC row; the race read from the sheet through the manifest; Quick NPC in the compendium | `apps/web/e2e/hero-builder-npc.spec.ts` |
| **(c)** | A player | A holder's imagery grant at the data boundary; the world setting; the per-character lock; building while creating your own character | `apps/web/e2e/hero-builder-player.spec.ts` |
| **(d)** | Everyone above | `world_actor_images.hero_spec`; re-opening a saved hero; travel through collections and the export | `apps/web/e2e/hero-builder-saved.spec.ts` |

Each phase ships alone. (b) needs (a), because the builder is what the button
opens. (c) needs (b) for the control and adds only authorisation. (d) needs (b)
for something to save, and changes the data model as (c) does.

**The owner's request of 2026-09-15 — a builder button beside the
portrait and token on an actor's edit page, now labelled "Build look" — is
phase (b), and it is the second phase.** There is no honest way to make it the first: the button opens a
builder that does not exist yet. Phase (a) is therefore scoped to the builder
and nothing else, and phase (b) puts the control in `ActorImageryPanel.tsx`
rather than in a page, so it arrives on the NPC editor and on the character page
at once, wherever that panel is mounted.

## Technical Context

**Language/Version**: TypeScript 5 with React 19 (web, the new library, the new
app). Rust (server) for phases (c) and (d) only. No engine change in any phase.

**Primary Dependencies**: `@thunderforge/heroes` (framework-free, zero runtime
dependencies, `packages/heroes/package.json:17-20`); React 19 as a **peer**
dependency of the new library; Vite (the web app's, not the sandbox's older
one); Playwright; async-graphql 7, Diesel and Postgres for (c) and (d).

**Storage**: Postgres, phases (c) and (d) only. One new column each on `worlds`
and `world_actors` (phase c) and one on `world_actor_images` (phase d). No new
table. See [data-model.md](./data-model.md).

**Testing**: `node --test` in `packages/heroes` (already `pnpm verify`'s
`heroes` step, `scripts/verify.mjs:144-147`); a Playwright spec in
`apps/hero-builder/e2e` run directly against a static preview, with no stack;
`cargo test` for the phase (c) authorisation rules and the phase (d) shape
check; three Playwright specs in `apps/web/e2e` through the usual harness.
See [quickstart.md](./quickstart.md).

**Target Platform**: Chromium. The standalone app is a static build; it is a
developer tool and is not deployed with a release (spec, Decisions 3).

**Project Type**: Existing multi-part repository. One new workspace package and
one new workspace app, both under globs `pnpm-workspace.yaml` already carries
(`apps/*`, `packages/*`).

**Performance Goals**: A control change is reflected in both previews within
100 ms (SC-006), measured in the phase (a) spec. Continuous input redraws at
most once per animation frame (FR-021). The builder adds nothing to the initial
load of any `apps/web` page that does not open it (SC-012, FR-022), which means
a dynamic `import()` at the point of opening, in the manner of
`apps/web/src/services/pdfReader.ts:72`.

**Constraints**:

- **One rasteriser.** Every stored hero goes through `uploadActorImage` as SVG
  and comes back WebP (FR-024, research R3). The builder adds no upload path.
- **No server in phase (a).** No GraphQL, no auth, no database — the property
  that lets the standalone app be a static deploy later (Decisions 3).
- **The builder carries no catalogue.** Controls come from `HERO_PARTS`,
  `HERO_COLORS`, `HERO_FLAGS` and the labels/palettes phase (a) adds to
  `packages/heroes` (FR-001, FR-002).
- **`packages/heroes` stays framework-free.** It has no dependencies today and
  gains none; the CLI, `seed-demo-art.mjs` and an e2e import it under plain Node.
- **One React.** The library takes React as a peer dependency. `apps/web`'s
  existing force-resolution of `react`, `react-dom` and `react/jsx-runtime`
  (`apps/web/vite.config.mts:41-47`, written for `@thunderforge/genie`) is the
  failure mode to avoid repeating.
- **The server decides.** Phase (c)'s grant is enforced in
  `require_actor_permission`'s neighbourhood, not in the panel (Principle III).

**Scale/Scope**: Four phases. They touch `packages/heroes` (five pure additions
plus a JSON Schema in phase d), two new workspace members, about eight files
in `apps/web`, one manifest field in `packs/systems/dnd5e/system.json`, four
server modules, two migrations, and four Playwright specs. No
new service and no new runtime dependency.

## What the code says, against what the spec assumed

The spec's context was written on 2026-09-10 and decided on 2026-09-14. Verified
against `main` at `d3b91d7` on 2026-09-15, five of its statements had moved.
Re-verified against `main` at `3101e9f` on 2026-09-16: items 1–4 still hold
(`copy.rs`'s image select now starts at `:595`), item 5 has changed, and a sixth
has appeared.
Each is a planning input, not a correction to the spec's intent.

1. **The catalogue has doubled, and the builder must cover monsters.** The spec
   says "`HERO_PARTS` holds six closed lists" and "`HERO_COLORS` lists ten colour
   fields". Since `5239839` ("Give the bestiary faces", 2026-09-12) there are
   **twelve parts** (`packages/heroes/src/spec.ts:91-104`: the original six plus
   `build`, `muzzle`, `eyeStyle`, `hide`, `wings`, `tail`) and **twelve colour
   fields** (`spec.ts:128-141`, plus `hideColor` and `wingColor`), two flags
   (`spec.ts:144`), and `MONSTER_TONES` beside `SKIN_TONES` (`spec.ts:162-175`).
   This is US2 proven by history rather than by argument — the monsters landed
   and no builder existed to be told about them — and it is why FR-001 is
   written as it is. It also means the phase (a) e2e must not hard-code counts.

2. **`size` is a spec field that FR-001 does not name.** The same commit made
   size first-class: `ResolvedHero.size`, `SIZE_CATEGORIES`
   (`spec.ts:114-123`), and `Hero.footprint` (`render.ts:45`). It is in neither
   `HERO_PARTS` nor `HERO_COLORS` nor `HERO_FLAGS`, so a builder built strictly
   to FR-001's list would have no size control for a gargantuan dragon. This
   plan gives `size` and `title` their own controls beside name, and records the
   gap as a question for the owner (below).

3. **A player cannot change their own character's art today — confirmed, and
   the reason is exactly as the spec read it.** `effective_actor_permission`
   (`src/server/src/auth/permissioned_entities.rs:97-137`) resolves Owner for a
   Game Master, then the caller's `world_actor_permissions` row, then **Viewer**
   (`:132-136`, test `member_with_no_explicit_row_defaults_to_viewer` at
   `src/server/src/auth/actor_permissions.rs:84-101`). It consults neither
   `world_actor_claims` nor `world_actors.owned_by` — `owned_by` is now only NPC
   visibility and ownership listings (`auth/actor_permissions.rs:3-8` says so
   itself). Claiming (`mutations_actor_claims.rs:351`) and creating your own
   (`:397`) write no permission row. `uploadActorImage` requires Editor
   (`mutations_actor_images.rs:113-121`). So phase (c) is the finding the spec
   said it was, and it is the largest piece of server work in this plan.

4. **A spec stored on an image does *not* travel for free.** Research R4 says a
   column on `world_actor_images` "is carried by the copy the moment the copy
   carries the row". It is not: `collections/copy.rs:597-616` selects
   `(role, asset_id)` by name and inserts those three columns, so a new column is
   silently dropped. `users/export_content.rs:304-314` does the same for
   `ExportedImage { role, asset_id }`. Both need an explicit task in phase (d)
   (FR-039, FR-040) — this is the same trap that dropped `is_unique` from three
   copy paths after spec 046 Phase 5 (046 `tasks.md`, Notes). The share path is
   different again: `mutations_actor_shares.rs:390-420` copies an actor **without
   any imagery rows at all**, so nothing travels there and nothing needs to.

5. **The imagery panel is now mounted twice, and the compendium row uploads
   too** (`04b1cf9`, `88d7a55`, `f15a9d6`, 2026-09-15/16).
   - `NpcEditorPage.tsx:224` (edit mode, after the NPC is saved) and
     `ActorDetailPage.tsx:489-494` (edit mode, `canEdit =
     myPermissionLevel !== "VIEWER"`, `:144`). A comment at `:~496-501` holds
     the builder's place. Phase (b)'s control in the panel therefore reaches
     both pages at once, and phase (c)'s "mount the panel" task is gone.
   - **But phase (c) is not free on that page.** Edit mode redirects a Viewer
     to `/view` (`:140-142`), and a claim holder resolves to Viewer (item 3).
     So the holder never sees the panel. Phase (c) mounts it in view mode as
     well, gated by a server-computed `myMayChangeImagery` (contract §4).
   - `NpcCompendiumTab` now has a per-row portrait upload
     (`handlePortrait`, `:177-200`; input at `:296-352`), and the old Add-NPC
     link at `:261-270` is a "New NPC" button at `:397-407`. FR-028a's row
     control sits beside the row's upload; Quick NPC sits beside "New NPC".
   - `ActorImageryPanel`'s `canEdit` gate is at `:182`, not `:175`.

6. **An image upload is refused while the world is paused.**
   `refuse_content_if_paused` (`mutations_actor_images.rs:122-124`) runs after
   the permission check. The spec now names the edge case; the save helper
   reports it per role and keeps the dialog open with the hero intact.

### The race the dice read (FR-007a)

- The dnd5e sheet stores race as free text, `trait_data.race`
  (`packs/systems/dnd5e/system.json:855-858`); Genie has no race field.
- `apps/web` already reads sheet fields without naming a system: size is
  located by the manifest's `combat.sizes.source` (`utils/sizeCategory.ts:30`).
  Race follows the same pattern: a manifest `appearance.race.source`, read by
  a small `utils/raceOnSheet.ts`, matched by `matchRace` in `packages/heroes`
  (research R6, R7).
- The manifest is a typed schema, not loose JSON: `SystemManifest`
  (`crates/pack_system_spec/src/lib.rs:53-110`) carries each optional block, and
  `combat.rs:215-220` refuses a `sizes.source` whose slot and field the system's
  data types do not declare. `appearance` joins it the same way — an optional
  `SystemAppearance { race: Option<{ source: { slot, field } }> }`, checked by
  the same `require_field`, with the published JSON Schema regenerated. Absent
  stays a correct answer, as it is for `vision`.
- The race picker and dice live in the library; the race looks and aliases live
  in `packages/heroes`. No drawing is added: every race in R6's first table is
  built from parts that already exist.

Two smaller confirmations, since the plan rests on them: `storage/svg.rs`
rasterises at 1024 px on the longest edge (`:25`, `:54-56`), resolves no
external reference (`:44-50`), draws no text (no `text` feature,
`src/server/Cargo.toml:152`) and ignores the declared size (test at `:97-105`);
and spec 046's `tokens.linked` / `tokens.system_data` (`schema.rs:818-819`),
`world_actors.is_unique` and `world_actors.visible_to_players`
(`schema.rs:1086-1087`) are all present, which is why this plan has a paragraph
about copies below.

### What spec 046 and NPC visibility change here

- **A token is its actor, or a copy of it** (ADR-102). Art is not hit points: a
  copy has no imagery of its own, and `token_art.rs:41-58` resolves a token's
  art from its actor at read time. So a built token reaches two hundred goblins
  by being saved once on the NPC they copy, and Quick NPC needs no per-token
  work. Phase (b)'s e2e places one built NPC twice and asserts both copies wear
  the face.
- **A hidden NPC's portrait is as hidden as its name**
  (`assets_serve/actor.rs:155-158`), while its **token art follows the map**
  (`:161-165`, `token_art_visible_sync` at `:86-114`) because a player who can
  see the scene can see the figure standing on it. Built art inherits both rules
  unchanged; phase (b) adds an e2e check that it does, since a builder is the
  first easy way to put art on a hidden NPC.
- **Unique NPCs place linked** (`mutations_token_links.rs:62`). Quick NPC makes
  an ordinary NPC, so its tokens are copies, which is what "six bandits" wants.

## Constitution Check

*Gate before Phase 0; re-checked after the Phase 1 design below.*

| Principle | How this plan satisfies it |
|---|---|
| **I. ECS owns simulation, React owns chrome** | The builder never speaks to the engine (FR-029). The race-aware roll is deterministic presentation data in `packages/heroes`, not a game rule, and it never writes the sheet. It draws SVG in React and hands bytes to an existing mutation; a built token reaches the map the way every uploaded token has since spec 031, through `token_art.rs`. No canvas state moves, and no rule is computed in React — there is no rule here, only a picture. |
| **II. Plugin-modular engine** | No engine change in any phase. `make lint-wasm` is named in the gates only because the pre-push hook runs it, not because this feature touches wasm. |
| **III. Ownership at the data boundary** | Phases (a) and (b) add no authority: `uploadActorImage` keeps its Editor gate and its pause gate (`mutations_actor_images.rs:113-124`), and the panel's `canEdit` stays presentation. Phase (c) widens the gate **on the server**, in one function that reads the claim, the world setting and the per-character lock, and every e2e proves the refusal by calling the mutation directly (SC-010). Phase (d) treats a stored spec as untrusted input, shape-checked and size-capped server-side before it is written (FR-037). |
| **IV. ADRs before divergent implementation** | Two, each landing with its phase: **ADR-105, "A character's look belongs to whoever holds it"** (phase c) — the first right derived from a claim rather than a `world_actor_permissions` row, and the two ways a Game Master withdraws it; and **ADR-106, "A stored image remembers the spec that drew it"** (phase d), recorded as an extension of ADR-057 and carrying R4's rejected alternatives and the append-only consequence (FR-038). Phases (a) and (b) need none: they add a tool over paths that are already decided. |
| **V. Verify before claiming done** | Per phase: `pnpm verify` (which already runs `packages/heroes`' own `check`), `pnpm -F @thunderforge/web exec tsc --noEmit` for every web phase because verify does not type-check the app, `cargo test` for (c) and (d), the phase's e2e through the harness with the log searched for `✘`, and `make lint-wasm` before the final commit because the hook demands it. The playtest is **not** a gate here: no playtest scenario builds a hero, and inventing one to satisfy a habit would prove nothing. |

**Gate result (pre-research)**: PASS.

**Re-check after design**: PASS, with one asymmetry recorded rather than
waived. Phase (c) makes a claim confer a right, which the one permission
declaration of ADR-050 — world role, then an explicit row, then Viewer — does
not do for any noun. That is a divergence, which is why it gets ADR-105
rather than a comment: the alternative — writing an Editor grant row on every
claim — would hand a player the whole actor (its sheet, its abilities, its
inventory) to give them a hat, and would leave a stale grant behind when the
claim ended. The narrower right is the safer one, and FR-030's "the grant MUST
cover imagery only" says so.

## Project Structure

### Documentation (this feature)

```text
specs/044-hero-builder/
├── spec.md              # the specification, decided 2026-09-14, clarified 2026-09-16
├── research.md          # Phase 0: R1–R7 (R6 race looks, R7 where race is read)
├── plan.md              # this file
├── data-model.md        # Phase 1: the three columns, and who carries them
├── contracts/
│   └── builder.md       # Phase 1: the package's new exports, the library's seam,
│                        # the GraphQL surface, and rules B1–B9 and B5a
├── quickstart.md        # Phase 1: how each phase is proved
├── checklists/
│   └── requirements.md  # the spec's own quality gate
└── tasks.md             # Phase 2 (/speckit-tasks)
```

### Source Code (repository root)

```text
packages/heroes/src/                # phase (a): six pure additions, phase (d): one
├── spec.ts                         # unchanged lists; labels and palettes import from here
├── labels.ts                       # new (a): HERO_LABELS, one per field and per choice
├── palettes.ts                     # new (a): HERO_PALETTES, a swatch list per colour field
├── races.ts                        # new (a): HERO_RACES, matchRace — race looks and aliases
├── random.ts                       # new (a): randomHero(seed, { locked, race }), over `seeded`
├── minimal.ts                      # new (a): minimalSpec(spec) — drop what does not change the draw
├── presetSource.ts                 # new (a): presetSource(slug, spec) — SKIN_TONES names where they match
├── schema.ts                       # new (d): HERO_SPEC_SCHEMA, derived from the lists
└── *.test.ts                       # a node test per addition; labels are complete or the check fails

packages/hero-builder/              # new workspace package (a)
├── package.json                    # @thunderforge/hero-builder; React 19 as a peer dependency
├── src/HeroBuilder.tsx             # the builder; state is a HeroSpec, drawn through validateHero
├── src/controls/                   # one control kind per field kind, rendered from the catalogue
├── src/roll/                       # race picker and dice (FR-007a), from HERO_RACES
├── src/preview/HeroPreview.tsx     # portrait and token, each with a unique id prefix (FR-011)
├── src/io/                         # import, export, copy-as-preset
└── src/index.ts                    # the seam in contracts/builder.md §2

apps/hero-builder/                  # new workspace app (a)
├── package.json                    # dev, build, preview, test:e2e
├── vite.config.mts                 # the web app's toolchain, its own port
├── playwright.config.ts            # webServer: the app's own preview; no stack
└── e2e/builder.spec.ts             # SC-002 to SC-006, from the catalogue at run time

apps/web/src/
├── pages/world/actor/ActorImageryPanel.tsx     # (b): "Build look", both roles, per-role result
├── pages/world/actor/HeroBuilderDialog.tsx     # (b): full-screen dialog, dynamic import(), focus trap, race
├── pages/world/actor/saveBuiltHero.ts          # (b): the one save path — two uploads, per-role, paused
├── pages/world/actor/heroFiles.ts              # (b): SVG string + role → File
├── pages/world/actor/ActorDetailPage.tsx       # (b): drop the reserved comment; (c): panel in view mode, the GM's lock
├── pages/world/compendium/NpcCompendiumTab.tsx # (b): "Build look" per row (FR-028a); Quick NPC beside "New NPC"
├── utils/raceOnSheet.ts                        # (b): the manifest-declared race field, system-agnostic
├── pages/world/compendium/QuickNpcDialog.tsx   # (b): name, previews, reroll, room for a template
├── pages/world/ActorSelectionPage.tsx          # (c): build while creating your own character
├── pages/world/settings/…                      # (c): "Players may change their character's art"
├── api/actors.ts                               # (b) heroSpec on upload (d); (c) the two new mutations
└── vite.config.mts                             # (a/b): the workspace deps, no second React

crates/pack_system_spec/src/lib.rs              # (b): optional `appearance` block on SystemManifest
crates/pack_system_spec/src/appearance.rs       # (b): SystemAppearance + require_field check, with tests
packs/systems/dnd5e/system.json                 # (b): "appearance.race.source" → traitData.race

src/server/src/
├── auth/actor_imagery.rs           # new (c): may_change_actor_imagery — claim, setting, lock
├── graphql/mutations_actor_images.rs # (c) the new gate; (d) heroSpec in, spec out
├── graphql/mutations_worlds.rs     # (c): updateWorldAllowPlayerActorArt
├── graphql/mutations_actors.rs     # (c): setActorArtLocked
├── graphql/types_actors.rs         # (d): heroSpec on GraphQLActorImage
├── heroes/spec_schema.rs           # new (d): the shape check and the size cap (FR-037)
├── collections/copy.rs             # (d): carry hero_spec (it does not travel by itself)
├── users/export_content.rs         # (d): carry hero_spec (FR-040)
└── migrations/                     # one directory for (c), one for (d)

docs/adrs/                          # ADR-105 (c), ADR-106 (d)
```

**Structure Decision**: the repository's existing layout, plus the two new
workspace members research R1 decided. Two points the structure makes that the
task list depends on:

- **The control lives in the panel, not in a page, and the save lives in one
  helper.** `ActorImageryPanel` already owns both roles and the per-role status
  line, and is mounted on both actor pages. The compendium row and Quick NPC are
  the other two hosts, so the two-upload, per-role, paused-aware save is
  `saveBuiltHero`, used by all three — FR-025 is written once.
- **Phase (a)'s e2e does not go through `scripts/e2e-parallel.mjs`.** That
  harness knows two suites, both rooted at `apps/web`
  (`scripts/e2e-parallel.mjs:185-193`), and every lane stands up a database, a
  server and a frontend. The standalone builder needs none of that, which is the
  point of it. Phase (a) therefore runs its own Playwright config directly
  (`pnpm -F @thunderforge/hero-builder-app test:e2e`), and phases (b) to (d) use
  the harness exactly as every other web change does.

## Complexity Tracking

No constitution violations to justify. Four risks, named rather than tracked:

- **A second React.** The library is ordinary React on the host's React, and
  `apps/web` already carries a force-resolution block written because
  `@thunderforge/genie` brought its own (`vite.config.mts:41-47`). The library
  declares React as a peer dependency and lists it nowhere else; the phase (b)
  e2e opening the dialog is the proof, because an "Invalid hook call" fails
  loudly at the first render.
- **The catalogue becomes a compatibility surface at phase (d), not before.**
  Until specs are stored, a choice may be renamed freely; after, every stored
  spec naming it stops validating (FR-038). The append-only rule therefore
  arrives with phase (d) and is enforced by a package test over a recorded list,
  not by a comment.
- **A new column on `world_actor_images` is dropped by two carriers unless
  threaded by hand** (finding 4 above). Phase (d) treats those as first-class
  tasks with their own tests, not as follow-through.
- **Phase (c) touches the permission ladder.** The new function is additive —
  it can only widen, never narrow, and never for anything but imagery — and its
  refusals are proven by direct mutation calls rather than by a hidden button
  (SC-010).

## Left for the owner

Two questions the code raised that the spec's Decisions do not answer. Neither
blocks phase (a); both want an answer before the phase that needs them.

0. **Settled 2026-09-16** (spec Clarifications): look only, full-screen
   dialog, "Build look" on each NPC row, and a race picker pre-selected from the
   sheet. A full character builder is a later spec that embeds this one.

1. **Does the builder offer `size`?** It is a spec field with a footprint on the
   board (`SIZE_CATEGORIES`, `render.ts:45`) and FR-001 does not list it. This
   plan gives it a control, because a builder that can draw a dragon but not say
   it is huge writes a spec whose token is the wrong size on the map. If size
   should instead be the actor's business and not the hero's, the control comes
   out and phase (d)'s stored spec keeps carrying the default. A race that
   narrows size (halfling, gnome, goblin in R6) depends on this answer too.
2. **What happens to a stored spec when a choice is retired anyway?** FR-038
   makes choices append-only, and US6 scenario 4 says a spec that no longer
   validates shows its problem and leaves the image alone. That is the right
   behaviour for an accident. Whether a deliberate retirement should get a
   migration that rewrites stored specs is not decided, and this plan does not
   build one.
