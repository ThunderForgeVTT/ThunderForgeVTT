# ADR-050: One Permission Declaration, and World Invites as Revocable Access Links

**Date**: 2026-08-26

**Status**: Accepted (2026-08-26)

**Accountable owner**: Michael Bruno, project owner

**Supersedes/Amends**: none. Implements spec `027-unified-access-links`.
Extends the ownership model established in ADR-009/ADR-013/ADR-028 and
Constitution Principle III.

## Problem Statement

Two decisions here look unrelated but share one cause: **authorization
primitives were built once per noun instead of once.**

### A. Four copies of one permission rule

`src/server/src/auth/` carries four modules — `actor_permissions.rs`,
`item_permissions.rs`, `lore_permissions.rs`, `ability_permissions.rs` — each
with a near-verbatim `effective_*_permission` / `require_*_permission` pair.
All four resolve identically:

```
DM of the owning world → Owner (implicit, un-removable)
else explicit grant row → that level
else                    → Viewer
```

Only the backing table and the noun differ. The duplication is acknowledged in
the source and never resolved: `item_permissions.rs` calls itself "a direct
structural mirror" of `actor_permissions.rs`, and `ability_permissions.rs`
calls itself a mirror of `item_permissions.rs`.

**It cost us a live privilege leak.** `remove_member_impl` cleaned up a removed
member's grants for actors, items and lore in three hand-written blocks. Spec
025 added `world_ability_permissions` and never added the fourth. A removed
member kept their ability grants; re-adding them silently restored Editor or
Owner rights. Fixed by hand in commit `6de2add`; this ADR is about making the
class of omission impossible.

Separately, `is_dm_of_world` — the single DM check, with 49 call sites across
moderation, dice, items, abilities, lore and world mutations — lives inside
`actor_permissions.rs`, an actor-specific module. `lore_permissions.rs`
launders the awkward import with `pub use`.

### B. Invites and share links are complementary halves

`world_invites` and the three `world_*_shares` tables are two half-finished
versions of one primitive:

| | `world_invites` | `world_*_shares` |
|---|---|---|
| Expiry | ✅ | ❌ |
| Use cap | ✅ | ❌ |
| Revoke | ❌ | ✅ |
| Code entropy | 8 chars (~32 bits) | 20 chars (~80 bits) |

The missing revoke is the user-visible consequence: **a GM cannot stop a leaked
invite link.** Their only remedy is to remove each unwanted member after they
have already joined and seen the world.

## Decision

### 1. Permission resolution is generated from a single declaration

One declarative macro invocation lists every permissioned content type,
supplying only the tokens that differ — grants table, content FK, user column,
parent table, noun. It emits the per-type `effective_*` / `require_*` functions
**under their existing names and signatures** (so no resolver call site
changes), plus a per-type cleanup, plus one aggregate `purge_member_grants`
that walks every declared type.

Adding a permissioned content type is one entry. That entry carries both
resolution and removal cleanup, which is the mechanism that makes the spec-025
omission unrepeatable.

**Not generated, deliberately**: `is_ability_visible_to`. Visibility is a
separate axis from the permission ladder — `Viewer` is both the ladder's floor
and its default, so the ladder structurally cannot express "hidden", which is
why `world_abilities.gm_only` exists. The macro must never gain a visibility
parameter "for symmetry"; that invites the next content type to express
hidden-ness as a permission level.

`is_dm_of_world` moves to `auth/world_membership.rs`, beside the
`require_world_member` it already calls, and the `pub use` shim is deleted
rather than repointed.

### 2. `world_invites` becomes a revocable, rotatable access link

Extended additively with `revoked BOOLEAN NOT NULL DEFAULT FALSE` and
`rotated_from UUID NULL`. Gains explicit revocation and rotation — retire the
old code and issue a replacement in one transaction. New codes use the same
20-character generator as share links.

**Rotation retires the old code immediately.** Both-codes-valid was considered
and rejected: surviving a leak is the entire point, and a grace window defeats
it. The replacement inherits the cap and expiry with the count reset, so a
refresh yields "this link, but new".

**Invites and content share links stay distinct at the storage level.** An
invite grants membership in a world; a content share grants a read-only preview
plus copy-to-world. They differ in what they reference, what they confer, and
who may use them. One table would need a nullable half for each case.

## Alternatives Considered

### Trait with associated Diesel types (rejected)

Diesel gives every table its own generated type. A function generic over "any
permissions table" needs bounds on `Table`, `Column`, `SelectableExpression`,
`QueryFragment`, `AppearsOnTable` and the query DSL types for each
`filter`/`select` in the body — a bound list longer than the four function
bodies it replaces, failing with errors that name Diesel internals rather than
anything in this codebase. The usual escape (`Box<dyn>` erased queries)
discards the type safety that motivates using Diesel.

### One polymorphic `world_content_permissions` table (rejected — and this is the load-bearing rejection)

Superficially the cleanest answer, with direct precedent: ADR-043's
`content_moderation_actions` is polymorphic over content types, so the pattern
is already accepted here.

**Rejected because every permission table declares
`<content>_id REFERENCES world_<content>(id) ON DELETE CASCADE`.** Deleting a
piece of content removes its grants automatically, today, with no code. A
polymorphic table cannot carry that FK. Every content-delete path would need
explicit grant cleanup — trading the one missing-cleanup bug this ADR exists to
prevent for the same bug class on a hotter and far more numerous set of paths.

Moderation could accept polymorphism precisely because a moderation row
*should* outlive its target. A permission row must not.

### Runtime table name via `diesel::sql_query` (rejected)

Interpolating a table name into SQL to serve authorization is the wrong place
to abandon compile-time checking, and it defeats `schema.rs` drift detection.

### A new access-link table, migrating invites onto it (rejected)

Would require either migrating live invite rows or running two lookup paths
during a transition. An additive migration with `DEFAULT FALSE` makes every
existing row read as active, invalidating nothing.

## Consequences

**Good**

- A new permissioned content type is one declaration entry; it cannot ship with
  a missing or divergent check, or with missing removal cleanup.
- `is_dm_of_world` resolves from a module that matches what it does.
- A GM can kill a leaked invite link — the capability did not previously exist.
- Invite codes go from ~32 to ~80 bits.
- Consolidating the code generator also fixes a latent concurrency bug in
  `join_world_impl`, whose read-validate-write sequence admitted two members
  against one remaining use.

**Costs, stated plainly**

- **Macro-generated functions lose go-to-definition** and produce worse error
  messages at call sites. Mitigated by keeping the macro body thin and
  mechanical and the declaration adjacent to it, and by keeping every
  non-uniform behaviour out of it.
- **The use cap becomes resettable.** Because rotation resets the count, a DM
  can rotate a 1-use link indefinitely to admit any number of people. Accepted:
  only a DM can rotate, and a DM can already create unlimited links. The cap is
  a convenience control, **not a security boundary**, and GM-facing copy must
  not describe it as one.
- **`max_uses = 0` (unlimited) remains unreachable via the API** while the model
  still branches on it. Preserved in the SQL predicate so any such row behaves
  as the model claims; removing the branch is a behaviour change left out of
  scope.

**Neutral**

- No DMCA guardrail implication. An access link admits a person *into* a world;
  it exposes no compendium content across worlds and creates no new sharing
  surface. ADR-049's determination for content share links is unchanged, and no
  new determination is required.
- The `world_lore_permissions.world_member_user_id` naming asymmetry is
  absorbed as a declaration parameter, not migrated. Renaming it would touch
  live data for cosmetic uniformity.

## Verification

The consolidation is behaviour-preserving, so its acceptance is defined by
*absence* of change: the entire pre-existing authorization suite must pass
**unmodified**. A test edited to accommodate the change is evidence that
behaviour moved, not that a test was stale.

The cleanup test must derive its type list from the declaration rather than
restating four types by hand — a test that restates the list cannot catch the
omission it exists to prevent.
