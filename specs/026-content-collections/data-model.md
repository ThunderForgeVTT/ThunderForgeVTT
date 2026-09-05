# Data Model: Content Collections

**Feature**: `026-content-collections` · **Date**: 2026-09-04

Three new tables. Column conventions follow the shipped `world_*_shares` tables
and Principle III's `created_by`/`updated_by` provenance.

---

## `world_collections`

A named set of artifacts belonging to one world.

| Column | Type | Notes |
|---|---|---|
| `id` | `UUID PRIMARY KEY` | |
| `world_id` | `UUID NOT NULL REFERENCES worlds(id) ON DELETE CASCADE` | FR-003: a collection belongs to exactly one world. |
| `name` | `VARCHAR(200) NOT NULL` | |
| `description` | `TEXT` | Optional; shown in the preview (US4). |
| `created_by` | `UUID NOT NULL REFERENCES users(id)` | FR-017a's counterpart on the authoring side. |
| `updated_by` | `UUID NOT NULL REFERENCES users(id)` | Principle III. |
| `created_at` / `updated_at` | `TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP` | |

**Index**: `world_collections_world_id_idx` on `world_id`.

That index deserves a word, because it looks like the enumeration FR-020
forbids. It is not: it serves *an owner listing their own world's collections*,
which FR-020 explicitly permits ("beyond a user's own"). What must never exist
is an index or query reaching collections **by share code across worlds**, or
any resolver returning collections the caller does not own. The existing
`world_ability_shares` migration carries the same warning and it should be
restated in this one.

---

## `world_collection_members`

The association between a collection and one artifact.

| Column | Type | Notes |
|---|---|---|
| `id` | `UUID PRIMARY KEY` | |
| `collection_id` | `UUID NOT NULL REFERENCES world_collections(id) ON DELETE CASCADE` | |
| `member_type` | `VARCHAR(32) NOT NULL` | One of `actor`, `item`, `ability`, `lore`, `scene`. |
| `member_id` | `UUID NOT NULL` | **Deliberately not a foreign key** — see below. |
| `sort_order` | `INT NOT NULL DEFAULT 0` | Preview ordering. |
| `added_by` | `UUID NOT NULL REFERENCES users(id)` | |
| `created_at` | `TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP` | |

**Unique**: `(collection_id, member_type, member_id)` — adding the same artifact
twice is a no-op, not a duplicate.

**Index**: `world_collection_members_collection_id_idx`.

### Why `member_id` carries no foreign key

A polymorphic column cannot. `ADR-050` records this exact tradeoff being weighed
for the four permission tables and resolved the other way — four tables, each
with its own `ON DELETE CASCADE` — because a polymorphic table cannot carry the
FK that made deletion safe there.

Here the opposite resolution is right, and the reason is the edge case the spec
already names: **a member deleted from its world after being added must not make
the collection unopenable.** A cascading FK would delete the membership row
silently; five typed tables would be five near-identical tables and five places
to forget a type. So the row survives its artifact, and the read path resolves
each member and **skips what no longer resolves**, recording it as a fidelity
note (FR-015) exactly as it does for a moderated member.

The cost is honest: a `member_id` can dangle. That is acceptable *because
nothing in this feature trusts it* — every read resolves the artifact and
handles absence, and no copy path assumes presence.

### What this table deliberately does NOT have

**No `disabled` column.** Moderation status is asked at read and copy time via
`moderation::effective_status`, which performs lazy auto-restoration. A cached
flag would be stale in both directions: serving a taken-down artifact, or
withholding a restored one after FR-025 says it should return.

**No `restricted` column**, for the same reason. FR-001b requires an artifact
that *becomes* restricted to be withheld from that point, which a value captured
at add time cannot express.

---

## `world_collection_shares`

The unguessable code by which a collection is reachable, and its state. Separate
from the collection so one may be shared, revoked and shared again without
losing its identity (spec, Key Entities).

| Column | Type | Notes |
|---|---|---|
| `id` | `UUID PRIMARY KEY` | |
| `collection_id` | `UUID NOT NULL REFERENCES world_collections(id) ON DELETE CASCADE` | |
| `share_code` | `VARCHAR(32) NOT NULL UNIQUE` | From `generate_link_code()` — v4-derived, never v7 (FR-008). |
| `created_by` | `UUID NOT NULL REFERENCES users(id)` | |
| `revoked` | `BOOLEAN NOT NULL DEFAULT FALSE` | Soft flag, never a delete (FR-010). |
| `created_at` / `updated_at` | `TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP` | |

**Index**: `world_collection_shares_collection_id_idx` only. The `UNIQUE` on
`share_code` is the lookup; no index exists that would let shares be listed by
world or by user, matching the discipline the spec-025 migrations state
explicitly.

**On deleting a collection** (US2 scenario 4): the `ON DELETE CASCADE` removes
the share row, so the code resolves to nothing. FR-010 requires "no longer
available" to be **distinct from a code that never existed** — and the resolver
cannot tell those apart once the row is gone. So both must produce the *same
user-facing sentence*, and that is not a compromise: FR-009d requires that an
outsider cannot distinguish a revoked collection from a nonexistent one, because
distinguishing them is a probe. The distinctness FR-010 asks for is between
"no longer available" and an **error**, which is preserved.

---

## Not a table: Copy Record and Fidelity Note

The spec's remaining two entities are **response shapes, not stored rows**.

A **Copy Record** is a receipt for the person who copied: what arrived, what did
not. FR-012 forbids any referential link between the copies and the source, and
a stored copy record is exactly such a link — it would name a collection and the
rows made from it, which is the thing the one-time-deep-copy invariant exists to
prevent. It is returned from the copy mutation and shown once.

A **Fidelity Note** is one entry in that receipt: a reference that could not be
brought across (FR-015), a member withheld by moderation (FR-022, unnamed), a
member no longer resolving, or a scene child not copied by design (tokens, fog,
interactives — see research §4).

---

## State transitions

**A collection**: `draft` (no share row) → `shared` (an active share row) →
`revoked` (`revoked = true`) → `shared` again (a new share row, a new code).
Deleting the collection removes everything and the code stops resolving.

**A member**, resolved fresh on every read — never stored:

| Condition | Preview | Copied? |
|---|---|---|
| Resolves, unrestricted, unmoderated | Shown in full | Yes |
| Moderated (`effective_status` is `Some`) | "Something has been withheld", unnamed (FR-022) | No (FR-021) |
| Became restricted (grant rows or `gm_only`/`hidden`) | Same withheld treatment (FR-001b) | No |
| No longer resolves (artifact or world deleted) | Same withheld treatment | No |
| **Every** member in one of the above | "Nothing is available" (FR-024) | Refused |
