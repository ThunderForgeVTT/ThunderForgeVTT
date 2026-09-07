# Contract: The Instance Access Policy and the Admission Gate

**Plan**: [../plan.md](../plan.md) | **Data model**: [../data-model.md](../data-model.md)

Covers US1. The invitation surface is in
[instance-invitations.md](./instance-invitations.md).

---

## The gate

One function, called by **every** path that can cause a `users` row to exist
for a person who did not already have one.

```rust
/// The instance's answer to "may this request create an account?"
///
/// FR-005: every account-creating path calls this, and there are exactly two —
/// `auth/sessions.rs`'s local register and `auth/oauth.rs`'s
/// `resolve_oauth_login`. A path that creates an account without calling this
/// is the defect this feature exists to prevent, not an oversight to fix later.
pub(crate) async fn ensure_admission_allowed(
    state: &AppState,
    route: AdmissionRoute,
    invitation: Option<&str>,
) -> Result<Admission, AdmissionRefused>;
```

`ensure_registration_allowed` (`auth/registration.rs:18`) becomes this. It keeps
its existing first-run check as the first branch, so the bootstrap exemption
stays where it already is and is not re-derived.

| Policy | No invitation presented | Valid invitation presented |
|---|---|---|
| `open` | admit | admit, and consume the use |
| `invite_only` | refuse | admit, and consume the use |
| `closed` | refuse | **refuse** — a closed instance admits nobody |

`closed` refusing a valid invitation is deliberate and is US2 scenario 6's
counterpart: the policy switch does not invalidate an issued invitation
(revocation, expiry and use count remain its own), but while the door is shut
the invitation cannot open it. Reopening to `invite_only` makes it work again.

### What the gate must not touch

- **Existing accounts.** The OAuth call site is placed *after* the
  "does this verified email match a user" lookup, so a match never reaches the
  gate (US1 scenarios 4 and 5).
- **First-run bootstrap** (FR-011, SC-003).
- **Sign-in, session refresh, and 2FA** — none create accounts.

---

## Refusal shapes

### Local registration

Existing `auth_session_error` shape, existing status code for a blocked
registration:

```
409 Conflict
{ "error": "registration_blocked",
  "message": "This instance is not accepting new accounts" }
```

**FR-008 / US1 scenario 2**: the response must not reveal whether the submitted
identifier already belongs to a user. The gate runs **before** the
username/email uniqueness probes in `sessions.rs`, so a closed instance returns
this identical body whether or not the address is taken — the probes are never
reached.

### OAuth callback

A browser redirect, not JSON (research §5). The existing callback error path
carries a reason code:

```
302 → /login?error=instance_closed
```

No session cookie is set, no `users` row is written, and no identity link is
created. The front end renders "This instance is not accepting new accounts."

---

## Public status (FR-003)

`GET /api/authentication/setup/status` — already unauthenticated, already read
by `App.tsx` on every page load. Two fields added:

```jsonc
{
  // ...existing fields: setupCompleted, providers[]...
  "accessPolicy": "open" | "invite_only" | "closed",
  "acceptingAccessRequests": false   // always false until US3 ships
}
```

**What this must not expose** (FR-003): no user count, no invitation codes, no
invitation existence, no operator identity. The policy alone, so the sign-in
surface can offer only routes that will work.

**What the surface must then say** (FR-003a): on `invite_only` or `closed`, no
registration affordance, *and* a line stating the instance is invite-only or
not accepting new accounts. Saying so is required rather than optional — a page
that silently omits sign-up is indistinguishable from a broken one, and it
leaves someone holding an unclicked invitation with no way to tell the instance
is working as intended.

**`acceptingAccessRequests` ships as a constant `false`.** It is in the contract
now so the front end's shape does not change when US3 lands.

---

## Administrative surface

GraphQL, admin-only, beside the existing `authSecuritySettings` query
(`graphql/queries/admin.rs:72`).

```graphql
type Query {
  instanceAccessSettings: InstanceAccessSettings!   # admin only
  instanceAccessEvents(limit: Int, after: UUID): [InstanceAccessEvent!]!
}

type Mutation {
  setInstanceAccessPolicy(policy: InstanceAccessPolicy!): InstanceAccessSettings!
}

enum InstanceAccessPolicy { OPEN, INVITE_ONLY, CLOSED }

type InstanceAccessSettings {
  policy: InstanceAccessPolicy!
  updatedAt: String!
  updatedBy: UUID
}

type InstanceAccessEvent {
  id: UUID!
  eventType: String!          # policy_changed | admission_refused | invitation_redeemed
  occurredAt: String!
  actorUserId: UUID
  previousPolicy: InstanceAccessPolicy
  newPolicy: InstanceAccessPolicy
  attemptedRoute: String      # "local" | "oauth:<provider_key>"
  policyAtAttempt: InstanceAccessPolicy
}
```

`setInstanceAccessPolicy` writes a `policy_changed` event in the same
transaction as the settings update (FR-004) — an audit row that can be lost
independently of the change it records is not an audit row.

**FR-002 — no restart.** The gate reads the settings row per admission attempt.
That is one indexed single-row read on a path that already does several writes,
and it is what makes SC-004's three minutes possible.

---

## Test contract

| Requirement | Proven by |
|---|---|
| FR-005 — every path gated | A test asserting the gate is called on both routes, plus an e2e attempting both against a closed instance |
| FR-006 — OAuth refused when closed | e2e: closed instance + configured provider + unmatched verified email → no account, no session |
| FR-008 — refusal reveals nothing | Unit: closed instance returns byte-identical bodies for a taken and an untaken email |
| FR-011 / SC-003 — bootstrap exempt | Unit: no admin exists + policy closed → bootstrap succeeds |
| US1 scenarios 4–5 — existing users unaffected | e2e: sign in and reach a world throughout a closed instance |
| FR-003a — the surface says why | e2e: on invite-only, no sign-up affordance and the stated reason is visible |
| FR-013 / FR-013a — seeded default | Migration tests: fresh DB → `invite_only`; DB with a user → `open` |
| FR-004 / FR-007 — audit | Unit: a policy change and a refused attempt each write exactly one event with the stated fields and no email |

**Why the OAuth cases must be e2e**: the refusal is a redirect produced
mid-callback after a provider handshake. A unit test can prove
`resolve_oauth_login` returns the refusal; only a browser can prove the person
lands on a page that says so and holds no session.
