# A Pack Owns the Tables It Writes

- **Date**: 2026-09-03
- **Status**: Accepted, and **implemented the same day** — see *Outcome*
- **Spec**: `specs/032-pack-architecture/` Increment F, F2 (FR-004, T014a2)
- **Related**: ADR-029 (a bundled pack may contribute behaviour), ADR-061
  (a pack's implementation is discovered, not listed), ADR-028 (the directory
  is the row of record)

## Context

`src/server/src/graphql.rs` branches on `game_system_id == "genie"` during
world creation and inserts a `world_genie_sessions` row. It is the only
remaining entry in `scripts/check-system-registry.mjs`'s `KNOWN` list, and
spec 032's Increment F set out to retire it with a world-creation hook that
any pack could contribute.

ADR-029 removed the gate: a bundled pack may contribute behaviour, and
`genie-server` is a Cargo workspace member compiled into the product. So this
looked like a one-line fix behind a small mechanism.

It is not, and this ADR records why, because the reason was found by measuring
rather than by reasoning and would otherwise be found again.

### The obstacle is table ownership

- `genie-server`'s `Cargo.toml` lists `serde`, `serde_json`, `tokio`,
  `async-trait`, `inventory` and `thunderforge_canvas_core`. **No `diesel`.**
- Genie's **six** tables — `world_genie_sessions`, `world_genie_puzzle_clocks`,
  `world_genie_puzzle_clock_rewards`, `world_genie_resource_holdings`,
  `world_genie_shop_listings`, `world_genie_trade_proposals` — are declared in
  `src/server/src/schema.rs`, which is *generated* by `diesel print-schema`
  from the live database, not hand-written.
- `src/server/diesel.toml` names one migrations directory. A pack-owned
  `migrations/` is not something the tooling looks for.

So a pack cannot write to its own table, because in no sense the code
recognises does it have one.

### What the spike found, and it was not what the plan expected

The plan (`plan.md` § F2) framed this as choosing between three shapes for a
hook. Measuring the code first changed the question.

**Genie's session domain already lives in shared server code, and it is not
small.** `src/server/src/graphql/mutations_genie_session.rs` is 2,385
production lines carrying thirteen GraphQL mutations — Session Wish Pool, Doom
Clock, Puzzle Clocks, Session Resource grants, shop listings, two-party
resource trades — and `src/server/src/graphql/queries/genie_session.rs` is a
further 378. Together, **2,763 lines of one ruleset's rules in the shared
server**, plus fourteen Genie model declarations in `models.rs` and an
`EVENT_CODE_GENIE_SESSION_STATE` in shared `world_events`.

`check-system-registry.mjs` passes over all of it honestly: the rule it
enforces is that shared code must not *name* a system, and these files quote
`"genie"` only inside `#[cfg(test)]` modules, which are correctly exempt. The
check is not broken. It measures what it says it measures, and this is simply
a larger thing standing beside it.

**That makes the world-creation insert twenty lines of a 2,763-line problem.**
Moving those twenty and declaring the system-agnostic server achieved would be
a truthful commit message and a misleading claim.

### The three candidate shapes, against that

1. **Pack declares its own table, the server keeps generating one too.**
   Cheapest, and it works today. It also accepts two `table!` declarations for
   one table, which is precisely the drift Increments A–E spent their length
   removing — two divergent `GameSystem` traits, seven re-declared
   `GameSystemTrait`s, two fixed `DerivedStats`. Buying the hook with the
   defect the feature exists to retire is a poor trade at any price.
2. **Pack owns its migrations and its schema; `print_schema` excludes pack
   tables.** Single declaration, single owner. Verified available:
   `diesel_cli` 2.3.12 supports `filter = { except_tables = [...] }` under
   `[print_schema]`. **But excluding the six genie tables from the generated
   schema breaks 2,763 lines of shared code on the next build** — that code
   imports seven table modules from `crate::schema`. This shape cannot be
   applied to one table for one hook; it forces the whole domain to move at
   once.
3. **Packs do not touch tables; the hook returns data the server persists.**
   The shape whose feasibility research called genuinely unknown. It is now
   answerable: it cannot express this row. The server would have to write to a
   table with bespoke columns it does not know, which means either building
   SQL from a pack-supplied table name — losing the type safety that is the
   reason this codebase uses Diesel, and adding an injection surface for a
   pack — or a generic key-value store, which is a different design for a
   different problem. And it addresses only the *insert*: every other query
   against those tables stays in the server regardless.

**Rejected outright**, and worth writing down because it is the tempting one:
moving the branch into `src/server/src/system_packs.rs`, which the registry
checker exempts. That relocates a violation into the one file exempted from
noticing it. The check would go green and nothing would be true that was not
true before.

## Decision

**A pack owns the tables it writes: its own `table!` declarations, its own
migrations, and `print_schema` configured to leave its tables out of the
server's generated schema.** That is shape 2, and it is the destination.

**It is not reached in Increment F.** Reaching it requires moving Genie's
session domain — six tables, 2,763 lines of GraphQL, fourteen models and one
event code — out of the server and into `packs/systems/genie/server/`. That
is an increment with its own research, not a task inside this one.

> **Amended the same day.** This ADR originally closed by naming an
> unanswered question — how a pack contributes GraphQL mutations, given
> `async-graphql` composes its schema from types named at compile time — and
> treating it as the increment's open risk. It has since been researched and
> answered; see `specs/032-pack-architecture/research.md` § F-5. Two things
> changed:
>
> 1. **A `MergedObject` entry naming a pack's type is not a registry.** It
>    carries no information that can drift — the same argument that exempts
>    `system_packs.rs`'s `use <pack> as _;` lines. Composing at build time is
>    a different act from deciding per system at runtime, which is the thing
>    FR-029 forbids.
> 2. **The real obstacle was never GraphQL — it is that `thunderforge` is a
>    binary-only crate**, so a pack cannot import from it at all. That looked
>    like it demanded extracting `state`, `models`, `schema` and
>    `auth::world_membership` into a shared crate: ~4,000 lines across ~100 of
>    136 files. It does not. **The server compiles as a library, and it takes
>    two lines** — verified by doing it, zero errors, 655 of 659 tests coming
>    along. The whole server→pack cycle is seven `use` lines in one file and
>    seven Cargo blocks, all of which move to a thin binary crate.
>
> So the move is a workspace restructure plus the domain migration — an
> increment, still, but standard Cargo mechanics against a proven-compilable
> library rather than an open-ended extraction. The sizing below stands; the
> *risk* does not.

So, concretely:

1. The `game_system_id == "genie"` branch in `graphql.rs` **stays**, and stays
   in `check-system-registry.mjs`'s `KNOWN` list with this ADR as its reason
   rather than the previous "gated on ADR-029", which is no longer true.
2. No hook is added to `SystemContribution` yet. Adding one whose only
   implementation cannot own its table would fix the check and not the code.
3. Spec 032's T014a2 stays open, re-scoped to what it actually needs.

## Outcome

**Done, 2026-09-03.** This ADR was written expecting the move to be a later
increment, and it was carried out within the day once research § F-5 found
that the obstacle was not what it looked like.

What shipped:

- `src/server` became the library `thunderforge-server`; `src/app` is a thin
  binary that links the packs, merges their GraphQL, and runs `main`. Two
  lines were needed to make the library compile.
- Genie's six tables, eleven models, thirteen mutations and its queries moved
  to `packs/systems/genie/server/src/session/`. `diesel.toml` excludes those
  tables from `print-schema`, so there is one declaration of each.
- The world-creation branch became `world_hooks.rs`: packs submit a
  `WorldCreatedHook`, the server runs whichever match, inside the creation
  transaction. It lives in the server rather than beside `SystemContribution`
  in canvas-core, because it takes a `&mut PgConnection` and canvas-core is
  compiled to wasm.
- **`check-system-registry.mjs` reports zero violations and nothing exempted.**

Two constraints the spike had not found, both worth carrying forward:

1. **`allow_tables_to_appear_in_same_query!` cannot span crates.** It emits an
   impl in each direction and the reverse lands on a foreign type, which the
   orphan rule refuses. Genie needed no cross-crate join; a pack that does
   will have to split the query or leave the join on the server's side.
2. **A dev-dependency cycle produces two compiled instances of the library**,
   and `inventory` collects into one. Anything collected in the crate under
   test is invisible to that crate's own tests, which is why hook discovery is
   asserted in the binary.

## Consequences

- ~~`check-system-registry.mjs` continues to report one outstanding
  violation.~~ **Superseded by the outcome above** — the list is empty.
- The claim Increment F can make is narrower and true: the shared *application*
  no longer knows which systems exist (ADR-028), a failing pack surface is
  contained and named, and the contract is published. A pack contributing
  *behaviour* is not delivered, and F's checkpoint is amended to say so.
- **The next person to look at this does not have to re-measure.** The three
  shapes are evaluated, shape 3 is closed rather than open, and the blocker is
  a line count rather than a judgement.

## What would change this

- **Genie's session domain moves into its pack.** Then shape 2 applies to six
  tables at once, `except_tables` costs nothing, and the hook is small — which
  is the order this work actually goes in.
- **A second pack needs a world-creation hook.** One pack wanting something is
  a case; two is a shape, and it would be worth paying shape 1's cost
  temporarily to learn what the hook's signature should be.
- ~~The GraphQL contribution question gets an answer.~~ **Answered
  2026-09-03** — see the amendment above and research § F-5. What remains is
  the work, not the doubt.
