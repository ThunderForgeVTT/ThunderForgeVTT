# Phase 1 Data Model: Two-Factor Enrolment, Recovery and Removal

Five pieces of state. The line between the first two is the whole security
fix: **a pending secret and a live secret must not share a column**, because
the moment they do, starting an enrolment can end one.

## 1. `users` — the live second factor, plus one guard

Exists today (`src/server/src/schema.rs:561`) with four two-factor columns:
`two_factor_enabled`, `two_factor_secret_encrypted`,
`two_factor_confirmed_at`, `two_factor_admin_required`.

**Their meaning does not change. What changes is who may write them.**
`two_factor_setup_start` writes three of them today
(`src/server/src/auth/two_factor.rs:91`) on the password alone. After this
feature, **only confirmation writes them**, in one transaction (FR-013).

| Column | Change | Why |
|---|---|---|
| `two_factor_enabled` | unchanged; **written only at confirmation** | FR-013. Starting an enrolment must not clear it. |
| `two_factor_secret_encrypted` | unchanged; **written only at confirmation** | FR-004. An abandoned enrolment leaves the account exactly as it was, and that includes the working secret. |
| `two_factor_confirmed_at` | unchanged; surfaced to the account holder | FR-005: "see it is on and when it was confirmed" has no other source. |
| `two_factor_admin_required` | unchanged; gains a **surface** | FR-023. The column and its endpoint exist and are reachable only by hand-rolled REST. |
| `two_factor_last_used_step` | **new**, `BIGINT NULL` | FR-016. The highest TOTP step this account has ever had accepted. See § 6. |
| `two_factor_required_by` | **new**, `UUID NULL REFERENCES users(id) ON DELETE SET NULL` | FR-023: "see that it is required **and who required it**". `two_factor_admin_required` is a boolean and cannot answer the second half. |

**No `is_admin`-derived column is added, and that is a requirement, not an
omission.** FR-027 says the administrator rule is a property of the role and
must not be switchable off. A column is a switch. The rule is computed:
`is_admin` ⇒ required, with nothing to `UPDATE`. See § 5.

## 2. `two_factor_enrolments` — the pending secret, in its own row

The new table the feature turns on. One in-progress enrolment per account.

```sql
id                  UUID PRIMARY KEY
user_id             UUID NOT NULL UNIQUE REFERENCES users(id) ON DELETE CASCADE
secret_encrypted    TEXT NOT NULL      -- same AES-256-GCM envelope as users.two_factor_secret_encrypted
created_at          TIMESTAMP NOT NULL
expires_at          TIMESTAMP NOT NULL -- created_at + 30 minutes
```

- **`UNIQUE (user_id)`** — one pending enrolment at a time. Starting again
  replaces the row (`ON CONFLICT (user_id) DO UPDATE`), which is what "I
  closed the tab and started over" means, and costs nothing because the row
  holds no confirmed state.
- **Encrypted with the same envelope** as the live secret
  (`src/server/src/crypto.rs`, `v1.<nonce>.<ciphertext>`, AES-256-GCM keyed
  from the instance secret). A pending secret is exactly as sensitive as a
  confirmed one; it just is not in force yet.
- **Expires.** Thirty minutes is generous for "scan this and type six
  digits" and short enough that an abandoned row is not a secret sitting
  around indefinitely.
- **Deleted on confirmation**, in the same transaction that writes `users`.
  The pending row and the live secret are never both authoritative.
- **No `created_by`/`updated_by`**: the row is self-owned by `user_id`, the
  same shape `user_sessions` uses, and there is no second actor who can
  create one — R2 removed the path by which somebody could start an enrolment
  for another account.

**A wrong confirmation code does not touch this row** (FR-001c). That is why
"try again without re-scanning" needs no additional state.

## 3. `user_recovery_codes` — ten credentials, verifiable and unreadable

```sql
id           UUID PRIMARY KEY
user_id      UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE
code_hash    TEXT NOT NULL      -- Argon2id PHC string, as password_hash is
used_at      TIMESTAMP NULL
created_at   TIMESTAMP NOT NULL
```

Index: `(user_id) WHERE used_at IS NULL` — the only query is "this account's
unspent codes", and it is the one on the recovery path.

- **Argon2, not the raw value and not encryption.** The precedent is the admin
  bootstrap code, hashed with the very same `hash_password`
  (`auth/admin_bootstrap.rs:8`), not the raw-stored share codes. FR-009 —
  "stored so that the instance cannot display them again" — is then true
  because there is nothing to display, rather than because a rule says not to.
- **No `code_prefix` or lookup hint column.** It would make verification one
  hash instead of ten, and it would also make the codes partially readable,
  which is the property being removed.
- **`used_at` is the single-use guard**, not a flag read beforehand:
  `UPDATE … SET used_at = now() WHERE id = $1 AND used_at IS NULL`, and zero
  rows updated means refuse (FR-008). Two simultaneous presentations of one
  code cannot both win.
- **Regeneration deletes.** Issuing a fresh set removes every outstanding row
  for the account first (FR-010), so "every earlier code stops working" is
  true even where a later query forgets a filter.
- **"Running low" is `count(*) WHERE used_at IS NULL <= 3`** (FR-011).

**Verification is a full scan without an early exit** — all ten hashes are
verified even after one matches, sequentially. Constant work removes the
timing difference between "matched the second one" and "matched none"
(FR-018), and sequential keeps peak memory at one Argon2 hash rather than ten.

## 4. `two_factor_events` — what happened to a second factor

Modelled on `instance_access_events` (spec 035,
`migrations/2026-09-06-000000-0000_instance_access/up.sql:85`), including its
stated rule: the record describes the act, never the person.

```sql
id              UUID PRIMARY KEY
occurred_at     TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
subject_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE
actor_user_id   UUID NULL REFERENCES users(id) ON DELETE SET NULL
event_type      TEXT NOT NULL CHECK (event_type IN (
                  'enrolled', 'removed', 'recovery_code_used',
                  'recovery_codes_issued', 'reset_by_operator',
                  'requirement_set', 'requirement_cleared'))
```

- **`subject_user_id` and `actor_user_id` are separate**, and that separation
  is the feature: FR-025 asks who did it *and* for whom, which one column
  cannot say. For a self-service enrolment they are equal; for an operator
  reset they are not, and that is the row that matters.
- **`actor_user_id` is nullable and `ON DELETE SET NULL`** — an operator who
  later leaves must not take the record of their reset with them.
- **No IP address, no user agent, no email, no code and no fragment of one.**
  Same rule spec 035 wrote into its migration, and for the same reason. A
  `recovery_code_used` row says a code was used, never which.
- **Written in the transaction it describes.** A removal that is not recorded
  did not happen.
- **Provenance**: `actor_user_id` *is* the `created_by` this convention asks
  for; rows are append-only and never updated, so there is no `updated_by` to
  carry.

**Notification is not a column.** FR-015 and FR-025 require telling the
account holder by a route other than the acting session; there is no mail
subsystem (spec 040 owns building one), and FR-001b forbids enrolment from
depending on one. So the row is the durable half, a best-effort call to the
notification seam is the other, and its failure changes no outcome. Where
nothing can be sent, the person still sees these events in their own security
settings.

## 5. Who must hold a second factor — computed, never stored

```text
required(user) =
      user.is_admin                                  -- FR-027, not a policy
   OR user.two_factor_admin_required                 -- FR-023, one account
   OR instance.two_factor_required_for_all_users      -- FR-019, an operator's choice
```

This replaces the expression inlined at `auth/sessions.rs:477`, which today
reads `global_required || two_factor_admin_required || two_factor_enabled`
and is duplicated in `is_two_factor_required_for_user`
(`auth/two_factor.rs`). Two changes:

1. **`is_admin` joins it.** FR-027. No column, no switch, nothing to turn off.
2. **`two_factor_enabled` leaves it.** Whether an account *has* a factor is
   not a reason it *must* — that term is what makes "required" and "enrolled"
   the same word, and it is why the instance-wide switch currently reads as
   safe. Enrolment is still enforced for anybody who has one; it is enforced
   because they have one, on a separate test.

And the outcome when `required(user)` is true:

| `required(user)` | `two_factor_enabled` | Outcome after a correct password |
|---|---|---|
| false | false | signed in |
| any | true | `verify` challenge (today's behaviour) |
| **true** | **false** | **`enrol` challenge** — FR-019, FR-030, FR-031. Never a refusal. |

That third row is the entire fix for "the instance-wide switch is a lockout
button". It is also, unchanged, the answer for an account newly granted
`is_admin` (FR-030) and for an instance upgraded into the administrator rule
(FR-031) — one rule, three situations, no migration that touches accounts.

**Losing `is_admin` writes nothing** (FR-032). The factor survives because
nothing removes it, which is the only way to be certain.

**An account that only ever signs in through a provider** is out of scope by
the spec's own assumption. `auth/oauth.rs:475` calls
`is_two_factor_required_for_user` on that path; the instance-wide term does
not apply there, and the administrator term does — an administrator holds the
instance however they sign into it.

## 6. `login_two_factor_challenges` — the ticket, given a job and a budget

Exists today (`schema.rs:236`) as
`id, user_id, expires_at, consumed_at, created_at`, minted at
`auth/sessions.rs:480` and traded for a session at `two_factor.rs:219`.

| Column | Change | Why |
|---|---|---|
| `purpose` | **new**, `TEXT NOT NULL DEFAULT 'verify'` CHECK IN (`'verify'`, `'enrol'`, `'setup'`) | FR-001a. One flow, three authorisations. `setup` is the first-run ticket (FR-028). |
| `failed_attempts` | **new**, `INTEGER NOT NULL DEFAULT 0` | FR-017. Five wrong codes spend the challenge; the person starts a new sign-in. Counting here and not on the account means nobody can lock somebody else out by guessing at them. |

`consumed_at` and the 10-minute `expires_at` keep their meanings exactly. A
`setup` ticket gets a longer life — enrolling during first-run setup means
finding a phone, installing an app, and writing ten codes down.

### The step high-water mark (FR-016)

`users.two_factor_last_used_step`, advanced by the write that accepts a code:

```sql
UPDATE users SET two_factor_last_used_step = $step
 WHERE id = $user
   AND (two_factor_last_used_step IS NULL OR two_factor_last_used_step < $step)
```

**Zero rows updated is the refusal.** Not a read followed by a write — the
interesting case is two requests carrying the same six digits inside one
30-second window, and a read-then-write loses exactly that one.

**Accepted consequence**: one TOTP verification per account per 30-second
step. Signing in twice inside thirty seconds means waiting for the next code.
The second use *is* the replay FR-016 forbids, and nothing can tell the
honest one from the intercepted one.

## State transitions

### A second factor

```text
none ──(start enrolment)──> pending ──(correct code)──> confirmed
  ▲                            │                            │
  │                            ├──(wrong code)──> pending   │  (FR-001c: unchanged)
  │                            ├──(abandoned)───> none      │  (FR-004: users untouched)
  │                            └──(30 min)──────> none      │
  │                                                          │
  ├──(password + possession, deliberately)───────────────────┤  (FR-012, FR-014)
  └──(operator reset, recorded and notified)─────────────────┘  (FR-024, FR-025)

confirmed ──(start a NEW enrolment)──> confirmed + pending
                                        └─(confirmed)─> confirmed (new secret)
```

The last two lines are FR-013. A confirmed factor and a pending enrolment
coexist; the confirmed one stays in force throughout, and is replaced only by
a confirmation. **There is no arrow from `confirmed` back to `none` that does
not cost possession or an operator.**

### A recovery code

```text
issued ──(presented and matched)──> spent      [terminal]
   │
   └──(a new set is issued)──────> deleted     [terminal, FR-010]
```

### A login challenge

```text
issued ──(correct code)─────────> consumed ──> session
   │
   ├──(recovery code matched)───> consumed ──> session   (FR-007)
   ├──(5 wrong codes)───────────> consumed, no session   (FR-017)
   └──(10 minutes)──────────────> expired
```

An `enrol` challenge is consumed by the enrolment confirming, which is the
same arrow with a different destination — the session is issued after the
codes are shown, not before (FR-029: they must be saveable *before* setup
ends).

## Entity relationships

```text
User ──0:1──> TwoFactorEnrolment        (pending; UNIQUE user_id)
User ──0:N──> RecoveryCode              (10 unspent at issue; whole set replaced)
User ──0:N──> LoginTwoFactorChallenge   (purpose: verify | enrol | setup)
User ──0:N──> TwoFactorEvent            (as subject)
User ──0:N──> TwoFactorEvent            (as actor — an operator's resets)
Instance ──1:1──> AuthSecuritySettings  (two_factor_required_for_all_users; id = 1)
```

## Validation rules, traced to requirements

| Rule | Requirement |
|---|---|
| Starting an enrolment writes no column on `users` | FR-004, FR-013 |
| Enrolment requires a session, an `enrol` ticket or a `setup` ticket — never a bare password | FR-001, and the hole `two_factor.rs:91` opens |
| A wrong confirmation code leaves the pending row intact | FR-001c |
| Confirmation writes `users`, issues ten codes and deletes the pending row, in one transaction | FR-003, FR-006 |
| Ten codes are returned exactly once and stored only as Argon2 hashes | FR-006, FR-009 |
| A recovery code is spent by a conditional `UPDATE`; zero rows means refuse | FR-008 |
| Issuing a set deletes every outstanding code for the account | FR-010 |
| Three or fewer unspent codes is reported at sign-in | FR-011 |
| Removal requires the password **and** a current code or a recovery code | FR-012, FR-014 |
| A matched step ≤ the stored high-water mark is refused | FR-016 |
| Five failed codes spend the challenge; the account is never locked | FR-017 |
| A refusal names neither the cause nor whether the account has a factor | FR-018 |
| `required(user)` is true for every `is_admin`, with no column to unset | FR-027, FR-033 |
| A required account with no factor gets an `enrol` challenge, never a refusal | FR-019, FR-030, FR-031 |
| Clearing `is_admin` writes no two-factor column | FR-032 |
| Setup is incomplete until an administrator holds a confirmed factor | FR-028 |
| Enrolment completes with no mail configured | FR-001b, FR-029 |
| Every enrolment, removal, reset and recovery use writes a `two_factor_events` row | FR-015, FR-025 |
| An operator reset is a defined endpoint, never a database edit | FR-024 |
