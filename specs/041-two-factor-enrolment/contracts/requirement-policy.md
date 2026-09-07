# Contract: Who must hold a second factor

Three reasons an account must, and they are not the same kind of thing. Two
are policies an operator chooses. One is a property of a role and has no
switch — which is the distinction this contract exists to keep from
collapsing into one setting.

```text
required(user) =
      user.is_admin                                 -- FR-027. Not a policy. No column.
   OR user.two_factor_admin_required                -- FR-023. One account.
   OR instance.two_factor_required_for_all_users     -- FR-019. An operator's choice.
```

This replaces the expression inlined at `auth/sessions.rs:477` and duplicated
in `is_two_factor_required_for_user` (`auth/two_factor.rs`). The `||
two_factor_enabled` term those carry today is dropped: having a factor is not
a reason you must have one, and conflating the two is what makes the
instance-wide switch look safe.

## The administrator rule (FR-027 to FR-033)

**Nothing is added to the schema for this.** `users.is_admin` already exists;
the rule reads it. A column would be a switch, and FR-027 says there must not
be one.

| Situation | What happens | Requirement |
|---|---|---|
| First-run setup | Setup does not complete until the administrator's factor is confirmed and their codes are issued | FR-028, FR-029 |
| An existing account is granted `is_admin` | Its next sign-in gets an `enrol` challenge. No migration, no backfill | FR-030 |
| An instance upgrades into this rule | Its administrators' next sign-in gets an `enrol` challenge. Never a refusal | FR-031 |
| An account loses `is_admin` | **Nothing is written.** The factor survives | FR-032 |
| Somebody looks for a way to turn it off | There is none to find | FR-027 |

Rows two and three are the same code path as row four of
`verification.md`'s table — one rule, three situations. That is the point of
computing it: an upgraded instance needs no data migration for FR-031,
because there is no stored requirement to migrate.

**Everybody else stays optional by default** (FR-033). The instance-wide
switch and the per-account flag are unchanged in meaning; what changes is
that an account they apply to is now *taken through enrolment* rather than
refused.

## First-run setup, and the boundary with spec 040

**Spec 040 owns the setup wizard.** This contract owns one step inside it and
one condition.

Today: `admin_setup_basic` (`auth/admin_setup.rs:65`) inserts the
administrator with every two-factor column at its empty default
(`admin_setup.rs:129-144`), calls `mark_admin_setup_complete_sync`
(`admin_bootstrap.rs:446`), and issues a session cookie. And `setup_status`
computes completion as `admin_exists || setup_completed_at.is_some()`
(`admin_setup.rs:37`).

**Both halves of that have to move**, and the second is easy to miss:
deferring `setup_completed_at` alone would not defer completion, because the
administrator row exists by then.

| Step | Before | After |
|---|---|---|
| `POST /authentication/setup/basic` | creates the admin, marks setup complete, issues a session | creates the admin, **returns a `setup` ticket**, marks nothing |
| `POST /authentication/2fa/setup/start` (with that ticket) | — | shows the QR and the typeable secret |
| `POST /authentication/2fa/setup/confirm` | — | confirms the factor, **issues the recovery codes, marks setup complete, and issues the session** — in that order |
| `GET /authentication/setup/status` | `admin_exists \|\| setup_completed_at` | **an administrator exists *and* holds a confirmed second factor** |

### Rules

1. **The codes are in the confirm response, before setup ends** (FR-029). A
   fresh instance has no mail, no second administrator and nobody to ask;
   this is the moment they matter most and the only moment they are free.
2. **No step of this consults mail** (FR-001b, SC-010). Notification is
   best-effort after commit and cannot fail the request.
3. The same OAuth setup route (`admin_setup_oauth_callback`,
   `auth/admin_setup.rs:277`) redirects to the enrolment step rather than to
   `return_to`. An administrator provisioned through a provider is still an
   administrator.
4. A `setup` ticket has a longer life than a login challenge's ten minutes —
   this step involves finding a phone, installing an app and writing ten codes
   down. It stays single-use and account-bound.
5. If the process dies between the admin row and the confirmation, setup reads
   as **incomplete** and a fresh `setup` ticket can be issued for the existing
   administrator. A half-finished setup must not be a locked instance.

## The operator's view (FR-021)

One GraphQL field, on `AdminQuery` beside `authSecuritySettings`
(`src/server/src/graphql/queries/admin.rs:72`) — where the policy switch it
explains already lives.

```graphql
type TwoFactorCoverage {
  "Accounts holding a confirmed second factor."
  enrolled: Int!
  "Accounts without one."
  notEnrolled: Int!
  "Of those, how many are required to have one and have not."
  requiredNotEnrolled: Int!
}

extend type AdminQuery {
  twoFactorCoverage: TwoFactorCoverage!
}
```

### Rules

1. **Counts, never a list of accounts.** SC-007 asks an operator to tell "at a
   glance, how much of their instance has a second factor". A roster of who
   has not enrolled is a target list, and it is not what the requirement asks
   for.
2. `requiredNotEnrolled` is the number that matters before turning the
   instance-wide switch on: it is how many people will meet an enrolment step
   at their next sign-in. **Nobody is locked out either way** — that is
   FR-019 — but an operator should know what they are about to ask of people.
3. Rendered in `apps/web/src/pages/admin/components/SecurityPanel.tsx`, next
   to the switch, because a switch whose consequence is on another screen is a
   switch nobody reads the consequence of.

## Requiring it of one account (FR-023)

`POST /api/authentication/admin/users/{user_id}/2fa/required` exists
(`auth/two_factor.rs:344`) and is reachable only by hand-rolled REST. It gains
a surface and one column.

### Rules

1. Setting it records `two_factor_required_by = <the acting administrator>`
   and writes a `two_factor_events` `requirement_set` row. Clearing it nulls
   the column and writes `requirement_cleared`. FR-023 asks to see that it is
   required **and by whom**, which a boolean cannot answer.
2. The account is taken through enrolment at its next sign-in and **no other
   account is affected** — the instance-wide switch is the other control and
   this one must never reach past its subject.
3. Turning any requirement off leaves every existing second factor in force
   (FR-022). Requirements and factors are different things; the code path that
   clears one must not touch the other, and a test says so.
4. There is no GraphQL equivalent. The instance-wide policy is settable over
   both REST and GraphQL today, which is a duplication this feature does not
   extend.

## What is deliberately absent

- **No per-world or per-role requirement.** `is_admin` is the only role this
  product has; inventing a second axis here would outrun the data model.
- **No grace period.** "Required from next Tuesday" is a scheduler, a
  notification and a second definition of "required", for a rule whose whole
  cost is one enrolment step at one sign-in.
- **No exemption list.** An exemption is a switch, and FR-027 is about not
  having one.
