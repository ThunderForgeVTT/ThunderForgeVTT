# Contract: World Access Link GraphQL Surface

**Phase 1** · Spec: [../spec.md](../spec.md) · Data model: [../data-model.md](../data-model.md)

Covers FR-001 – FR-014. Existing operations are marked **CHANGED**; the two
new mutations are marked **NEW**.

---

## Types

```graphql
enum WorldAccessLinkState {
  ACTIVE
  EXPIRED
  EXHAUSTED
  REVOKED
}

type WorldInvitePayload {
  id: UUID!
  worldId: UUID!
  inviteCode: String!
  maxUses: Int!
  usedCount: Int!
  expiresAt: String

  created_by: UUID!
  createdAt: String!
  updatedAt: String!

  "CHANGED — was a free-form string like \"3/10 uses\"."
  state: WorldAccessLinkState!

  "NEW — uses left, or null when maxUses is 0 (unlimited)."
  remainingUses: Int

  "NEW — the link this one replaced, if it was created by rotation."
  rotatedFrom: UUID

  "DEPRECATED — retained for one release. Prefer `state` + `remainingUses`."
  status: String!
}
```

**On `status`**: the existing field is a display string built server-side
(`format!("{}/{} uses", …)`). It cannot express revoked, so it is superseded by
`state`. It stays for one release rather than breaking any caller mid-flight;
the frontend moves to `state` in this feature.

---

## Queries

### `worldInvites(worldId: UUID!): [WorldInvitePayload!]!` — CHANGED

Lists **one world's own** links, for that world's DM. Behaviour is unchanged
apart from the payload gaining `state` / `remainingUses` / `rotatedFrom`.

| Rule | Behaviour |
|---|---|
| Authorization | Caller must be Owner or GM of `worldId` (FR-008) |
| Revoked links | **Included**, so a GM can see what they retired |
| Ordering | Newest first, stable |

**FR-009 boundary, stated precisely.** This query is world-scoped and
DM-gated, and is therefore *not* enumeration. What must not exist is any
operation returning links across worlds or across users, or resolving a link
without its code. No such operation is added, and none exists today.

---

## Mutations

### `generateInviteCode(input: GenerateInviteCodeInput!): WorldInvitePayload!` — CHANGED

```graphql
input GenerateInviteCodeInput {
  worldId: UUID!
  maxUses: Int!        # must be > 0; 0-as-unlimited is not creatable (data-model §5)
  expiresAt: String    # RFC 3339; null = never
}
```

Only the issued code changes: **20 characters** from an independent v4 UUID
(~80 bits), up from 8 (~32 bits) — FR-006. The code MUST NOT derive from any
time-ordered value; deriving from a v7 UUID caused a real unique-index
collision (spec 005), and the shared generator preserves that fix.

Authorization unchanged: Owner or GM only.

---

### `revokeInviteCode(inviteId: UUID!): WorldInvitePayload!` — NEW

Retires a link permanently without issuing a replacement (FR-002).

| Rule | Behaviour |
|---|---|
| Authorization | Owner or GM of the link's world (FR-008) |
| Already revoked | Succeeds idempotently; returns the row unchanged |
| Effect on members | **None.** Anyone who already joined stays (FR-005) |
| Reversibility | None. A revoked code never becomes usable again (FR-013) |

Returns the updated link so the panel can re-render without a refetch.

---

### `rotateInviteCode(inviteId: UUID!): WorldInvitePayload!` — NEW

Retires a link and issues its replacement in one atomic action (FR-003,
FR-004). **Returns the new link**, not the retired one.

| Rule | Behaviour |
|---|---|
| Authorization | Owner or GM of the link's world |
| Atomicity | Single transaction. Partial failure leaves exactly one usable link (FR-004) |
| Old code | Fails on its **next** use — no cache or grace window (SC-001) |
| Inherits | `maxUses`, `worldId`, and the chosen **lifetime** (see below) |
| Resets | `usedCount` → 0, and the expiry clock (FR-014) |
| Fresh | `id`, `inviteCode`, `createdBy` = the rotating GM |
| Links back | `rotatedFrom` = the retired link's id |
| Expired / exhausted source | **Allowed** — produces a usable link (US1 scenario 4) |
| Already-revoked source | **Refused** — would yield two replacements for one original |

**Expiry is re-based, not copied.** The replacement carries the lifetime the
GM originally chose, measured again from the rotation. Copying the absolute
expiry would mean a link rotated after it lapsed is born already dead, which
contradicts the row above allowing rotation of an expired source. Uses-spent
and elapsed time are both *consumed state*; the cap and the lifetime are both
*settings*. Rotation resets the state and keeps the settings. A link with no
expiry still has none.

**Cap reset is intentional and is not a security boundary.** Because
`usedCount` resets, a DM can rotate a 1-use link repeatedly to admit any
number of people. This is accepted (a DM can already create unlimited links).
GM-facing copy MUST NOT describe the use cap as enforcement.

---

### `joinWorld(input: JoinWorldInput!): WorldMembershipPayload!` — CHANGED

Behaviour visible to callers is unchanged on success; the failure surface and
the concurrency guarantee change.

**Order of operations** — the sequence matters and is contractual:

1. Resolve the code. Unknown → uniform failure.
2. **Already a member? → return the already-a-member error.** This precedes
   consumption so a repeat click never burns a use.
3. Atomically validate-and-consume one use (data-model §4). 0 rows → uniform
   failure.
4. Insert membership with role `Player`, in the **same transaction** as step 3.

**Uniform failure (FR-011).** Unknown, expired, exhausted, and revoked codes
MUST be indistinguishable — identical message, identical error extensions,
no timing or shape difference:

> `This invite link is no longer available.`

This mirrors the wording content share links already use
(`load_active_share`), so the two surfaces stay consistent.

**Already a member** is a *separate*, non-uniform message. It requires a valid
code, so it leaks nothing an attacker could not already establish:

> `You are already a member of this world.`

**Exactly-once under concurrency (FR-012).** Two callers racing for the last
use: exactly one succeeds, the other receives the uniform failure. The current
read-validate-write sequence admits both; the conditional `UPDATE` is what
makes this true.

---

## Frontend operations

`apps/web/src/api/world.ts` gains `revokeInviteCode` and `rotateInviteCode`.
Both go through the shared `postGraphQL` transport — no bespoke fetch.

`useWorldInvites` refetches explicitly after a successful revoke or rotate.
It has **no live push transport** (its own doc comment records this), so a
rotation performed elsewhere will not appear until refetch or remount. The
panel must not present its cached view as authoritative.

---

## Contract test checklist

| # | Assertion | Requirement |
|---|---|---|
| 1 | Rotated old code fails on next use | FR-003, SC-001 |
| 2 | Rotation is atomic — forced mid-transaction failure leaves exactly one usable link | FR-004 |
| 3 | Members who joined via the retired code keep membership | FR-005 |
| 4 | Rotation inherits cap and expiry, resets count to 0 | FR-014 |
| 5 | Rotating an expired link yields a usable one | US1-4 |
| 6 | Rotating a revoked link is refused | data-model §3 |
| 7 | Revoke is idempotent | FR-002 |
| 8 | Non-DM refused for revoke and rotate | FR-008 |
| 9 | Unknown / expired / exhausted / revoked all fail identically | FR-011, SC-005 |
| 10 | Already-a-member returns its own message, consuming no use | US4-2 |
| 11 | Concurrent last-use: exactly one join succeeds | FR-012, US4-3 |
| 12 | Newly issued codes are 20 chars, non-time-derived | FR-006, SC-007 |
| 13 | Pre-existing 8-char codes still join successfully | FR-007, SC-006 |
| 14 | `state` reports all four values correctly | FR-010 |
| 15 | No operation returns links across worlds or users | FR-009 |
