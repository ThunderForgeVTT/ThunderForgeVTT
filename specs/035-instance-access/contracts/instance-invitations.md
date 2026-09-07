# Contract: Instance Invitations

**Plan**: [../plan.md](../plan.md) | **Data model**: [../data-model.md](../data-model.md)

Covers US2. The gate is in [instance-access.md](./instance-access.md).

**This is not a world invite.** A world invite admits an *existing user* to
*one world's table* and is granted by a Game Master. An instance invitation
admits *a person with no account* to *the application* and is granted by the
operator. They compose — a stranger needs both — and neither replaces the
other. `mutations_invites.rs` is not touched by this feature.

---

## Administrative surface (admin only)

```graphql
type Query {
  instanceInvitations: [InstanceInvitation!]!    # admin only; never paged by code
}

type Mutation {
  createInstanceInvitation(input: CreateInstanceInvitationInput!): InstanceInvitation!
  revokeInstanceInvitation(invitationId: UUID!): Boolean!
}

input CreateInstanceInvitationInput {
  maxUses: Int          # default 1
  expiresInHours: Int   # null = no expiry
  note: String          # operator's own label; never shown to the recipient
}

type InstanceInvitation {
  id: UUID!
  inviteCode: String!            # admin-visible ONLY
  maxUses: Int!
  usedCount: Int!
  remainingUses: Int!
  state: InstanceInvitationState!
  expiresAt: String
  note: String
  createdBy: UUID!
  createdAt: String!
  redemptions: [InstanceInvitationRedemption!]!   # US2 scenario 8
}

enum InstanceInvitationState { ACTIVE, REVOKED, EXPIRED, EXHAUSTED }

type InstanceInvitationRedemption {
  userId: UUID!
  username: String!
  redeemedAt: String!
  route: String!     # "local" | "oauth:<provider_key>"
}
```

`state` is derived, never stored (data-model). It exists only on this
admin-only surface: to a redeemer, all three non-active states are one refusal.

**No lookup by code.** There is deliberately no `instanceInvitation(code:)`
query. The only thing that may resolve a code is redemption itself, which
consumes a use — mirroring the no-enumeration invariant ADR-049 depends on for
share links.

---

## Redemption

The recipient opens `/invite/{code}` and completes **either** local
registration or a first-time provider sign-in. Both routes carry the code to
the gate.

```
POST /api/authentication/register
{ "username": "...", "email": "...", "password": "...",
  "invitationCode": "ABCD..." }         // new optional field

GET  /api/authentication/oauth/{provider}/start?invitation=ABCD...
```

The OAuth code rides the existing authorization session (`oauth.rs`'s
`load_and_consume_authorization_session`) so it survives the provider round
trip without a cookie of its own.

### The one statement that admits

Per data-model, the validity predicate lives in the UPDATE's WHERE clause, so
check-and-increment is indivisible (SC-006). Redemption is one transaction:

0. **Rate limit the attempt, before the code is looked up** (FR-019a). Reuses
   `graphql::share_rate_limit` — the limiter already guarding the anonymous
   share reads, which already declines to honour the e2e bypass flag for the
   reason ADR-070 records. A throttled caller gets a **distinguishable**
   refusal, so a legitimate recipient retrying is never told their invitation
   is bad.
1. **Is this already a user?** If so, stop: authenticate them and consume
   nothing (FR-020a). This runs **before** step 2 for the reason spec 027
   records at `mutations_invites.rs:195` — an operator who tests the link they
   just issued must not burn the single use meant for its recipient.
2. `UPDATE instance_invitations SET used_count = used_count + 1 WHERE <predicate> RETURNING id`
3. zero rows → refuse, uniformly
4. create the account (ADR-042's rules unchanged for the OAuth route)
5. insert `instance_invitation_redemptions`
6. insert an `invitation_redeemed` event

A failure at 4 rolls back 2 — **a failed signup must not burn a use.**

Step 1 is not a disclosure risk: it requires a *valid* code, so it reveals
nothing to anyone who does not already hold one, and the caller learns only
about their own account.

### Uniform refusal (US2 scenarios 4 and 5)

Revoked, expired, exhausted and never-existed are one response:

```
409 Conflict
{ "error": "invitation_unusable",
  "message": "This invitation is no longer valid" }
```

OAuth equivalent: `302 → /login?error=invitation_unusable`.

Distinguishing them tells a stranger holding a guessed code whether they guessed
a real one.

---

## Invariants

- **A redeemed account is an ordinary account** (US2 scenario 7). No
  invited-user flag, no residual status, nothing that affects ownership,
  export or deletion. The redemption row records history; it grants nothing.
- **The policy switch neither invalidates nor extends an issued invitation**
  (US2 scenario 6). Validity is revocation, expiry and use count only. A
  `closed` instance still refuses it — that is the door being shut, not the
  invitation being void.
- **`inviteCode` never leaves the admin surface.** Not in the public status
  response, not in an event row, not in a log line.
- **Codes come from `share_codes::generate_link_code`** — v4-derived, never
  v7, for the reason ADR-049 gives: a v7 code leaks its creation time and
  narrows a guess.

---

## Test contract

| Requirement | Proven by |
|---|---|
| US2 2–3 — redeem via local **and** OAuth on a closed... | e2e, both routes, on an `invite_only` instance |
| SC-005 — revoked admits nobody | Unit + e2e: revoke, then redeem → refused, user count unchanged |
| SC-006 — at most N under concurrency | Unit: N+2 concurrent redemptions of an N-use invitation admit exactly N |
| US2 4–5 — uniform refusal | Unit: revoked, expired, exhausted and unknown produce byte-identical bodies |
| US2 6 — policy switch does not alter validity | Unit: issue while closed, open, close, redeem — governed only by its own fields |
| US2 7 — ordinary account | Unit: the redeemed user differs in no column from one created on an open instance |
| US2 8 — who redeemed | Unit: an admin read returns the redeeming account and time |
| A failed signup burns no use | Unit: force the account insert to fail; `used_count` unchanged |
| FR-020a — an existing user burns no use | Unit: redeem as an existing account; `used_count` unchanged and no redemption row |
| FR-019a — redemption is rate limited | Unit: sustained attempts refuse with a message distinguishable from `invitation_unusable` |
