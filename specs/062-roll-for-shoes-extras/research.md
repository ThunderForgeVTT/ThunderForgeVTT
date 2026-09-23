# Research: Roll for Shoes Extras

**Feature**: [spec.md](./spec.md) · **Date**: 2026-09-23

Spec 062 adds five opt-in per-world settings to the `roll_for_shoes` pack. This
records what the codebase already does, and the decisions taken before any code
is written.

## What the pack looks like today

Established by reading the pack; every claim here has a file behind it.

| Rule | Where it lives |
| --- | --- |
| Pool size = skill level | `web/src/ActorSheet.tsx` — as a `rollDice` binding, `(LEVEL)d6` |
| Dice randomness | Host: `src/server/src/graphql/mutations_roll.rs`, via `rollDice` |
| Verdict, tie included | `web/src/game.ts` — `verdict()` |
| XP on failure | `web/src/game.ts` — `xpAward()`, applied in `ActorSheet.tsx` |
| XP spent for a six | `web/src/game.ts` — `spendXp()` |
| All-sixes check | `web/src/game.ts` — `isAdvancement()` |
| New skill's level and parent | `web/src/game.ts` — `grantSkill()` |
| Starting skill | `web/src/game.ts` — `skillsOf()`, a read-time default; declared in `system.json` |
| Stored-shape legality | Pack Rust: `server/src/validators.rs` (rules T1–T11, R1–R3) |
| Persistence and authorisation | Host: `updateActorSystemData` |

Two properties of that layout decide most of this plan:

1. **`game.ts` is pure and separately tested.** It imports no React, no network
   and no host. Four of the five Extras are changes to functions there plus the
   sheet that calls them, provable by `node --test` before any browser opens.
2. **The pack's Rust crate owns no tables and contributes no GraphQL.** Its
   `Cargo.toml` says so in as many words. Anything that changes the *stored*
   shape must go through `validators.rs`, and anything that needs new storage
   has no pack-owned place to put it.

## D1 — Where the settings live

**Decision**: a pack-owned table, `world_roll_for_shoes_settings`, one row per
world, declared and written by `roll-for-shoes-server`. Read and written through
two root GraphQL fields the pack contributes.

**What was actually available**, established by reading the server:

| Candidate | Verdict |
| --- | --- |
| A generic per-world settings store | **Does not exist.** `worlds` has no `Jsonb` column; every column is a named scalar. |
| A column on `worlds` | Exists, and is how Genie's one setting works. Rejected — see below. |
| `instance_settings` key/value rows | Exists (ADR-091), but is **instance**-scoped. No world-scoped counterpart. |
| `data_types` in `system.json` | Exists, but validates **actor** JSON only. No world-scoped counterpart. |
| A pack-owned table | Exists, is the governing decision (ADR-063), and is how Genie's six `world_genie_*` tables work. **Chosen.** |

**Why not a column on `worlds`.** That is what
`worlds.genie_resource_carryover_enabled` does, and the Genie panel's own source
files it as a known defect — a column named for one ruleset on a table every
system shares. `scripts/check-system-registry.mjs` would not catch
`worlds.roll_for_shoes_tie_succeeds`, because a column name is neither a quoted
id nor a filename; passing that check while breaking the rule it exists to
enforce is worse than failing it. Five such columns would make one mistake five.

**Why not build the generic world-settings surface now.** ADR-063 sets the bar
itself: "One pack wanting something is a case; two is a shape." Genie's carryover
plus these five clears that bar, so a generic surface is now *justified* — and it
is still not this feature's job. ADR-063 also rejected the shortcut version of it
("a generic key-value store, which is a different design for a different
problem"), and ADR-091's instance registry shows the real shape: declared types,
declared defaults, declared redaction, a precedence rule and an audit trail.
That is a spec, not a paragraph of this one.

The choice made here is forward-compatible: a pack-owned table holding five
named, typed columns migrates into a declared registry by moving rows. A column
on `worlds` would not — it would need a migration *and* a rename *and* a
deprecation, which is precisely the loose end Genie is carrying.

**What this costs.** `roll-for-shoes-server` stops being the table-free crate its
`Cargo.toml` advertises. It gains `diesel`, `async-graphql` and
`thunderforge-server`, a migration under `src/server/migrations/`, an
`except_tables` entry in both `diesel.toml` files, and a `PackSurface`
declaration classifying its new fields — the read as `reads`, the write as
`gated`, with a request document, or `play_pause_surface_tests` fails. Genie's
`packs/systems/genie/server/src/session/` is the worked example for every one of
those steps.

That cost is the honest price of the feature. Four of the five Extras are pure
`game.ts` changes; it is the *settings* that need a server, and settings that
live nowhere cannot be per-world.

**One consequence worth stating.** The `world-settings` panel receives
`world: WorldRecord`, which will not carry these settings — they are not on
`worlds`. Both the panel and `ActorSheet.tsx` read them with their own query
through `postGraphQL` from `@thunderforge/host`, exactly as Genie's panel posts
its own GraphQL. A world with no row reads as all-defaults-off, so the row is
written on first change and never needs seeding.

**An ADR is owed.** Not for choosing the pack-owned table — ADR-063 already
decided that — but for declining to build the generic surface at the moment its
own stated threshold was met. That reasoning should be findable by whoever adds
the third case. Drafted as ADR-108 in the same change set (Principle IV).

## D2 — How the settings reach the screen

**Decision**: the pack contributes `web/src/panels/world-settings.tsx`.

`world-settings` is an existing panel slot (`apps/web/src/host/index.ts`), one of
four in the `PanelSlot` vocabulary. A pack fills it by putting a file of that
name under `web/src/panels/`, and `apps/web/src/panels/systemPanels.ts` finds it
with a build-time `import.meta.glob`. The slot's props are `worldId`, `world`,
`isGm` and `onWorldChanged` — exactly what a settings card needs, including the
"re-read the world" signal for after a toggle.

**Why it matters**: no shared file has to name `roll_for_shoes`, so
`scripts/check-system-registry.mjs` keeps passing with no exemption (FR-006).
The mechanism was built for precisely this and needs no extension.

**Alternative rejected**: a bespoke settings page inside the pack. It would need
a route, and routes are the host's; the slot exists so packs do not invent them.

## D3 — The order of a roll

**Decision**: dice → sum → statuses → opposition → comparison under the tie
rule. Fixed, stated once, and encoded as a single function in `game.ts` rather
than as a sequence of `if`s in the sheet.

**Why**: three Extras touch one roll. Left implicit, every pair of them is a
question somebody has to re-derive — does a status apply before or after the
comparison, does the tie rule see the modified total or the raw one. Written as
one ordered function it is one testable claim, and `game.test.ts` can assert the
order directly.

**Consequence**: `verdict(total, opposition)` grows into something like
`resolve({ faces, statuses, opposition, tieSucceeds })` returning the raw sum,
the modified total, the verdict and the XP awarded. The existing two-argument
`verdict` is kept or re-expressed in terms of it so the current tests stay
meaningful.

## D4 — Statuses never touch the advancement check

**Decision**: `isAdvancement(faces, bought)` keeps its exact signature and is
never passed a status.

**Why**: the guarantee in FR-023 — a status can neither create nor destroy an
advancement — is strongest when it is structural rather than remembered. A
function that cannot see statuses cannot be affected by them. `game.ts` already
documents that the check reads the dice and never the total; this keeps that
true by construction when a total finally becomes something other than the sum.

## D5 — The starting skill is a read-time default, and that is a problem

**The wrinkle**: `skillsOf()` returns `Do Anything 1` when nothing is stored.
Skills first persist when something else saves the character. So a character can
exist, be listed, and be opened, while holding no stored skills at all.

FR-039 says changing a world's starting skills must not alter existing
characters. Under a read-time default it would: a character created last week
and never saved would silently acquire the new starting skills, because it has
nothing of its own to show instead.

**Decision**: the world's starting skills are resolved **at the moment the
character first stores anything**, not on every read. Until then the character
has no skills of its own and reads as the world's current starting set — which
is the honest description of a character nobody has opened.

FR-039 is therefore read as protecting a character whose skills have been
stored. That is a narrowing of the spec's wording and is recorded here rather
than assumed.

**Why not seed at creation**: nothing in the pack runs at actor creation. The
pack contributes no GraphQL and the host's `createActor` knows no system rules —
making it seed would mean the host executing pack logic at creation time, which
is a much larger change than this feature earns.

**Why not seed on sheet mount**: a mount-time write needs edit permission, and
the play dock mounts sheets with `canEdit: false` for a player who may hold only
a claim. A character would then seed or not depending on who opened it first.

## D6 — Slot caps change the stored shape, so the validator changes with it

**Decision**: bought slots are stored on the character and validated by the
pack's Rust crate alongside the existing T-rules.

The existing validator enforces that exactly one skill is a root, that a child's
level is its parent's plus one, and that ids are unique — and deliberately
enforces **no cap on level**. Caps on *breadth* are new stored state (how many
slots this character has bought at a level), so they need a rule of their own.

**Explicitly not added**: a validator rule that refuses a character holding more
skills at a level than the caps allow. Enabling the setting in a world whose
characters already exceed the caps must not make those characters unstorable
(FR-036, and the spec's edge case). The cap governs *gaining* a skill, which is
a rule about a transition, not about a stored shape.

## D7 — Game Master dice use the roll path that already exists

**Decision**: a rolled difficulty band is a second `rollDice` call with a
`(BAND)d6` formula binding, exactly as a skill roll is `(LEVEL)d6`.

**Why**: the dice are then server-rolled and auditable like every other die in
the product, with no new mutation, no new formula syntax and no client-side
randomness. The band-to-count mapping (1/2/3/4) lives in `game.ts` as data.

**Consequence**: Game Master dice are a separate roll result from the
character's, which is what keeps FR-014 structural — they are never in the
`faces` array the advancement check reads.

## D8 — Proof

**Decision**: new e2e specs named `apps/web/e2e/system-roll-for-shoes-extras.spec.ts`.

The `game-systems` slice owns the `system-` prefix, so a spec with that name
joins the slice with no edit to `scripts/e2e/slices.json` or `package.json`. The
slice's `paths` already cover `packs/systems/{...,roll_for_shoes,...}/**`.

Per constitution Principle VI the proof is `pnpm e2e:game-systems`. The slice
measured 176s for 18 specs at its last recording, so there is room.

**Two constraints the tests inherit**, both learnt the hard way in spec 061:

- A player driving the play dock must be granted Editor on the character
  explicitly, because claiming grants no write access. Recorded separately as a
  host change that has not happened yet.
- The sheet reports a refused write in a badge rather than throwing, so every
  test that expects a write to land must also assert `rfs-error` has count 0.
  Without it a lost write reads as a disagreement about a number.

**No dice seed exists.** Tests are built from facts true of every roll, as spec
061's are — a single d6 cannot beat 6, four d6 cannot beat 24, a status of −100
cannot produce a positive total. Each Extra needs a case constructible that way.

## Unit-testability

`game.test.ts` covers the rules today with `node --test` and no browser. Every
decision above keeps the new rules inside `game.ts`, so difficulty bands, the
tie rule, status summing and slot arithmetic are all unit-testable without a
stack. The e2e then proves they reached a screen, rather than proving the
arithmetic.
