# Research: Roll for Shoes

**Feature**: `specs/061-roll-for-shoes` | **Date**: 2026-09-22

The spec describes a game. This file works out which parts of it the existing
pack contract already answers, and which parts the pack has to answer itself.
Every question here was settled by reading the code, and each answer names
where.

The short version: **almost none of Roll for Shoes is declarable.** The
declarative surfaces of a pack — `sheet`, `checks`, `resources` — are built
around values that exist before play starts and are named in the manifest. A
Roll for Shoes skill is invented mid-sentence, at the table, by a player. So
the manifest carries the character's shape and the pack's own code carries the
game, which is exactly the division ADR-029 draws.

---

## D1. The sheet is contributed, not declared

**Decision**: ship `packs/systems/roll_for_shoes/web/src/ActorSheet.tsx`. The
manifest declares `sheet` only for the description.

**Rationale**: three separate walls, any one of which would be enough.

- A `sheet` entry's `kind` is one of `text`, `number`, `list`, `slots`,
  `track`, `state` (`packs/systems/README.md:178-186`), and `list` is "an
  ordered list of strings". A skill is `{ name, level, parent }`. There is no
  kind that holds it.
- Worse than unsupported, it is *invisible*: `declared_values_for_actor` walks
  a slot's top-level fields and converts an array by keeping only its string
  items — objects are skipped rather than flattened
  (`src/server/src/declared_values.rs:58-84`). An array of skill objects
  publishes as an empty list, so nothing declarative could read it even if a
  kind existed.
- The format is deliberately inert. "Every kind here is a **shape of value**,
  never a rule about one. There is no conditional, no expression and no
  formula" (`packs/systems/README.md:194-198`). Rolling a skill, awarding XP on
  a failure and granting a skill on all sixes are rules about values.

`resolveActorSheet(actor.gameSystemId)` is a build-time glob over
`packs/systems/*/web/src/ActorSheet.tsx`
(`apps/web/src/pages/world/actor/systemActorSheets.ts:52-72`), and where it
finds one it is what mounts — the actor page falls back to the declarative
`PackActorSheet` only when it finds none
(`apps/web/src/pages/world/actor/ActorDetailPage.tsx:569-577`). So a
contributed sheet needs no registration and no edit to shared code.

**Alternatives considered**: extending the declarative format with an
object-list kind and an expression language. Rejected — that is the boundary
ADR-029 draws, and crossing it to serve one pack would make every installed
manifest a program.

---

## D2. A `server/` crate is required, and the contract's "optional" is about a
## different kind of pack

**Decision**: ship `packs/systems/roll_for_shoes/server/`, whose whole job is
to register the system and validate its two slots.

**Rationale**: the pack contract calls `server/` optional
(`packs/systems/README.md:17-29`) and it is — for a pack that only declares.
The moment a pack *stores* anything, it is not. `update_actor_system_data`
validates unconditionally through `GameSystemRegistry::validate`, which
refuses a system it does not hold:

```rust
let system = self.systems.get(system_id)
    .ok_or_else(|| format!("System '{}' not registered", system_id))?;
```

(`src/server/src/systems.rs:650-654`, called from
`src/server/src/graphql/mutations_actor_system_data.rs:150-155`.) The registry
is populated only from `inventory`-submitted `SystemContribution`s
(`src/server/src/systems.rs:700-715`). Without a crate, every write of XP or a
skill fails with `Validation failed: System 'roll_for_shoes' not registered`.

Registration alone would do — a contribution may leave every validator `None`
and `validate` returns `Ok(())` for a slot with none
(`src/server/src/systems.rs:665-668`). We supply two anyway, because the skill
list is the one thing in this game that must not be allowed to go crooked.

**The lines outside the pack directory** are the four the contract names
(`packs/systems/README.md:445-457`), and SC-008 already allows them: a
`members` entry in the root `Cargo.toml`, a dependency in
`src/app/Cargo.toml`, one `use roll_for_shoes_server as _;` in
`src/app/src/system_packs.rs`, and nothing else. `check-system-registry.mjs`
fails the build if the string `roll_for_shoes` appears anywhere else in shared
server code.

**No `engine/` crate.** Roll for Shoes has no geometry: no movement, no
vision, no sizes, no combat. `engine/` stays absent, and absence is a fact
about the ruleset.

**No root GraphQL fields**, and therefore no `PackSurface` play/pause
classification to submit (`packs/systems/README.md:404-425`). The pack rides
`rollDice` and `updateActorSystemData`, both of which already refuse a paused
world.

---

## D3. Rolling goes through `rollDice`, not `checks`/`rollCheck`

**Decision**: the sheet calls `rollDice` directly with `(LEVEL)d6` and a
binding, and reads the dice back.

**Rationale**: `checks` is a static list. Each entry has an `id` fixed in the
manifest, the sheet "sends only this `id`"
(`packs/systems/README.md:323-332`), and the server resolves its bindings from
the actor's declared values
(`src/server/src/graphql/mutations_roll_check.rs:111-130`). A Roll for Shoes
skill has no manifest id — it did not exist when the manifest was written —
and its level lives inside an array of objects that `declared_values` drops.
Both halves of `rollCheck` are therefore unavailable, and no amount of
declaration fixes it.

`rollDice` needs none of that:

```
rollDice(input: { worldId, formula: "(LEVEL)d6", bindings: [{ name: "LEVEL", value: 3 }] })
```

- **A dice count may be an expression.** `DiceTerm.count` is itself an `Expr`
  (`crates/thunderforge-dice/src/ast.rs:42`), and `(rating)d6kh1` is a shipped
  test case (`crates/thunderforge-dice/src/eval.rs:860`). The placeholder must
  be parenthesised; `LEVELd6` would lex as one identifier.
- **A missing binding is a refusal, not a zero**
  (`crates/thunderforge-dice/src/eval.rs:131-137`).
- **Any world member may roll**, and a paused world refuses
  (`src/server/src/graphql/mutations_roll.rs:60-67`); `rollDice` is already
  classified gated
  (`src/server/src/graphql/play_pause_surface_tables.rs:277-278`).
- **The dice are the server's.** The RNG is constructed inside the resolver —
  "the one and only place in the whole system a 'real' roll is produced"
  (`src/server/src/graphql/mutations_roll.rs:137`) — and one row is written to
  `world_roll_records`. This is what FR-020 asks for, and the client cannot
  fake an outcome.

**Read `dice[].finalValue`, never `resultValue`, for the advancement check.**
`GraphQLDieOutcome.final_value` is the per-die face
(`src/server/src/graphql/types_dice.rs:17-24`); `resultValue` is the sum, and
a sum cannot tell you whether every die showed a six. FR-034's "must not be
affected by any modifier applied to a total" falls out of this for free.

**Alternatives considered**: declaring one `check` per level, up to some
ceiling. Rejected — FR-040 puts no cap on levels, so any ceiling is wrong, and
it would still not bind to a skill the manifest cannot name.

---

## D4. Where the character's four things live

**Decision**:

| The spec's word | Where it is stored |
|---|---|
| name | the actor's own `label` — not system data |
| description | `trait_data.description` (a string) |
| XP | `resource_data.xp` (an integer, ≥ 0) |
| skills | `trait_data.skills` (an array of objects) |

**Rationale**: XP is a counter and nothing else, so it is declared as one:
`kind: "counter"`, `source: { slot: "resourceData", entries: [{ current: "xp" }] }`
— the shape Blades' Coin and Fate's Fate Points already use. Being a top-level
scalar, it publishes through `declared_values` and appears wherever the product
draws a system's counters, including on a token. That is the whole benefit of
putting it in `resource_data` rather than beside the skills.

The skills go in `trait_data` as an array of objects. JSONB stores it
faithfully and `data_types.properties` is never type-checked against stored
data — `pack_system_spec` reads `properties` only to confirm that `combat` and
`appearance` name a declared key. So the shape is ours to enforce, in our own
validator, which is D2's second job.

Storing the description as a declared `sheet` entry of `kind: "text"` costs one
line and makes the one part of the character that *is* a plain value visible to
the declarative surfaces. The skills will never be, and that is accepted.

---

## D5. The opposition is a number the player records, not a message the product
## carries

**Decision**: the sheet takes the opposition as an optional number entered
beside the roll. FR-021's "supplied by the Game Master" is satisfied at the
table, and the number is shown with the result.

**Rationale**: a roll today is **private**. `rollDice` writes one row to
`world_roll_records` and emits no world event, no NOTIFY and no chat message;
the only read-back, `worldRollRecords`, is annotated GM-only
(`apps/web/src/api/roll.ts:55`). Carrying a contest between two seats would
mean a pack-owned table, new root GraphQL fields and a `PackSurface`
classification — a materially larger feature than the six rules this spec
scopes, and one the game does not need: the Game Master says a number out loud,
or rolls their own dice and says the total.

FR-023's unjudged roll is the same control left empty, which is why the spec
insists an unjudged roll is *not* a success: there is nothing to have beaten.

**Alternatives considered**: broadcasting rolls to the table. Not rejected on
merit — it is genuinely wanted, for every system, and is written into Out of
Scope as later work rather than smuggled in here for one pack.

---

## D6. The manifest cannot reach the sheet, so the starting skill is stated
## twice

**Decision**: `Do Anything 1` is declared in `system.json` for the record and
for the server validator, and is also a constant in the pack's web module. The
duplication is accepted and commented at both sites.

**Rationale**: the host surface a pack may import
(`apps/web/src/host/index.ts`) exposes actor data, roll and world helpers and
the presentational primitives — and no way to read a manifest. A pack imports
from there or from nowhere. Since both copies live inside the pack's own
directory, they cannot drift apart without the pack's own tests noticing, and
neither one leaks the system's name into shared code.

This is worth writing down because the house preference is the opposite: the
manifest is normally the authority, as Genie's level ladder is. Here it cannot
be, and the reason is the host surface rather than a judgement about where
rules belong.

---

## D7. A level above 1000 is a refused roll, not a clamped one

**Decision**: render the server's refusal. Do not cap the level.

**Rationale**: FR-040 says levels have no upper bound, and they do not — a
character may hold a skill at any level and the sheet will show it. What has a
bound is one resolution: `MAX_TOTAL_DICE = 1_000`
(`crates/thunderforge-dice/src/eval.rs:14-23`), enforced after the count is
evaluated (`eval.rs:231`), and exceeding it fails the mutation with no roll
record written. Clamping the pool would be a silent lie about what was rolled.
Reaching level 1001 requires a thousand consecutive all-sixes advancements, so
this is a correctness statement rather than a scenario.

---

## D8. Every write and every roll is wrapped

**Decision**: each mutation the sheet issues is caught and surfaced through the
host's `StatusBadge`.

**Rationale**: `useUpdateActorData.mutate` rethrows on failure
(`apps/web/src/hooks/useUpdateActorData.ts:74-79`) and Genie's sheet calls it
as a bare `void handleX(...)`, so a rejected write shows the player nothing.
Roll for Shoes has three writes that must not silently vanish — the XP award,
the XP spend and the granted skill — and two refusals it is expected to
produce: a spend beyond the balance (FR-030) and an empty skill name (FR-037).
A sheet that swallows those fails its own requirements.

A write replaces the whole slot rather than merging
(`apps/web/src/api/actorSystemData.ts:85-100`), so every write spreads the
slot it read, and `refetch()` follows it. There is no subscription on actor
system data.

---

## D9. The proof is the `game-systems` slice

**Decision**: the pack's directory joins the `game-systems` slice's `paths`,
its e2e specification is named so that slice's `system-` prefix owns it, and
`pnpm e2e:game-systems` is this feature's named proof.

**Rationale**: constitution Principle VI asks for the *smallest* set of specs
crossing every seam the feature touches. Roll for Shoes touches the pack
machinery — manifest, enforced legal metadata, contributed-sheet resolution —
and that is precisely what `game-systems` proves today, through
`system-settings`, `system-panel-slots` and `system-change-guard`, with
`status-systems` as its neighbour. Genie holds its own slice because it has
fourteen specifications across three features; this pack adds one.

**A roll cannot be forced.** There is no seed and no deterministic-roll hook
anywhere in the server or the harness. The assertions available are the
invariants of the returned dice — one die per level, six sides each, every face
between one and six, the total equal to their sum — and arithmetic that cancels
the die out, which is the technique `roll-check.spec.ts` already uses. A
branch that depends on a particular face is asserted conditionally on the dice
that came back, or driven directly through the data mutation.

The sheet needs its own `data-testid`s. A contributed sheet has no
`[data-slot="sheet-layout"]` wrapper, and the system picker matches options by
manifest `title`, not by id.
