# Phase 0 Research: Interfaces Shaped By Their System

Nine decisions. §1–§7 survive from the first pass, three of them amended;
§8–§11 are new and come out of the clarification session. Every finding was
checked against the code on 2026-09-02.

---

## 1. A pack is data, not a module *(unchanged, and now doing more work)*

**Decision**: An interface pack is a manifest of values — colour tokens, an
engine appearance override, and a layout declaration. It contributes no
JavaScript, no stylesheet, no module, and nothing that executes.

**Rationale**: unchanged from the first pass, but the stake has risen. When the
pack was a palette, "no code" was easy because there was nothing to compute.
Now the pack describes a character sheet, and the temptation to add one
conditional is real and will arrive attached to a genuine problem. The answer
is the same and has to hold harder: the format has nowhere to put code, and
`deny_unknown_fields` means a key that is not in the contract is a rejection
rather than an ignored value.

**The line, stated so it can be applied under pressure** (FR-003a): declaring
*where a value appears* is presentation. Declaring *what a value is* is
behaviour. "Show `strengthMod` next to `strength`" is layout. "Show
`(strength - 10) / 2`" is a computation, and belongs to the system.

---

## 2. The theme vocabulary already exists, and there is only one of it *(unchanged)*

The custom properties in `apps/web/src/styles/globals.css` under `:root` and
`.dark` are the whole runtime-swappable vocabulary.
`apps/web/src/styles/tokens.scss` looked like a second, build-time-only token
system that would have made half the app unthemeable — it is imported by
**zero** files. Applying a pack is writing custom properties onto
`document.documentElement`.

---

## 3. Bundled packs only, read from disk *(unchanged)*

No `interface_packs` table and no upload flow. Discovery is a directory
listing, which also gives FR-007 for free: Forge is present because it is in
the directory, on the same footing as anything else there.

---

## 4. The engine gets the palette through a command that already exists *(unchanged)*

`set_display_appearance` is implemented in `src/engine/src/lib.rs`, owned by
`StatusDisplayPlugin`, typed in `apps/web/src/engine/sdk/commands.ts`, and has
**no caller**. This feature is its first. No engine change for the palette.

---

## 5. Light/dark stays with the reader; the pack stays with the world *(unchanged)*

A pack declares both palettes. The Game Master picks the pack; each participant
keeps their own brightness. This is the accessibility escape hatch that
survived making the look table-wide, and FR-012a's validation floor is the
other half of it.

---

## 6. The legibility floor is WCAG contrast, computed once, at validation *(unchanged)*

Rejection rather than a warning, because FR-009 leaves a reader no setting to
escape to. Computed in the validator crate so there is one implementation.
Stated explicitly against `thunderforge_canvas_core::resource_display::luma`,
which is Rec. 709 and a near neighbour that must not be confused with WCAG
relative luminance.

---

## 7. Propagation is a world event, not a poll *(unchanged)*

`EVENT_CODE_WORLD_APPEARANCE_CHANGED`, the next free code, recorded by the
mutation and re-resolved by every client on receipt. The spec 028 catch-up then
covers a client that was offline for it, at no extra cost.

---

## 8. The contract lives in canvas-core, because that is the dependency both sides already have

**Decision**: the single system contract (FR-027) is stated in
`crates/thunderforge-canvas-core`, alongside `AttributeDeclaration`,
`ResourceDefinition` and `MovementDeclaration`.

**Findings that decided it**:

- `src/engine/Cargo.toml` and `src/server/Cargo.toml` **both already depend on
  `thunderforge_canvas_core`**. It is the only crate both sides have.
- The contract is currently declared **twice**. `src/engine/src/systems/core.rs`
  declares `trait GameSystem`; `packs/systems/dnd5e/engine/src/plugin.rs`
  re-declares `trait GameSystemTrait` with the comment *"should match the one in
  `src/engine/src/systems/core.rs` / Re-defined here to avoid cross-package
  dependency"* — and has drifted from it (`&'static str` versus `String`, a
  `version()` the other lacks).
- `src/server/src/attributes.rs` records that the engine's version "has a single
  stub implementation, nothing depends on it".

The duplication has a stated cause — avoiding a cross-package dependency — and
canvas-core is exactly the dependency that removes the reason. It is also where
this codebase has twice put rules of this kind, each time giving the same
reason: its tests execute natively, and the engine crate's do not.

**Alternatives considered**:
- *A new crate for the contract alone.* Rejected: it would be a crate holding
  one trait and the declaration types it references, which already live in
  canvas-core. Two crates that must be versioned together are one crate.
- *Leave it in the engine and have packs depend on the engine.* Rejected: the
  server would then depend on the engine to read a character's values, and the
  engine cannot build for the host.

---

## 9. The contract carries declared values, never a fixed struct

**Decision**: the contract returns `identifier → value` pairs. It names no
system's concepts in its own vocabulary.

**Finding that decided it**: the engine's existing trait returns
`DerivedStats { effective_health, armor_class, initiative, proficiency_bonus }`.
That is one ruleset's character sheet compiled into a contract. It has nowhere
to put Blades in the Dark's stress, trauma and coin; nothing to say to Fate
Core, which declares **zero** abilities and eighteen skills; and no room for
Genie's Wish Points.

This codebase has made and corrected this mistake twice already. The engine
carried `TokenAbilities { strength, dexterity, constitution, intelligence,
wisdom, charisma }` and, in `attributes.rs`'s own words, "what it stored for a
Genie character was six `None`s". Resources went the same way and arrived at
`ResourceDefinition`. A third fixed struct would be a regression with two
precedents against it.

---

## 10. Derived values are computed by the pack, server-side, and never stored

**Decision**: a system pack implements the contract in its own crate; the
server resolves stored and derived values together and returns one set.
Derived values are never written to the database.

**Findings**:

- `packs/systems/*/server` are already Cargo workspace members, compiled into
  the product. Bundled packs therefore need no runtime code loading, which is
  what ADR-029 governs and has not answered. This is the distinction the whole
  increment rests on.
- The 5e pack's own `lib.rs` says its models hold "only BASE stats, never
  derived data", and that SRD reference data is "used for derived data
  calculations **on engine/web**". That intent produced
  `packs/systems/dnd5e/web/src/derived-data.ts` — 215 lines computing
  modifiers, saves and HP by hit die — which **nothing builds or loads**: the
  package has no `dist`, and `vite.config.mts` aliases only Genie's web source
  into the bundle. Meanwhile `apps/web/src/components/game-systems/dnd5e/
  CharacterSheet.tsx` computes a dexterity modifier inline at line 208.
- The server already resolves declarations server-side —
  `src/server/src/attributes.rs` and `src/server/src/status_display.rs` do
  exactly this for attributes, movement and resources.

So the stated intent has been tried and produced two implementations, one dead.
Resolving server-side puts derivation on the path that already exists, makes it
natively testable, and gives the sheet and the canvas status bars the same
numbers rather than each deriving its own.

**Not stored**, because a derived value that is also stored is two values that
can disagree, and the stored one is the one that will be stale.

---

## 11. Layout addresses declarations generically or by name

**Decision**: a layout construct targets either a declaration *set* — "every
declared attribute, in declaration order" — or a specific identifier. Forge
uses only the generic form (FR-025b); targeted packs use names and validate
against each named system's manifest (FR-026).

**Why both are needed** — the shipping manifests differ in kind, not degree:

| System | abilities | skills | resources | movement |
|---|---|---|---|---|
| dnd5e | 6 | 18 | hitPoints | 5 |
| pathfinder2e | 6 | 18 | hitPoints, focusPoints, heroPoints | 5 |
| genie | 3 | 0 | health, wishPoints | stride |
| blades_in_the_dark | 3 | 12 | stress, trauma, coin | none |
| year_zero_engine | 4 | 12 | — | — |
| cypher_system | 3 | 0 | — | — |
| fate_core | **0** | 18 | — | — |

A layout that names identifiers cannot serve all of these; a layout that only
addresses sets cannot express a nine-level spell slot grid or a six-box death
save tracker. Generic addressing is what lets Forge be the system-agnostic
default of FR-006 as a mechanism rather than a promise — it works everywhere
precisely because it names nothing.

**Scale, from the source**: the published 5e sheet carries 336 fields — 122
checkboxes, 100 spell name slots, 18 slot counters, 9 attack fields, 5 currency
denominations, 6 abilities and 6 separate modifier fields. That is the ceiling
a targeted pack has to reach eventually. It is **not** a design to copy: FR-003b
makes published sheets a source of scope and never of layout, ornament, or
wording.

---

## 12. Discovery, not a list — mechanism undecided, and flagged as such

**Decision for now**: replace the named registration functions with discovery
over the workspace's pack crates, and add `scripts/check-system-registry.mjs`
to fail the build if a central list reappears. **This is the decision most
likely to need revisiting**, and it is recorded that way rather than as settled.

**The state today**: `src/server/src/systems.rs` has `register_dnd5e_system`
and `register_genie_system`, each naming its system and wiring five validator
functions by hand, with a comment saying the remaining five systems "all follow
`register_genie_system`'s exact pattern". `src/server/Cargo.toml` has a
`[dependencies.dnd5e_server]` and `[dependencies.genie_server]` block. Adding a
system means editing both — SC-004's violation, twice.

**The precedent is weaker than the spec assumes.** The spec's Assumptions name
the interaction seam as the pattern to follow, but `src/server/src/interaction.rs`
still assembles from a `contributions()` function. What
`scripts/check-interaction-seam.mjs` actually enforces is that the *core* owns
no effect — a different and more modest claim than "no central list".

**Alternatives**:
- *A distributed-slice crate (`inventory`, `linkme`).* True discovery, no list
  at all. Costs a dependency and behaves differently across linkers; wasm is
  the case to check before committing.
- *A generated list, plus the checker.* Honest, boring, and the checker is what
  actually holds the property. Weaker than FR-029's wording.
- *Keep the hand-written list.* Rejected: it is the thing SC-004 measures.

---

# Increment F (User Story 2) — 2026-09-03

Three questions, researched against the code rather than reasoned about.

## F-1. Where does the list of installed systems come from?

**Decision**: the `packs/systems/` directory, served by `/api/systems`, exactly
as `packs/interface/` is already served. `BUNDLED_SYSTEM_IDS` and
`BUNDLED_SYSTEM_LABELS` are deleted from `apps/web/src/api/gameSystems.ts`.

**Findings**:

- `game_systems` is a real table and it holds **0 rows** (measured 2026-09-03
  against the development database). Nothing has ever seeded it with the
  bundled packs.
- `/api/systems` reads that table, so it honestly answers an empty list, and
  `gameSystems.ts` compensates with two hand-kept literals naming all seven
  systems and their titles. Its own comment admits this: *"there is no
  reconciled installed system packs catalog yet ... extend it (or replace it
  with a real catalog query) as more packs are added."*
- The interface-pack half already solved this. `interface_packs::list_installed`
  reads the directory, and the client's comment records the consequence —
  *"unlike `BUNDLED_SYSTEM_IDS` ... there is no hand-kept list here"*. The
  asymmetry is the bug.
- Each `system.json` already carries `id` and `title`, which is everything both
  lists hold.

**Alternatives considered**: seeding `game_systems` at boot from the directory.
Rejected — it makes the database a cache of the filesystem, with the filesystem
still authoritative, and adds a staleness mode for no gain. A row per installed
system earns its place only when a system can be installed at runtime, which
ADR-029 says it cannot.

**Raises**: ADR-028 (*Game Systems DB Model and Ownership Rules*) is an empty
stub, and this is its question. The likely answer to record: the table is
premature, the directory is the row of record, and the table is either dropped
or reserved for the runtime-installed case that does not exist yet.

## F-2. How does a pack own the table it writes to?

**Decision**: NEEDS RESOLUTION IN PHASE 0. This is the increment's hard part
and is deliberately not being settled from the armchair.

**Findings that constrain it**:

- `genie-server`'s `Cargo.toml` lists `serde`, `serde_json`, `tokio`,
  `async-trait`, `inventory` and `thunderforge_canvas_core`. **No `diesel`.**
- `world_genie_sessions`, `world_genie_puzzle_clocks` and
  `world_genie_resource_holdings` are declared in `src/server/src/schema.rs`,
  which is generated by `diesel print-schema` from the live database
  (`src/server/diesel.toml`), not hand-written.
- `[migrations_directory] dir = "migrations"` — diesel CLI reads exactly one
  directory, `src/server/migrations`. A pack-owned `migrations/` is not
  something the tooling looks for today.
- The inventory pattern is proven for the *declaration* half:
  `SystemContribution` is submitted by each pack crate and collected through
  the linker, with `check-system-registry.mjs` enforcing that shared code names
  no system.

**The three candidate shapes**, to be decided with evidence in Phase 0:

1. **Pack declares its own tables, server keeps generating them too.** Cheapest;
   accepts two `table!` declarations for one table. That is precisely the drift
   this spec has spent five increments removing, so it starts at a disadvantage.
2. **Pack owns its migrations and its schema; `print_schema` excludes pack
   tables.** Diesel supports a `filter` in `print_schema` (`only_tables` /
   `except_tables`). Single declaration, single owner — at the cost of a
   convention nobody can forget, which is what a check script is for.
3. **Packs do not touch tables; the hook returns data the server persists.**
   Keeps `diesel` out of pack crates entirely. Whether it can express the genie
   session row — with its own columns and its own lifetime — is the open
   question, and the honest test is to try writing it.

**Alternatives rejected outright**: moving the branch into
`system_packs.rs`, which the registry checker exempts. That relocates a
violation into the one file exempted from noticing it, which is a dodge rather
than a fix.

## F-3. What contains a failing pack surface?

**Decision**: a React error boundary per mounted surface, told which pack it
wraps, rendering a named message in place of the surface.

**Findings**:

- Nothing contains one today. `PackActorSheet` handles a *fetch* rejection and
  says the sheet could not be loaded; a component that throws while rendering
  takes the page, and the message names nothing.
- `apps/web` has **no error boundary at all**. Searching `src/` for
  `componentDidCatch`, `ErrorBoundary` and `getDerivedStateFromError` returns
  nothing (verified 2026-09-03), so this is new machinery rather than a
  boundary to reuse.
- SC-009 measures two things separately: 100% of the surrounding session stays
  usable, and the message names the responsible pack in 100% of cases. Both are
  observable from a test that injects a throwing surface.
- `MissingPackNotice` is the tonal precedent — name the pack, block nothing.

**Alternatives considered**: a global boundary at the app root. Rejected — it
satisfies "does not crash" and fails "the rest of the session remains usable",
because the whole page is the thing replaced.


## F-5. How does a pack contribute GraphQL mutations?

Researched 2026-09-03, after ADR-063 named this the unanswered question
blocking the move of Genie's session domain into its pack. **It is answered,
and the answer is much cheaper than ADR-063 assumed.**

**Decision**: invert the server↔pack dependency. The server becomes a
**library** crate, packs depend on it, and a thin **binary** crate depends on
both — holding `system_packs.rs`'s linkage lines and composing the GraphQL
roots. A pack contributes mutations as an ordinary `MergedObject` member.

### What was measured, in order

**1. `async_graphql::dynamic` is available and is the wrong tool.**
`dynamic-schema` is a default feature of async-graphql 7.2.1 and is enabled
here. But the dynamic API has **no interop with the static one**: there is no
way to register a `#[derive(Object)]` type into a `dynamic::Schema`. Using it
means rewriting the entire schema — every type in the product — as runtime
values, losing compile-time type safety everywhere to make one pack's
mutations discoverable. Rejected.

**2. `MergedObject` naming a pack's type is not the violation it looks like.**
`QueryRoot` and `MutationRoot` are tuples naming every contributing type at
compile time, `GenieSessionQuery` and `GenieSessionMutation` among them. That
reads like the registry FR-029 forbids, and it is not: it carries **no
information that can drift**. Compare `system_packs.rs`'s `use genie_server as
_;`, exempt for exactly this reason — a tuple entry says a type exists and
should be merged, and says nothing about that system's data shapes, validators
or rules. If the pack changes its mutations, the entry does not; if the type
goes away, the build fails loudly rather than drifting quietly.

The distinction worth holding: `match game_system_id { "genie" => … }` is
shared code **deciding** something per system at runtime, which is the
violation. A merge tuple is shared code **composing** at build time, which is
the same category as a dependency.

**3. The real obstacle was never GraphQL. It was that `thunderforge` is a
binary-only crate.** There is no `[lib]` target — which is why
`cargo test -p thunderforge --lib` answers "no library targets found". A pack
crate cannot import from it at all, so the 2,763 lines of Genie GraphQL cannot
move: they need `AppState`, `is_dm_of_world`, `require_world_member`,
`record_world_event`, `app_state`, `authenticated_user`, and the shared
`models`/`schema`.

**4. Extracting those into a shared crate would be enormous — and is not
necessary.** Measured first, in case it was the only way: of 136 server source
files, **91 reference `crate::schema`, 68 `crate::state`, 65 `crate::models`,
50 `crate::auth::world_membership`**. Extraction would touch roughly 100 files
and move ~4,000 lines before one line of Genie moved.

**5. The server compiles as a library, and it takes two lines.** Verified by
doing it: a generated `lib.rs` declaring the same 41 modules compiled with
**zero errors** after adding `#![recursion_limit = "512"]` (the same attribute
`main.rs` already carries, for the same MergedObject nesting reason) and one
`pub use state::AppState;` (which `main.rs` provides at its crate root).
**655 of the 659 tests** come along to the lib target; the remainder live in
`main.rs` itself. The spike was reverted — a permanent lib target beside the
bin compiles everything twice, and that cost is only worth paying as part of
the restructure.

**6. The whole server→pack coupling is seven lines and seven Cargo blocks.**
`src/server/src/system_packs.rs` holds `use <pack> as _;` seven times, and
`src/server/Cargo.toml` seven `[dependencies.*_server]` blocks. Nothing else
in the server names a pack crate. That is the entire cycle, and it moves to
the binary.

### The shape

```text
crates/thunderforge-server/     the current src/server, as a library
packs/systems/*/server/         depend on it; own their tables and mutations
src/server/  (bin)              depends on both: system_packs.rs, the merged
                                GraphQL roots, main(), the CLI
```

`MergedObject` nests, so the binary composes `MutationRoot(CoreMutation,
GenieSessionMutation, …)` where `CoreMutation` is itself a merged root in the
library. The pack half of `QueryRoot`/`MutationRoot` lives with the linkage
lines, in the one place whose job is composition.

### What this costs, honestly

A workspace restructure — moving a crate, splitting a binary out of it, and
rewriting seven pack manifests — plus the domain move itself: six tables, six
migrations, `print_schema`'s `except_tables`, 2,763 lines of GraphQL, fourteen
models and one event code. It is an increment. But it is **standard Cargo
mechanics on a proven-compilable library**, not the open-ended extraction
ADR-063 sized it as, and the risky unknown is now closed.

**It also unblocks `systemActorSheets.ts`.** Once a pack can own a table and a
mutation, `GenieActorSheet`'s reason for living in shared web code — that it
edits `trait_data.level` and recomputes max Wish Points, which a declared-value
sheet cannot express — becomes a thing the pack can own too. All three
remaining hardcoded-system violations close on the same work.


## F-6. What the move actually cost (recorded 2026-09-03, after doing it)

F-5 predicted the shape and got it right. Two constraints it did not find,
because both are only visible from inside the work:

**`allow_tables_to_appear_in_same_query!` cannot span crates.** The macro
emits an impl of a Diesel trait for *each* ordering of every pair it is given.
For a pair spanning two crates, one of those impls has a foreign self type and
a foreign trait, and the orphan rule refuses it. There is no invocation order
that avoids this — it is not a matter of which crate declares it.

Genie needed no cross-crate join, which was established by removing them and
watching it compile rather than by reading the code. A pack that *does* need
one has two options — split it into two queries, or leave that query on the
server's side of the line — and neither is a disaster, but the constraint
should be known before designing around it.

**A dev-dependency cycle produces two compiled instances of the library.** The
packs link against the normal build; `cargo test` compiles a second copy under
`cfg(test)`. `inventory` collects into one registry per instance, so a pack's
submissions are invisible to the tests of the crate they submit *to*. This
surfaced as a discovery test failing for reasons that had nothing to do with
the product.

`SystemContribution` never had this problem because it collects in
`thunderforge-canvas-core` — a plain dependency, compiled once, shared by
both sides. `WorldCreatedHook` cannot live there: it takes a
`&mut PgConnection`, and canvas-core is compiled to wasm as part of the
engine.

So the rule is: **a registry collected in the crate under test can only be
asserted from a binary that links everything.** Selection logic is testable in
the library; discovery is not. That is why the hook's discovery test lives in
`src/app` beside `system_packs.rs`, which had already made the same argument
for the same reason.

**What the move did not cost**: any of the extraction F-5 sized as the
fallback. No module moved out of `src/server` except Genie's own, and the
~4,000-line, ~100-file rewrite never happened.
