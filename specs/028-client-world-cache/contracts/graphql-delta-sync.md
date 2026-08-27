# Contract: Delta Sync over GraphQL

**Feature**: 028-client-world-cache

Extends the existing schema. Asset **bytes** continue to travel over the
existing authenticated `GET /canvas-assets/{asset_id}` route — this contract
adds no second byte path, deliberately, so the authorization already written
and tested for that route (ADR-039, FR-014) applies unchanged.

---

## Query: `worldSyncPlan`

```graphql
input HeldItemInput {
  """Opaque item identity: "scene:<uuid>" or "asset:<uuid>"."""
  id: String!
  """Lowercase hex SHA-256 the client believes it holds."""
  fingerprint: String!
}

type PlanItem {
  id: String!
  fingerprint: String!
  byteSize: Int!
  """Whether peers in this session are known to hold it (FR-044). Advisory."""
  peerAvailable: Boolean!
}

type SyncPlan {
  fetch: [PlanItem!]!
  evict: [String!]!
  """Server's view of the caller's budget ceiling, bytes. Advisory."""
  budgetHint: Int
  """Canonical-serialization version. A mismatch invalidates every
     scene-state fingerprint the client holds."""
  canonicalVersion: Int!
}

extend type Query {
  worldSyncPlan(worldId: UUID!, held: [HeldItemInput!]!): SyncPlan!
}
```

### Authorization

- Requires an authenticated session and world membership, via the same
  `require_world_member` path `canvasImageAssetsForScene` uses.
- The plan is computed **from what the caller may see**, not filtered
  afterwards. An item the caller lacks permission for can therefore never
  appear in `fetch`. If the caller *claims* to hold one, it appears in
  `evict` — the same answer a deleted item produces, so its existence is
  still not disclosed while the client is correctly told to discard it.
- Per-object permissions (ADR-050) narrow the plan further: an actor the
  caller may not view contributes nothing.

### Behaviour

| Condition | Result |
|---|---|
| Held fingerprint matches current | Item omitted entirely (this is the win) |
| Held fingerprint differs | Item in `fetch` with the current fingerprint |
| Server fingerprint is `NULL` (un-backfilled) | Item in `fetch` — never treated as unchanged |
| Item no longer exists | Item id in `evict` |
| Caller lost permission since caching | Item id in `evict` |
| Client claims an item it may not see | Item id in `evict` — byte-identical to a deleted item |
| `held` is empty | Full plan — the cold-start case |

**`evict` deliberately conflates "gone", "forbidden", and "never existed."**
All three produce a byte-identical response, which is what makes FR-015 fall
out of ordinary cache correctness rather than needing a parallel revocation
channel — and what prevents the manifest from becoming an existence oracle.

An earlier draft of this table said an unauthorized held item was "omitted
from both lists." That was wrong, and would have been a disclosure bug in the
opposite direction from the one it was trying to avoid: silence means "your
copy is current", so the client would have gone on holding content it may no
longer see, indefinitely. Forbidden content must be evicted. Non-disclosure
is preserved by making the eviction indistinguishable from a deleted item's,
not by staying quiet about it.

### Errors

| Case | Response |
|---|---|
| Not authenticated | Standard auth error |
| Not a world member | Same error as any other non-member access — must not reveal whether the world exists |
| `held` exceeds a sane bound | Rejected; clients must page |
| Malformed fingerprint | Rejected; do not silently coerce |

---

## Mutation: `reconcileQueuedChanges`

Replays changes made while disconnected (US7).

```graphql
input QueuedChangeInput {
  """Client-generated correlation id."""
  localId: String!
  """The emitted world-store command, verbatim."""
  command: JSON!
}

enum RejectionReason {
  PERMISSION_DENIED
  SUPERSEDED
  GONE_AWAY
  INVALID
}

type ReconcileOutcome {
  localId: String!
  applied: Boolean!
  reason: RejectionReason
  """Set when reason is SUPERSEDED: who won, so the UI can say so."""
  supersededByRole: String
}

extend type Mutation {
  reconcileQueuedChanges(
    worldId: UUID!
    changes: [QueuedChangeInput!]!
  ): [ReconcileOutcome!]!
}
```

### Behaviour

- Each change is authorized **at reconnect time**, against current
  permissions — never against what the user had when they made it (FR-042).
- Every input yields exactly one outcome. A missing outcome is a contract
  violation, because silent loss is prohibited (FR-041).
- Conflicts resolve by `thunderforge-cache-core::conflict`: GM beats player;
  same role, earlier reconnect wins. Client-supplied timestamps are ignored.
- Changes are applied in submitted order within one call, so a client's own
  sequential edits do not reorder against each other.
- Application emits ordinary `world_events`, so other connected clients see
  reconciled changes through the existing subscription with no special path.

### The `Applied → Superseded` case

A player's change may be applied on reconnect and later superseded when a GM
reconnects with a conflicting offline edit. The player is already gone from
this call by then, so the supersession is delivered through the normal
`world_events` subscription, and the client must recognise its own
previously-applied `localId` being overridden and tell the user.

This is the sharpest edge in the feature. It is not an error path — it is
the specified behaviour of GM precedence, and it needs real UX.

---

## Peer availability

`PlanItem.peerAvailable` is advisory only. A client MUST behave identically
whether it is true, false, or ignored — peer transfer is a strict
optimization with a mandatory server fallback (FR-048). The server does not
promise a peer is reachable, only that one recently reported holding that
fingerprint.
