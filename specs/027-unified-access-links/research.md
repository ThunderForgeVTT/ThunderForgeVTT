# Research: Unified World Access Links & Consolidated Permission Resolution

**Phase 0** · 2026-08-25 · Spec: [spec.md](./spec.md)

Findings are grounded in the current source, not assumed. Line references are
to the tree at `42b8be9`.

---

## §1. How to generalize four Diesel-typed permission modules

**Decision**: a single declarative macro invocation listing every permissioned
content type, expanding to concrete per-type functions plus one aggregate
cleanup function.

**The constraint that drives this.** Diesel gives every table its own distinct
generated type. A function generic over "any permissions table" needs bounds on
`Table`, `Column`, `SelectableExpression`, `QueryFragment`, `AppearsOnTable`,
and the query DSL types for each `filter`/`select` in the body. In practice
that bound list is longer than the four function bodies it would replace, and
it fails with type errors that name Diesel internals rather than anything in
this codebase.

The four modules differ in exactly three tokens:

| Type | Grants table | Content FK | User column |
|---|---|---|---|
| Actor | `world_actor_permissions` | `actor_id` | `user_id` |
| Item | `world_item_permissions` | `item_id` | `user_id` |
| Lore | `world_lore_permissions` | `lore_entry_id` | **`world_member_user_id`** |
| Ability | `world_ability_permissions` | `ability_id` | `user_id` |

Everything else — the DM→Owner shortcut, the explicit-grant lookup, the
Viewer default, the rank comparison, the `FORBIDDEN` extension — is identical
across all four. A macro parameterised on those three tokens plus the parent
content table produces the same typed code that exists today, from one
declaration.

**Why this satisfies FR-017 specifically.** The requirement is that declaring a
new content type in one place supplies *both* resolution and removal cleanup.
Because a single macro invocation lists all types, it can emit both the
per-type resolvers and an aggregate `purge_member_grants` that walks every
declared type. There is no second list to keep in sync — which is the precise
mechanism by which the ability-cleanup gap (US2) occurred.

**Alternatives considered**:

- **Trait with associated Diesel types** — rejected on the bound explosion
  above. This is a well-known friction point with Diesel's type-level DSL, not
  a skill issue; the workaround (`Box<dyn>` erased queries) discards the
  type safety that motivates Diesel.
- **`diesel::sql_query` with a runtime table name** — rejected. Interpolating
  a table name into SQL to serve authorization is the wrong place to give up
  compile-time checking, and it defeats `schema.rs` drift detection.
- **Unify the four tables into one polymorphic
  `world_content_permissions(content_type, content_id, …)`** — considered
  seriously, since the repo already has precedent (ADR-043's
  `content_moderation_actions`). **Rejected**, and the reason matters: every
  permission table today declares
  `<content>_id REFERENCES world_<content>(id) ON DELETE CASCADE`. Deleting a
  piece of content therefore removes its grants automatically. A polymorphic
  table cannot carry that FK, so every content-delete path would need explicit
  grant cleanup — trading the one missing-cleanup bug we are fixing for the
  same bug class on a hotter and more numerous set of paths. Moderation could
  accept this because a moderation row *should* outlive its target; a
  permission row must not.

**Honest cost**: macro-generated functions do not support go-to-definition and
produce worse error messages at call sites. Mitigation: keep the macro body
thin and mechanical, keep the declaration adjacent to it so both are read
together, and keep every non-uniform behaviour (below) out of it.

---

## §2. Where `is_dm_of_world` belongs

**Decision**: move it into `src/server/src/auth/world_membership.rs`.

It answers a world-level membership question, and it is already implemented by
calling `require_world_member` from that very module
(`actor_permissions.rs:40`). It lives in `actor_permissions.rs` only because
spec 010 needed it first. `lore_permissions.rs:12` currently launders the
import with `pub use crate::auth::actor_permissions::is_dm_of_world;`, which is
direct evidence the location is wrong.

**Scope of the move**: 49 call sites, of which 20 are `use` statements and 7
are fully-qualified inline paths in `graphql.rs`. Mechanical.

**Alternative rejected**: leave it and add a `pub use` shim in a neutral
module. That makes two valid import paths for one function and preserves the
confusion the spec asked to remove.

**Note**: `require_world_member` is synchronous (`&mut PgConnection`) while
`is_dm_of_world` is async (`&AppState`). Co-locating them is still correct —
they answer the same question at two layers — but the signature difference is
deliberate and should not be "harmonised" as part of this work.

---

## §3. What must NOT be absorbed into the generalization

**Decision**: `is_ability_visible_to` stays hand-written in
`ability_permissions.rs`; the macro emits resolution and cleanup only.

Per FR-019 and spec 025's own module doc: `ActorPermissionLevel`'s lowest value
(`Viewer`) is also its default for a member with no row, so the ladder
structurally cannot express "hidden". Visibility is `world_abilities.gm_only`,
a separate axis. Folding it into a generic permission resolver would either
give every content type a meaningless visibility parameter or tempt a future
change to express hidden-ness as a permission level — the exact confusion spec
025 documented at length.

**The lore column difference is absorbed, not migrated.** `world_lore_permissions`
names its user reference `world_member_user_id`. Renaming it would touch live
data for cosmetic uniformity. It becomes a macro parameter.

---

## §4. Access link storage

**Decision**: extend `world_invites` in place. Add `revoked BOOLEAN NOT NULL
DEFAULT FALSE` and `rotated_from UUID NULL REFERENCES world_invites(id)`.

**Why extend rather than create a new table**: FR-007 requires every live code
to keep working. A new table means either a data migration of live invite rows
or two lookup paths during a transition — both riskier than one additive
migration. `DEFAULT FALSE` makes every existing row read as active, which is
exactly correct.

**Why `rotated_from`**: it makes the panel able to say "this replaced link X"
and gives revocation an audit trail, at the cost of one nullable column. It
also makes FR-013 (a retired code never becomes usable again) inspectable
rather than merely asserted.

**No column-width migration needed**: `invite_code` is already
`VARCHAR(32)`; the new 20-character codes fit.

**Rejected**: unifying `world_invites` with the three content share tables at
the storage level. The spec lists this as a non-goal, and the design work
confirms it: an invite grants *membership in a world*, a content share grants
*a read-only preview plus copy-to-world*. They differ in what they reference
(a world vs. a content row), what they confer, and who may use them. One table
would need a nullable half for each case.

---

## §5. Making rotation and use-consumption correct under concurrency

**Decision**: one `UPDATE … WHERE …` that validates and consumes atomically,
inside the same transaction as the membership insert.

**This fixes a live race.** `join_world_impl`
(`mutations_invites.rs:284-314`) currently reads the invite, validates it
in memory via `CoreWorldInvite::is_valid()`, calls `use_invite()` on the
in-memory copy, then writes the computed count back:

```
core_invite.use_invite()?;
let updated_count = core_invite.used_count;
diesel::update(...).set(world_invites::used_count.eq(updated_count))
```

Two concurrent joins on a 5-use link with `used_count = 4` both read 4, both
compute 5, both write 5. Two members are admitted against one remaining use.
FR-012 requires exactly one. The fix is a conditional update whose `WHERE`
clause carries the whole validity predicate:

```
UPDATE world_invites
   SET used_count = used_count + 1, updated_at = now()
 WHERE invite_code = $1
   AND revoked = FALSE
   AND (expires_at IS NULL OR expires_at > now())
   AND (max_uses = 0 OR used_count < max_uses)
RETURNING id, world_id
```

Zero rows affected means unusable — for any reason — which lands FR-011's
uniform failure for free rather than as a separate string-matching exercise.

**Rotation** is two statements in one transaction: set `revoked = TRUE` on the
old row, insert the replacement carrying the same `max_uses` and `expires_at`
with `used_count = 0` and `rotated_from` set. FR-004's atomicity is the
transaction; a failure rolls back to exactly one usable link.

**Alternative rejected**: `SELECT … FOR UPDATE` then write. Correct, but holds
a row lock across more work for no benefit over a conditional update. Spec 017
already chose the constraint-based equivalent for actor claims
(`specs/017-invite-actor-selection`), so this matches established practice.

---

## §6. Code generation and entropy

**Decision**: extract the existing share-code generator into one shared helper
and use it for invites — 20 uppercase hex characters from an independent v4
UUID (~80 bits).

Current invite codes take 8 characters (~32 bits) from a v4 UUID
(`mutations_invites.rs:200-207`). Share links already take 20
(`mutations_ability_shares.rs:41-49`). The generators are otherwise identical,
so this is deduplication, not new code.

**Preserve the prior fix.** Spec 005 found and fixed a real collision: codes
were derived from a v7 UUID, which front-loads a millisecond timestamp, so two
invites created in the same millisecond collided on the unique index. The
current v4 source is deliberate and the shared helper must keep it. The macro
of this mistake is documented at `mutations_invites.rs:189-198` and must not
be lost in the refactor.

**Old codes are not upgraded.** FR-007 forbids invalidating live links, and
rewriting an existing row's code would do exactly that.

---

## §7. Dead branch found

`WorldInvite::is_valid()` treats `max_uses == 0` as unlimited
(`src/core/src/models/invites.rs`), but `generate_invite_code_impl` rejects
`max_uses <= 0` (`mutations_invites.rs:165-167`). Unlimited links are therefore
unreachable through the API while the model still branches on them.

**Decision**: keep the model's `max_uses == 0` semantics and carry them into
the SQL predicate (§5) so behaviour is preserved for any row that has one, but
do not add an API path to create them in this feature. Removing the branch
would be a behaviour change outside this spec's scope; documenting it prevents
the next reader assuming it is reachable.

---

## §8. Frontend surface

**Decision**: extend the existing invite panel; add no new route.

`useWorldInvites.ts` already fetches a world's invites and computes derived
display data client-side (`computeInviteDerivedData`). Link state (FR-010) is
derived, not stored (data-model §2), so it belongs alongside that existing
derivation — with the caveat that expiry must be evaluated server-side for
enforcement and client-side only for display.

**Known limitation, carried forward not fixed**: `useWorldInvites` has no live
push transport; its own doc comment says invites will not update when another
user generates or uses one. Rotation must therefore refresh the list
explicitly on success rather than assume a subscription will deliver it.

---

## Resolved unknowns

| Question | Answer | Section |
|---|---|---|
| How to generalize four Diesel-typed modules | One declarative macro over all types | §1 |
| Where the DM check lives | `auth/world_membership.rs` | §2 |
| What stays hand-written | `is_ability_visible_to`; lore column absorbed as a parameter | §3 |
| New table or extend | Extend `world_invites`, additive migration | §4 |
| Exactly-one-use under concurrency | Conditional `UPDATE` carrying the validity predicate | §5 |
| Code strength | Shared generator, 20 hex chars, v4-sourced | §6 |
| Unlimited-uses branch | Preserved in SQL, not newly exposed | §7 |
| Where the refresh control goes | Existing invite panel, explicit refetch | §8 |

No `NEEDS CLARIFICATION` items remain.
