# Phase 1 Data Model: Optional Lore Synchronisation to an External Repository

**Feature**: `034-lore-git-sync` · **Date**: 2026-09-04

Four new tables. Every one carries `created_by`/`updated_by` provenance per
Constitution Principle III and ADR-009/ADR-010, and every mutation over them is
authorised server-side at the GraphQL boundary.

Existing lore tables (`world_lore_entries`, `world_lore_revisions`,
`world_lore_tags`, `world_lore_image_assets`, `world_lore_links`,
`world_lore_permissions`) are **read but never altered by this feature** in the
first delivery. That is not a style preference — Stories 1 and 2 contain no
write path into a world, which is what makes "a first delivery cannot damage
in-app lore" true by construction rather than by care.

---

## `lore_repository_connections`

One per world, at most (FR-001). The world's link to one external repository.

| Column | Type | Notes |
|---|---|---|
| `id` | `Uuid` | |
| `world_id` | `Uuid` | **Unique.** The uniqueness constraint *is* FR-001; enforcing it in application code would be enforcing it nowhere. |
| `host_kind` | `Text` | Which host adapter arranged the grant. Read at the grant boundary and nowhere else (FR-004c). |
| `installation_ref` | `Text` | Opaque to everything past the grant. Not a credential. |
| `repository_ref` | `Text` | Which repository, as the host names it. |
| `branch` | `Text` | |
| `directory` | `Text` | The subtree this world owns. Paired with `repository_ref` under a unique constraint (FR-033). |
| `incoming_enabled` | `Bool` | Default **false**. Story 3's gate; FR-006 and FR-022 both depend on the default being off. |
| `notice_acknowledged_at` | `Nullable<Timestamp>` | FR-038: synchronisation MUST NOT begin until FR-037's notice is acknowledged. Null means never started. |
| `state` | `Text` | `working` · `needs_attention` · `never_configured` · `deactivated`, the first three matching FR-029's words exactly. |
| `deactivated_at` | `Nullable<Timestamp>` | Set by an enforcement action (FR-041a). |
| `deactivated_reason` | `Nullable<Text>` | |
| `state_reason` | `Nullable<Text>` | Plain language, naming the remedy. Never a raw host error. |
| `last_synced_at` | `Nullable<Timestamp>` | |
| `last_written_commit` | `Nullable<Text>` | What we believe the remote head to be. FR-031 compares against this. |
| `created_by` / `updated_by` | `Uuid` | |
| `created_at` / `updated_at` | `Timestamp` | |

**Constraints**

- `UNIQUE (world_id)` — FR-001.
- `UNIQUE (repository_ref, directory)` — FR-033, two worlds may not write to
  one directory of one repository.
- Deleting a world cascades; deleting the connection leaves both the world's
  lore and the repository's contents untouched (FR-005).

**No credential column.** The installation reference is not a secret and the
token derived from it is never persisted — see R5. This table is safe to read
in full when diagnosing a connection, which is the property that keeps FR-035's
"never appears in logs" achievable rather than aspirational.

---

## `lore_sync_runs`

One attempt to bring a repository into agreement with a world.

| Column | Type | Notes |
|---|---|---|
| `id` | `Uuid` | |
| `connection_id` | `Uuid` | |
| `started_at` / `finished_at` | `Timestamp` / `Nullable<Timestamp>` | |
| `outcome` | `Nullable<Text>` | `succeeded` · `failed` · `stopped_for_divergence` · `stopped_for_collision` |
| `from_commit` / `to_commit` | `Nullable<Text>` | What it worked from and wrote. |
| `entries_written` | `Int4` | |
| `failure_reason` | `Nullable<Text>` | In terms a Game Master can act on. |
| `attempt` | `Int4` | Drives FR-030's backoff. |

Retained rather than overwritten, because FR-030's backoff and FR-029's "notify
once rather than repeatedly" (Story 2, scenario 6) are both statements about a
*history* of attempts. A single mutable status column cannot express either.

---

## `lore_exported_entries`

The durable association between an entry and the file representing it — the
single most important table here.

| Column | Type | Notes |
|---|---|---|
| `id` | `Uuid` | |
| `connection_id` | `Uuid` | |
| `lore_entry_id` | `Uuid` | |
| `current_path` | `Text` | Relative to the connection's directory. **A label, never a key** (R7). |
| `exported_revision_id` | `Nullable<Uuid>` | Which revision the file currently carries. How "changed on both sides" (FR-024) is answered. |
| `last_exported_at` | `Nullable<Timestamp>` | |

**Constraints**: `UNIQUE (connection_id, lore_entry_id)` — FR-007's "exactly
one file per entry". `UNIQUE (connection_id, current_path)` — two entries may
not claim one path.

**Why this table exists at all.** Without it, a rename is indistinguishable
from a delete plus an unrelated create, and FR-010's history preservation is
impossible. The entry id in the file header lets an *incoming* file be matched
(FR-009, FR-027); this row is what lets the *outgoing* side know which file to
move. They are two directions of the same identity and both are needed.

---

## `lore_fidelity_notes`

A recorded instance of something that could not be represented (FR-013, FR-037).

| Column | Type | Notes |
|---|---|---|
| `id` | `Uuid` | |
| `connection_id` | `Uuid` | |
| `lore_entry_id` | `Nullable<Uuid>` | Null for a note about the whole connection, such as permission flattening. |
| `kind` | `Text` | `unresolvable_cross_link` · `permission_not_carried` · `path_disambiguated` |
| `detail` | `Text` | |
| `first_seen_at` / `last_seen_at` | `Timestamp` | |

Rows, not log lines, because SC-008 requires every fidelity loss to be
*enumerated* rather than discovered by the user. Something enumerable must be
queryable.

---

## Deferred to Story 3

`lore_pending_incoming_changes` is specified in the spec's Key Entities and is
**not created in the first delivery**. Nothing in Stories 1 and 2 produces a
pending change, and a table with no writer is a table whose shape is guessed.
It arrives with the story that fills it.

---

## State transitions

A connection's `state` moves only on the outcome of a run:

```
never_configured ──(grant + FR-038 acknowledgement)──> working
working ──(run fails)──────────────────────────────> needs_attention
working ──(divergence or collision)────────────────> needs_attention
needs_attention ──(run succeeds)───────────────────> working
any ──(enforcement action)─────────────────────────> deactivated
deactivated ──(administrative action only)─────────> working
any ──(connection removed)─────────────────────────> row deleted
```

`needs_attention` always carries a `state_reason`. A state that says something
is wrong without saying what is a state that sends a Game Master to a support
channel, which FR-029 exists to prevent.

**`deactivated` is a one-way door for the owner** (FR-041a). It is the only
state a Game Master cannot leave by fixing something, and the only transition
out of it is administrative. That is what makes spec 015 FR-016's commitment to
a rights holder real rather than aspirational — a deactivation the owner could
undo is not a deactivation.

It is distinct from `needs_attention` on purpose (FR-041c). Telling someone to
check a connection they are not permitted to restore leaves them retrying
forever, and reads as a bug rather than a decision.
