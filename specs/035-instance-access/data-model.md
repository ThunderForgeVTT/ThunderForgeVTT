# Data Model: Instance Access (US1 + US2)

**Plan**: [plan.md](./plan.md) | **Research**: [research.md](./research.md)

Four tables. Conventions follow the existing schema: `Uuid` primary keys from
`Uuid::now_v7()` except where a singleton uses `id = 1`, `created_by`
provenance per Constitution Principle III, and `Timestamp` (naive UTC) to match
the surrounding auth tables rather than `Timestamptz`.

---

## `instance_access_settings` — the policy

A singleton, exactly like `auth_security_settings` (`schema.rs:42`).

| Column | Type | Notes |
|---|---|---|
| `id` | `Int4` PK | Always `1`. `ensure_instance_access_settings` inserts it if absent. |
| `access_policy` | `Text` | `"open"` \| `"invite_only"` \| `"closed"`. |
| `updated_by` | `Nullable<Uuid>` → `users` | Null only for the row the migration seeds. |
| `updated_at` | `Timestamp` | |

**FR-001** — exactly one state in effect at any moment is enforced by the table
holding exactly one row, not by convention.

**Seeded value depends on whether the instance is new** (FR-013, FR-013a,
clarified 2026-09-06). The migration branches on `SELECT EXISTS(SELECT 1 FROM users)`:

| At migration time | Seeded policy | Why |
|---|---|---|
| No users — a fresh install | `invite_only` | FR-013. Admits no stranger, while letting the operator issue a working invitation as their first act. `closed` would refuse even a valid invitation (FR-001), so a fresh instance would fail confusingly. |
| Users exist — an upgrade | `open` | FR-013a. Preserves the behaviour the instance had before the upgrade. An upgrade must not silently stop an operating instance from admitting people; closing it is an explicit operator act. |

This is the only branch in the migration, and it is why the settings row is
seeded by the migration rather than lazily by `ensure_instance_access_settings`
— a lazy insert on first read has no way to tell a fresh instance from an
upgraded one after the fact.

**Why `Text` and not a Postgres enum**: the codebase already stores
`ActorPermissionLevel` and moderation statuses as text with a parse at the
boundary, and an unparseable value degrades to the safest member rather than
failing a query. Here that means an unrecognised policy is treated as
`closed` — fail shut.

---

## `instance_invitations` — the invitation

Mirrors `world_invites` (`schema.rs:826`) minus `world_id`.

| Column | Type | Notes |
|---|---|---|
| `id` | `Uuid` PK | |
| `invite_code` | `Varchar(32)` UNIQUE | From `share_codes::generate_link_code` — v4-derived, never v7. |
| `max_uses` | `Int4` | `>= 1`. |
| `used_count` | `Int4` | Starts `0`; only ever moved by the conditional UPDATE below. |
| `expires_at` | `Nullable<Timestamp>` | Null means no expiry. |
| `note` | `Nullable<Text>` | Operator's own label ("Priya, from the forum"). Never shown to the recipient. |
| `revoked` | `Bool` | Soft flag, never a delete — a revoked link must stay distinguishable *to the operator* while being indistinguishable to everyone else. |
| `created_by` | `Uuid` → `users` | |
| `created_at` / `updated_at` | `Timestamp` | |

**Deliberately absent**: `rotated_from`. `world_invites` carries it because
spec 027 US1 rotates a leaked world link in place. US2 has no rotation
requirement — issuing a second invitation and revoking the first is the same
act at instance scale, where invitations are per-person rather than per-table.

### Derived state (not stored)

`state` is computed the way `derive_link_state` does for world invites:
`revoked` → **revoked**; `expires_at` past → **expired**;
`used_count >= max_uses` → **exhausted**; else **active**.

**Only the operator ever sees this.** To an unauthenticated redeemer all four
non-active states are one refusal (US2 scenarios 4 and 5, FR-009d's rule
restated at instance scope).

---

## `instance_invitation_redemptions` — who came in on which link

| Column | Type | Notes |
|---|---|---|
| `id` | `Uuid` PK | |
| `invitation_id` | `Uuid` → `instance_invitations` ON DELETE CASCADE | |
| `user_id` | `Uuid` → `users` ON DELETE CASCADE | UNIQUE with `invitation_id`. |
| `redeemed_at` | `Timestamp` | |
| `route` | `Text` | `"local"` \| `"oauth:<provider_key>"` — which door they came through. |

**Why this is a table and not a `redeemed_by` column.** US2 scenario 8 requires
an administrator to see which account redeemed an invitation and when. An
invitation may have `max_uses > 1`, so that is one-to-many. A single column
would either cap every invitation at one use — which the spec does not — or
silently record only the last redeemer.

---

## `instance_access_events` — the audit trail

Append-only. Covers FR-004 (policy changes) and FR-007 (refused admissions),
in one table because an operator reads them together (research §6).

| Column | Type | Notes |
|---|---|---|
| `id` | `Uuid` PK | |
| `event_type` | `Text` | `"policy_changed"` \| `"admission_refused"` \| `"invitation_redeemed"`. |
| `occurred_at` | `Timestamp` | |
| `actor_user_id` | `Nullable<Uuid>` → `users` | The administrator, for `policy_changed`. Null for a refusal — there is no account. |
| `previous_policy` / `new_policy` | `Nullable<Text>` | `policy_changed` only. |
| `attempted_route` | `Nullable<Text>` | `"local"` \| `"oauth:<provider_key>"`. |
| `policy_at_attempt` | `Nullable<Text>` | What the policy was when it refused — so a log read later still explains itself. |

**What this table must never hold**: passwords, tokens, provider access tokens,
or the submitted email address. FR-007 requires the time, the path, and the
provider — not the identity. Recording the email would turn an audit log into a
list of people who tried to join, which is exactly the data US5 exists to stop
accumulating.

**Retention**: out of scope here; US5 covers retention for the request-access
data this feature does not build.

---

## The redemption transition, and why it is one statement

SC-006 requires an N-use invitation to admit at most N accounts *under
simultaneous redemption*. Spec 027 hit and fixed this exact race
(`mutations_invites.rs:240`): read-then-write loses updates when two redeemers
see the same `used_count`.

Redemption therefore carries the whole validity predicate in the UPDATE:

```sql
UPDATE instance_invitations
   SET used_count = used_count + 1, updated_at = now()
 WHERE invite_code = $1
   AND revoked = false
   AND (expires_at IS NULL OR expires_at > now())
   AND used_count < max_uses
RETURNING id;
```

Zero rows updated means unusable, and **which** of the four reasons is never
distinguished — satisfying US2 scenarios 4 and 5 by construction rather than by
remembering to write the same error string in four branches.

The account insert, the redemption row and this UPDATE are one transaction. A
failed account creation must not burn a use.

---

## What the policy does **not** gate

Two exemptions, both from the spec's Context section, both enforced by the gate
being called only where accounts are *created by a stranger*:

1. **First-run administrator bootstrap.** `admin_setup.rs`'s path runs when no
   administrator exists. A closed default must not brick a fresh install
   (FR-011, SC-003).
2. **Sign-in by an existing account, and identity linking to one.** That is
   authentication, not signup (US1 scenarios 4 and 5). The gate sits after the
   "does this email match a user" lookup in `resolve_oauth_login` precisely so
   that a match never reaches it.
