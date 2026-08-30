# Phase 0 Research: In-Engine Status Displays and the Engine UI SDK

Resolves the `NEEDS CLARIFICATION` items from [plan.md](./plan.md) and records
the decisions the Phase 1 contracts depend on.

---

## 1. Where the shared wire types live

**Decision**: The SDK's wire types are defined in
`thunderforge-canvas-core`, not in the engine crate. The engine imports them;
the TypeScript is generated from that crate.

**Rationale**: This is forced, and by a constraint that is independent of
which generator is chosen.

Any Rust→TypeScript generator has to _execute Rust_ on the host — as a test,
a build script, or a binary — to emit its output. The engine crate cannot do
that. It is `crate-type = ["cdylib", "rlib"]`, builds for
`wasm32-unknown-unknown`, and has no `wasm-bindgen-test` runner configured, so
its tests compile and never run. The constitution says as much
(Principle V), and the existing `#[cfg(test)]` modules in that crate are the
evidence: a module of intended coverage that reads green and executes nothing.

So type generation cannot be hosted in the engine crate whatever tool is
picked. `thunderforge-canvas-core` compiles and tests natively, is already the
home for the rules the engine renders (grid, vision, cursor, token kinds), and
is already an engine dependency. Putting the wire types there costs nothing
and is where generation can actually run.

**Alternatives considered**:

- _Types in the engine crate, generation via a separate native shim crate._
  Works, but adds a crate whose only purpose is to be buildable, and splits
  the definition from its documentation.
- _Hand-written TypeScript mirrors, as today._ This is the status quo the
  feature exists to end. Every shape crossing `apply_world_command` is
  mirrored by hand, the two drift, and a drifted field fails **silently** —
  the engine ignores what it cannot parse, so the symptom is a display that
  does not appear rather than an error anybody sees.

---

## 2. Which generator

**Decision**: `ts-rs` for the SDK command and state types.

**Rationale**: The central type is `ExternalCommand`, a Rust enum with
per-variant payloads. In TypeScript the correct rendering is a discriminated
union, which is what makes exhaustive `switch` handling and narrowing work on
the application side — the property that turns a mistake into a compile error
rather than a silent no-op. `ts-rs` emits that directly from the enum.

**Alternatives considered**:

- _`schemars` + `json-schema-to-typescript`._ This has real pull: `schemars`
  is already a dependency (`crates/pack_system_spec`), and the repository
  already publishes generated schemas under `docs/api/schemas/`. Continuing an
  established pattern is usually right. It is rejected here because JSON
  Schema is a validation vocabulary rather than a type language: Rust enums
  round-trip through it as `oneOf` and come out the far side as unions that
  narrow poorly, which loses exactly the property the SDK is being introduced
  for.

  This is a split by job, not a replacement: `schemars` stays where
  schema-as-contract is the point (manifest validation, published API
  schemas); `ts-rs` is used where the artifact wanted is a TypeScript type.
  The ADR should record that distinction so the next person does not read it
  as two tools doing one job.

- _`specta`._ Comparable output, smaller ecosystem, no existing use here.

**Verified 2026-08-29 (T002)**: `ts-rs` 12.0.1 produces what the decision
assumed, and one thing it did not.

The discriminated union is exactly right — a tagged Rust enum emits
`{ "disclosure": "visible", entries: … } | { "disclosure": "greyed" } | …`,
which narrows on the tag and is the property the whole choice rested on. Rust
doc comments carry through into TSDoc, so the disclosure warnings travel with
the type rather than living only in this repository.

`f32` becomes `number`, as expected. **`Option<T>` becomes `T | null`, not an
optional field** — `max: number | null`, not `max?: number`. That is stricter
than the contract draft assumed: the field must be present and explicitly
null rather than omittable. Stricter is the right direction here, so the
contract was corrected to match the generator rather than the generator
bent to match the contract.

Worth noting how that surfaced: a hand-written contract and a generated type
disagreed within minutes of each other, which is the drift this feature exists
to end, caught by the mechanism that ends it.

**Export location**: `TS_RS_EXPORT_DIR` is set to
`apps/web/src/engine/sdk/` by the regeneration script, so the bindings land
where the application imports them rather than in a `bindings/` directory
beside the crate.

---

## 3. How generated types stay honest

**Decision**: The generated TypeScript is committed, and a CI check
regenerates and fails on any difference.

**Rationale**: Generation alone does not prevent drift; it only moves where
drift can occur. A generated file that nobody regenerates is a hand-written
file with a misleading header. Committing it keeps the app buildable without a
Rust toolchain, and the diff check is what makes the commitment real.

This mirrors what already happens with `schema.rs` for diesel: generated,
committed, and wrong loudly rather than quietly if it falls behind.

---

## 4. SDK versioning

**Decision**: A single integer `sdkVersion` on the command envelope. The
engine rejects a mismatch with a reported error through the existing event
callback, and never partially applies.

**Rationale**: FR-019 requires rejection rather than partial application, and
FR-020 forbids silent discard. An integer is enough because both sides ship
together in one bundle — this is not a public API with independent release
cadences. Semantic versioning would imply a compatibility story nobody needs
and nobody would test.

**Alternatives considered**: per-command versioning (more granular than the
problem), and no versioning at all (leaves the current silent-ignore failure
mode intact for exactly the case the SDK exists to fix — a stale bundle).

---

## 5. Where disclosure state is stored

**Decision**: A new `token_resource_disclosure` table keyed by
`(token_id, resource_id)`, carrying the state and `created_by`/`updated_by`
provenance. Absence means the world default.

**Rationale**: FR-013d requires the state to be per token rather than per
actor, so two tokens of one creature can differ. A sparse table matches the
expected shape: most tokens use the default and store no row at all.

Storing it in `tokens.metadata` (JSONB) was considered and rejected — the
column is unstructured, unindexed and unconstrained, and this is
authorization-bearing data that Principle III requires be enforced at the data
boundary with real provenance.

**Resolved 2026-08-29**: there is no world-level default setting, because the
default **derives from the actor**.

A token is bound to an actor, and the actor already knows what it is —
`world_actors.actor_type` distinguishes a player character from an NPC, and
ownership says whose character it is. So the default falls out of data that
exists rather than from a setting somebody has to find and configure:

- **Your own character** — exact. You always know your own hit points.
- **Another player's character** — exact. Party members share this at a table.
- **An NPC** — chunked. Readable enough to play ("that ogre is nearly dead")
  without handing out figures the Game Master is entitled to keep.

An explicit `token_resource_disclosure` row still overrides, which is what the
GM control in US3a writes. The derived value is the floor, not a ceiling.

This is better than a configurable default for a reason worth stating: a
setting has to be discovered, and a table that never finds it plays under
whatever we guessed. A derived default is correct for a table that never
configures anything, which is most tables. It is also the third time this
feature has taken the same shape — the `token_type` backfill reads
`actor_type`, and the creation picker defaults to `npc` when staging an NPC.
Three inferences agreeing is a sign the actor is the right authority.

**Consequence for the engine binding**: the token's presentation is driven by
the actor behind it rather than configured on the token. That is the
difference between a board of interchangeable pieces and a board where each
piece is a character, and it is why this lands here rather than as a settings
screen.

---

## 6. Making the derived-stat pipeline execute

**Decision**: `TokenPlugin` attaches the `Token` component (and a new
`TokenStatus`) to spawned token entities. The existing
`calculate_derived_stats` / `calculate_ability_stats` systems then have input
for the first time.

**Rationale**: The subsystem is registered in the frame loop at `lib.rs:752`
and queries `(&Token, &mut DerivedStats)`. No spawned entity carries `Token` —
the only construction of that type anywhere is a unit test — so both systems
match nothing, every frame, forever. `TokenAbilities` is never constructed at
all.

This is not a separate cleanup to schedule later. That subsystem's only
possible consumer is a feature that displays what it computes, so the work of
attaching the component and the work of drawing the result are the same work,
and doing either alone leaves a dead end in place.

**Scope boundary**: making the components live is in scope. Adding a movement
speed to `DerivedStats` and gating movement on it is **not** — that is
ruleset enforcement, belongs to the Phase 8 game-system work, and the spec
excludes it explicitly.

---

## 7. Chunking and where it is computed

**Decision**: Quarter banding is computed **server-side** and transmitted as a
quarter index (0–4). The client receives no proportion and no totals.

**Rationale**: FR-013b. A client that never receives a figure cannot leak one,
whether through a bug, a devtools session, or a modified client. Computing the
band in the renderer would mean shipping the exact value to the machine of the
person it is being hidden from, which is the same mistake as a UI that hides a
field the API still returns.

The banding rule itself — the arithmetic mapping a value and its entries to an
index — belongs in `thunderforge-canvas-core` so it is unit-tested against
boundaries (exactly 25%, exactly zero, an entry list where the top entry is
spent), and is called by the server.

**Note for the contract**: percentage discloses materially more than chunked
(FR-013c — a viewer who knows the damage dealt can recover the maximum and
read exact values thereafter). Both are offered, but the SDK types should not
present the four states as interchangeable appearances.

---

## Summary of unknowns resolved

| Unknown                         | Resolution                                                                             |
| ------------------------------- | -------------------------------------------------------------------------------------- |
| Rust→TS generator               | `ts-rs`, hosted in `thunderforge-canvas-core`                                          |
| Why not the existing `schemars` | JSON Schema renders enums as poorly-narrowing unions; kept for schema-as-contract work |
| Drift prevention                | Commit generated output; CI regenerates and diffs                                      |
| SDK versioning                  | Single integer, reject on mismatch, report through the event callback                  |
| Disclosure storage              | `token_resource_disclosure`, sparse, per token per resource                            |
| Dead derived-stat systems       | Attach `Token`/`TokenStatus` in `TokenPlugin`; this feature is their consumer          |
| Chunking location               | Server-side; quarter index on the wire; arithmetic tested in canvas-core               |

## Still open, deliberately

- The **world-level default** disclosure state (§5) — a product decision.
- **`ts-rs` output quality** for `Option<T>`/`f32` payloads (§2) — verify
  before committing to it.
- Both spec-level deferred items remain closed as of the 2026-08-29
  clarification: entries resolve overflow, four states resolve GM control.
