# Data Model: Unified World Access Links & Consolidated Permission Resolution

**Phase 1** · 2026-08-25 · Spec: [spec.md](./spec.md) · Research: [research.md](./research.md)

---

## §1. `world_invites` — extended in place

The world access link. Extended additively; no existing column changes type or
meaning, so every live row remains valid (FR-007).

| Column | Type | Change | Notes |
|---|---|---|---|
| `id` | `UUID PK` | — | `Uuid::now_v7()`, app-supplied |
| `world_id` | `UUID NOT NULL` | — | FK → `worlds(id)` |
| `invite_code` | `VARCHAR(32) UNIQUE` | **value shape only** | New codes are 20 chars; existing 8-char codes untouched. Column width already sufficient. |
| `max_uses` | `INT4 NOT NULL` | — | `0` = unlimited (see §5) |
| `used_count` | `INT4 NOT NULL` | — | Incremented only by the atomic consume (§4) |
| `expires_at` | `TIMESTAMP NULL` | — | `NULL` = never expires |
| `created_by` | `UUID NOT NULL` | — | FK → `users(id)` |
| `created_at` / `updated_at` | `TIMESTAMP NOT NULL` | — | |
| **`revoked`** | `BOOLEAN NOT NULL DEFAULT FALSE` | **NEW** | Explicit retirement. The default is what makes existing rows read as active. |
| **`rotated_from`** | `UUID NULL REFERENCES world_invites(id)` | **NEW** | Set on a replacement link; `NULL` for an original. Self-referential, nullable, `ON DELETE SET NULL`. |

**Indexes**: existing unique on `invite_code` retained (it is the collision
guard spec 005's fix depends on). Add a partial index on
`(world_id) WHERE revoked = FALSE` if the panel query shows cost; not required
at current scale.

**Validation rules**

- `max_uses >= 0`. The creation path continues to reject `<= 0` (§5).
- `rotated_from` MUST reference a row in the same `world_id`.
- A row with `rotated_from` set MUST have been created against a row that was
  revoked in the same transaction (FR-004 — enforced by the rotate operation,
  not by a constraint).

---

## §2. Link State — derived, never stored

The answer to "does this link work right now". Computed from the row; **not** a
column, so it cannot drift from the facts that produce it.

| State | Condition | Precedence |
|---|---|---|
| `REVOKED` | `revoked = TRUE` | 1 (highest) |
| `EXPIRED` | `expires_at IS NOT NULL AND expires_at <= now()` | 2 |
| `EXHAUSTED` | `max_uses > 0 AND used_count >= max_uses` | 3 |
| `ACTIVE` | none of the above | 4 |

**Precedence matters for display only.** A link can be both revoked and
expired; the GM sees the most decisive reason. For *enforcement* the states
collapse to a single boolean — usable or not — and the caller is never told
which applied (FR-011).

**Two evaluation sites, deliberately asymmetric**:

- **Server, authoritative**: evaluated inside the SQL predicate of the atomic
  consume (§4). This is the only evaluation that gates access.
- **Client, display-only**: derived in the invite panel for badges and
  remaining-use counts. It may be momentarily stale — `useWorldInvites` has no
  live push (research §8) — and MUST never be treated as authorization.

---

## §3. Rotation

A rotation is one transaction producing two row states:

```
BEGIN
  UPDATE world_invites SET revoked = TRUE, updated_at = now()
   WHERE id = $old AND revoked = FALSE;          -- 0 rows ⇒ abort, already retired

  INSERT INTO world_invites
    (id, world_id, invite_code, max_uses, used_count,
     expires_at, created_by, rotated_from, created_at, updated_at)
  VALUES
    ($new_id, $world_id, $new_code, $old.max_uses, 0,
     $old.expires_at, $caller, $old.id, now(), now());
COMMIT
```

**Inherited**: `max_uses`, `expires_at`, `world_id`.
**Reset**: `used_count` → 0 (FR-014).
**Fresh**: `id`, `invite_code`, `created_by` (the rotating GM, who may differ
from the original creator), `created_at`.

**Rotation is permitted on an expired or exhausted link** (US1 scenario 4) —
the guard is only `revoked = FALSE`, so a GM can always revive a dead link into
a working one. It is refused on an already-revoked link, which would otherwise
produce two replacements for one original.

---

## §4. Use consumption — atomic

Replaces the read-validate-write sequence that currently loses updates under
concurrency (research §5). One statement carries the entire validity predicate:

```sql
UPDATE world_invites
   SET used_count = used_count + 1,
       updated_at = now()
 WHERE invite_code = $1
   AND revoked = FALSE
   AND (expires_at IS NULL OR expires_at > now())
   AND (max_uses = 0 OR used_count < max_uses)
RETURNING id, world_id;
```

- **1 row** → consumed exactly once; proceed to insert membership *in the same
  transaction*.
- **0 rows** → unusable for some reason the caller is never told (FR-011).

The membership insert and this update share a transaction, so a failure to
create membership returns the use.

**Already-a-member** is checked separately and is *not* a failure state: it
requires a valid link and returns its own message (US4 scenario 2). It must be
evaluated **before** consuming a use, so a repeat click does not burn the cap.

---

## §5. Preserved quirk: `max_uses = 0`

`WorldInvite::is_valid()` treats `0` as unlimited, but the creation path
rejects `<= 0`, so unlimited links are unreachable through the API
(research §7).

**Decision**: the SQL predicate in §4 carries `max_uses = 0 OR …` so any row
that has one behaves as the model always claimed. No API path to create one is
added. Removing the branch is a behaviour change outside this spec.

---

## §6. Permissioned Content Type — the single declaration

Not a table. A compile-time declaration listing every content type that
participates in the permission model. One entry per type; adding a type means
adding one entry and nothing else (FR-017).

Each entry supplies exactly the four tokens that differ:

| Type | Grants table | Content FK | User column | Parent table |
|---|---|---|---|---|
| Actor | `world_actor_permissions` | `actor_id` | `user_id` | `world_actors` |
| Item | `world_item_permissions` | `item_id` | `user_id` | `world_items` |
| Lore | `world_lore_permissions` | `lore_entry_id` | **`world_member_user_id`** | `world_lore_entries` |
| Ability | `world_ability_permissions` | `ability_id` | `user_id` | `world_abilities` |

**Generated per entry**: `effective_<type>_permission`,
`require_<type>_permission`, and a member-grant cleanup.
**Generated once, over all entries**: `purge_member_grants(conn, world_id, user_id)`.

**Not generated, deliberately**: `is_ability_visible_to`. Visibility is a
separate axis from the permission ladder (FR-019, research §3) and stays
hand-written in `ability_permissions.rs`.

**No schema change.** The four permission tables are untouched; this is a code
consolidation over existing storage.

---

## §7. Permission Grant

One member's explicit rights on one content row. Unchanged by this feature —
documented because the cleanup contract depends on its FK behaviour.

| Column | Notes |
|---|---|
| `id` | `UUID PK`, app-supplied v7 |
| `<content>_id` | FK → parent, **`ON DELETE CASCADE`** |
| `<user column>` | FK → `users(id)`, **`ON DELETE CASCADE`** |
| `level` | `VARCHAR(16)`, one of `Viewer` / `Editor` / `Owner` |
| `created_at` / `updated_at` | |
| `UNIQUE (<content>_id, <user column>)` | upsert conflict target |

**The cascades that exist, and the gap that does not.**

| Event | Grants removed? | By what |
|---|---|---|
| Content row deleted | ✅ | FK cascade |
| User account deleted | ✅ | FK cascade |
| **Member removed from world** | ❌ | **Nothing** — no FK path from `world_members` |

The third row is the bug. There is no FK from `world_members` to the grant
tables (the relationship runs through `world_id` on the parent content table),
so removal cleanup must be explicit. Today it is written by hand three times
and omitted a fourth (abilities). `purge_member_grants` (§6) replaces all four
call sites with one derived from the declaration.

---

## §8. Effective Rights

The resolved answer for one member and one content row. Unchanged in outcome
(FR-021) — restated because it is what the macro must reproduce exactly.

```
if is_dm_of_world(world_of(content))  → Owner        # implicit, un-removable
else if explicit grant exists          → grant.level
else                                   → Viewer      # default
```

`Viewer` being both the floor and the default is precisely why hidden-ness
cannot be expressed here (§6, FR-019).

**Rank order**: `Viewer(0) < Editor(1) < Owner(2)`. `require_*` compares
`level.rank() >= minimum.rank()` and returns a `FORBIDDEN`-extended error
otherwise — byte-identical behaviour to the four current implementations.

---

## Entity relationships

```
worlds ─┬─< world_invites ──┐ rotated_from (self, nullable)
        │        └───────────┘
        ├─< world_members            (removal ⇒ purge_member_grants)
        │
        ├─< world_actors      ──< world_actor_permissions   >── users
        ├─< world_items       ──< world_item_permissions    >── users
        ├─< world_lore_entries──< world_lore_permissions    >── users
        └─< world_abilities   ──< world_ability_permissions >── users
                    │
                    └── gm_only (visibility — separate axis, not a grant)
```
