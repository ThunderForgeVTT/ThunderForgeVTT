# Contract: Consolidated Permission Resolution

**Phase 1** · Spec: [../spec.md](../spec.md) · Research: [../research.md](../research.md) §1–§3

Covers FR-015 – FR-021. This is an **internal Rust contract**, not a GraphQL
surface: no schema changes, no resolver signature changes, and — by
construction — no observable behaviour change (FR-021 / SC-003).

---

## The declaration

One invocation, listing every permissioned content type. Adding a type means
adding one entry here and nothing anywhere else (FR-017).

```rust
// src/server/src/auth/permissioned_entities.rs
permissioned_entities! {
    Actor {
        grants:     world_actor_permissions,
        content_fk: actor_id,
        user_fk:    user_id,
        parent:     world_actors,
        noun:       "actor",
    },
    Item {
        grants:     world_item_permissions,
        content_fk: item_id,
        user_fk:    user_id,
        parent:     world_items,
        noun:       "item",
    },
    Lore {
        grants:     world_lore_permissions,
        content_fk: lore_entry_id,
        user_fk:    world_member_user_id,   // differs — absorbed, not migrated
        parent:     world_lore_entries,
        noun:       "lore entry",
    },
    Ability {
        grants:     world_ability_permissions,
        content_fk: ability_id,
        user_fk:    user_id,
        parent:     world_abilities,
        noun:       "ability",
    },
}
```

`noun` exists only to build the existing error strings verbatim — "You do not
have sufficient permission on this **actor**". Changing that wording is out of
scope; SC-003 treats it as observable behaviour.

---

## What the macro emits

**Per entry:**

| Generated | Signature | Replaces |
|---|---|---|
| `effective_<t>_permission` | `(&AppState, Uuid /*user*/, bool /*admin*/, Uuid /*content*/) -> GraphQLResult<ActorPermissionLevel>` | the four hand-written copies |
| `require_<t>_permission` | `(&AppState, Uuid, bool, Uuid, ActorPermissionLevel) -> GraphQLResult<()>` | ditto |
| `purge_<t>_grants_for_member` | `(&mut PgConnection, Uuid /*world*/, Uuid /*user*/) -> QueryResult<usize>` | the hand-written blocks in `remove_member_impl` |

**Once, over all entries:**

```rust
pub fn purge_member_grants(
    conn: &mut PgConnection,
    world_id: Uuid,
    user_id: Uuid,
) -> QueryResult<usize>
```

Calls every generated `purge_<t>_grants_for_member` and sums the rows removed.
**This is the mechanism behind FR-018**: the set of types it walks is the
declaration itself, so a type cannot be declared and then forgotten by cleanup.

**Names are preserved exactly.** The generated `effective_actor_permission`,
`require_item_permission`, etc. keep their current names and signatures, so
none of the ~40 resolver call sites change. The consolidation is invisible
above this module.

---

## Resolution semantics — reproduced exactly

```
1. world_id ← SELECT world_id FROM <parent> WHERE id = $content_id
              └─ not found ⇒ Err("<Noun> not found")
2. is_dm_of_world(world_id) ⇒ Owner            # implicit, un-removable (FR-015)
3. SELECT level FROM <grants>
    WHERE <content_fk> = $content_id AND <user_fk> = $user_id
              └─ present and parseable ⇒ that level
4. otherwise ⇒ Viewer                          # default
```

`require_*` then compares `level.rank() >= minimum.rank()`, returning on
failure an `Error` extended with `code = "FORBIDDEN"` — unchanged.

**Preserved edge behaviours** (each is currently relied upon; a generalization
that "cleans these up" violates FR-021):

- An unparseable `level` string falls back to `Viewer`, not an error.
- `is_admin = true` short-circuits to DM before any query.
- Multiple simultaneous `Owner` grants are all valid — Owner is uncapped.
- A missing content row errors; a missing *grant* row does not.

---

## What is NOT generated

### `is_ability_visible_to` — stays hand-written

Visibility is a **separate axis** from the permission ladder (FR-019). The
ladder's floor (`Viewer`) is also its default, so it structurally cannot
express "hidden"; `world_abilities.gm_only` does that instead.

This function stays in `ability_permissions.rs`, outside the macro, with its
existing doc comment intact. Two consequences the implementation must respect:

- A member with **Editor** on a GM-only ability still cannot see it. Rights and
  visibility are evaluated independently and both must pass.
- Only abilities have a visibility axis today. The macro MUST NOT gain a
  visibility parameter "for symmetry" — that invites the next content type to
  express hidden-ness as a permission level, which is the confusion spec 025
  documented at length.

### `is_dm_of_world` — moved, not generated

Relocates from `auth/actor_permissions.rs` to `auth/world_membership.rs`
(research §2). It is a world-level question already implemented by calling
`require_world_member` from that module, and `lore_permissions.rs`'s
`pub use` shim is removed rather than repointed.

Signature and behaviour unchanged. ~49 call sites update their import path;
every one is compiler-verified, so a miss is a build error.

---

## Member removal — the fix

`remove_member_impl` currently contains three hand-written cleanup blocks
(actors, items, lore) and is **missing a fourth for abilities**, which spec 025
added without extending this path. A removed-then-readmitted member silently
regains their ability grants.

**Sequenced deliberately** (plan §Implementation Sequencing):

- **Phase 3** adds the fourth block **by hand**, so the privilege leak is
  closed independently of the refactor.
- **Phase 5** replaces all four blocks with one `purge_member_grants` call,
  so the omission cannot recur.

Both phases must satisfy: cleanup is scoped to the named world only, succeeds
quietly on an empty set, and leaves other worlds' grants untouched.

---

## Contract test checklist

| # | Assertion | Requirement |
|---|---|---|
| 1 | All four types resolve identically under identical conditions | FR-015, US5-1 |
| 2 | DM with zero grants resolves Owner on every type | FR-015, US5-2 |
| 3 | Member with no row resolves Viewer on every type | FR-015 |
| 4 | Unparseable `level` falls back to Viewer, not an error | FR-021 |
| 5 | `is_admin` short-circuits to Owner | FR-021 |
| 6 | Multiple simultaneous Owners all accepted | FR-021 |
| 7 | Missing content row errors; missing grant row does not | FR-021 |
| 8 | Editor on a GM-only ability still cannot see it | FR-019, US5-3 |
| 9 | Removal purges grants on **all four** types | FR-018, US2-1 |
| 10 | Removed-then-readmitted member holds nothing | US2-2, SC-008 |
| 11 | Removal in World A leaves World B grants intact | US2-3 |
| 12 | Removal with zero grants succeeds quietly | Edge Cases |
| 13 | The type set walked by cleanup is derived from the declaration, not restated | FR-017, SC-002 |
| 14 | Whole pre-existing authorization suite passes **unmodified** | FR-021, SC-003 |

**Test 14 is the load-bearing one.** It is satisfied only if no existing test's
expected outcome was edited to accommodate this change. An edit there is a
signal that behaviour moved, not that a test was stale.

**Test 13 needs care to be meaningful.** It must fail if someone adds a type to
the declaration and cleanup silently skips it — so it should assert over the
declared set programmatically, not against a hardcoded list of four, which
would merely restate the bug in test form.
