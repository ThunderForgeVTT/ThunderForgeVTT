# Phase 1 Data Model: Client-Side World Cache

**Feature**: 028-client-world-cache | **Date**: 2026-08-26

Types live in `thunderforge-cache-core` unless marked otherwise, so both the
server and the engine use one definition.

---

## Server-side persistence

### `canvas_image_assets` — amended

One new column. No other change to the table.

| Column | Type | Notes |
|---|---|---|
| `content_hash` | `Nullable<Text>` | Lowercase hex SHA-256 of the **stored** (post-transcode WebP) bytes. Nullable so the migration can ship before the backfill completes. |

**Rules**

- Written on upload, in the same transaction as the row itself.
- `NULL` means "fingerprint unknown" and MUST be treated as "client must
  fetch" — never as "unchanged". Correct-by-default during backfill.
- Immutable once set for a given `asset_id`; replacing content means a new
  asset row, never a mutated hash.
- Indexed, because peer transfer and dedup both look items up by hash.

### `scene_state_fingerprints` — new

Fingerprint of a scene's logical state, distinct from its assets.

| Column | Type | Notes |
|---|---|---|
| `scene_id` | `Uuid` PK | |
| `content_hash` | `Text` | SHA-256 over the canonical serialization |
| `computed_at` | `Timestamp` | |
| `updated_by` | `Uuid` | Per Principle III / ADR-009 |

**Rules**

- Recomputed when any `world_events` row lands that mutates the scene.
- Computed from `CanonicalSceneState` (below), never from a raw DB dump —
  row order and float formatting must not affect the hash.

---

## Shared types (`thunderforge-cache-core`)

### `ItemId`

Identifies one cacheable thing, independent of where it is stored.

```
ItemId =
  | SceneState { scene_id }
  | CanvasAsset { asset_id }
```

Deliberately closed: adding compendium or system-pack content later is a
change to this enum and its handling, not something that happens by
accident.

### `Fingerprint`

Wrapper over a 32-byte SHA-256 digest. Hex on the wire.

**Rules**

- Constructed only from bytes actually hashed — never parsed from a claim
  without verification at the point of use.
- Equality is the only operation callers need; ordering exists solely for
  deterministic serialization.

### `HeldItem`

What a client says it has. **A claim, not an entitlement.**

```
HeldItem { id: ItemId, fingerprint: Fingerprint }
```

**Rules**

- The server MUST NOT infer any permission from an item's presence here.
  A client claiming to hold an item it may not see receives that item in
  neither `fetch` nor `evict` — it is simply omitted, revealing nothing.

### `Manifest`

The client's account of one world: a set of `HeldItem`, plus the world it
describes. Serialized in a stable order so two clients in identical states
produce identical bytes.

### `SyncPlan`

The server's answer.

```
SyncPlan {
  fetch:  [PlanItem],   # client must obtain these
  evict:  [ItemId],     # client must discard these
  budget_hint: Option<u64>,
}
```

```
PlanItem { id, fingerprint, byte_size, source_hint }
```

**Rules**

- Silence about an item means the client's copy is current. Absence is
  meaningful, and is what makes an unchanged world nearly free.
- `evict` covers both superseded content and **content the caller may no
  longer see** — the same channel serves cache correctness and FR-015. The
  client cannot distinguish the two, and must not try.
- `fetch` MUST NOT contain anything the caller is unauthorized for, because
  the plan is computed after authorization, not filtered afterwards.

### `CanonicalSceneState`

The stable serialization scene fingerprints are taken over.

**Rules**

- Entities sorted by identifier; no reliance on map iteration order.
- Floats serialized with fixed precision — an f32 that round-trips through
  the DB must not produce a different hash than the one that went in.
- Excludes anything per-viewer (selection, camera) — two users looking at
  the same scene differently must agree on its fingerprint.
- Versioned: a change to the canonical form changes every fingerprint, so it
  carries an explicit version that participates in the hash.

### `BudgetPlan`

```
BudgetPlan { limit_bytes, in_use_bytes, evict: [ItemId] }
```

**Rules**

- `limit_bytes` = min(quota × 0.5, 20GB), recomputed per world open (R8).
- Never selects items belonging to the currently-open world (FR-023).
- Evicts whole worlds before individual items within a world.

### `QueuedChange`

One offline edit awaiting reconnection.

```
QueuedChange {
  local_id,          # client-generated, for correlating the outcome
  world_id,
  command,           # the emitted world-store command, verbatim
  enqueued_at,       # client clock — diagnostics only, NEVER for conflicts
  actor_role,        # GM or Player, as known when enqueued
}
```

**Rules**

- `command` is stored verbatim so replay traverses the same mutation and
  authorization path as an online change (FR-042). It is an outbox entry,
  not a model.
- `enqueued_at` MUST NOT influence conflict resolution — client clocks are
  forgeable and routinely wrong (FR-040a).
- `actor_role` is a *hint* for predicting the outcome client-side. The
  server re-derives the real role at reconnect and never trusts this field.
- Durable before the originating change is acknowledged locally, or a closed
  tab loses work silently (FR-037).

### `ReconcileOutcome`

```
ReconcileOutcome =
  | Applied { local_id }
  | Rejected { local_id, reason: PermissionDenied | Superseded | GoneAway | Invalid }
```

**Rules**

- Every `QueuedChange` produces exactly one outcome. A change that produces
  none is a bug, because silent loss is prohibited (FR-041).
- `Superseded` MUST identify that a GM's change won, so the UI can say so
  rather than reporting a generic failure.

### `ConflictVerdict`

Pure function, shared so client prediction and server decision cannot drift.

```
resolve(a: QueuedChange, b: QueuedChange, order: ReconnectOrder) -> Winner
```

**Rules**

- GM beats Player, regardless of reconnect order (FR-040).
- Same role: earlier reconnect wins (FR-040a).
- Total and deterministic — never "it depends", never a tie.
- The server's verdict is final; a client's prediction is advisory (FR-040b).

---

## Client-side persistence (`thunderforge-cache-browser`)

### OPFS layout

```
/{user_scope}/{world_id}/{fingerprint}.bin      # encrypted bytes
```

**Rules**

- Named by **fingerprint, not identifier**: identical content shared across
  scenes or worlds is stored once, and a peer-supplied blob lands at the
  path its verified hash dictates.
- `user_scope` is derived from the session, so two users on one machine
  never collide (FR-003).
- Every blob encrypted under the session key before write (FR-016).
- A blob whose decrypted bytes do not hash to its own filename is corrupt by
  definition — self-validating, which is what makes FR-018 cheap.

### IndexedDB stores

| Store | Key | Holds |
|---|---|---|
| `index` | `ItemId` | fingerprint, byte size, last read, world |
| `outbox` | `local_id` | `QueuedChange`, in enqueue order |
| `keys` | `user_scope` | non-extractable `CryptoKey` |
| `meta` | fixed | canonical-form version, budget state |

**Rules**

- `index` and OPFS can disagree — a crash between writes guarantees it
  eventually. Reconciling them is FR-019's repair, and the fingerprint
  filenames make it decidable without re-downloading anything.
- `keys` entries deleted on sign-out (FR-016a); everything else may be
  reclaimed lazily (FR-016b).
- `outbox` survives key loss as a *record that work was lost*, so FR-041's
  "never silently discarded" holds even when the payload is unreadable.

---

## State transitions

### Cached item

```
Absent ──fetch──▶ Verifying ──hash ok──▶ Present
                      │
                      └──hash mismatch──▶ Absent  (discard, re-fetch: FR-046)

Present ──superseded──▶ Absent          (new fingerprint)
Present ──evicted──────▶ Absent          (budget, FR-023)
Present ──revoked──────▶ Absent          (permission, FR-015)
Present ──corrupt──────▶ Absent          (self-check, FR-018)
```

Every path out of `Present` ends at `Absent`, and `Absent` is always
recoverable by fetching. There is no degraded state — which is what makes
repair (US3) tractable.

### Queued change

```
Enqueued ──reconnect──▶ Submitted ──▶ Applied
                             │
                             └──▶ Rejected(reason)   → user informed

Applied ──GM conflict arrives later──▶ Superseded    → user informed (FR-041)
```

`Applied → Superseded` is the asymmetry GM-over-player creates: a terminal
state that is not, in fact, terminal until every participant has reconnected.

---

## Validation rules

| Rule | Source |
|---|---|
| Received bytes must hash to the promised fingerprint before use or storage | FR-010, FR-046 |
| A `NULL` server fingerprint means fetch, never "unchanged" | R3 |
| Client fingerprint claims never grant permission | FR-014, FR-047 |
| Conflict resolution never reads a client timestamp | FR-040a |
| Every queued change yields exactly one outcome | FR-041 |
| The open world is never evicted | FR-023 |
| Diagnostics never leave the device | FR-052 |
