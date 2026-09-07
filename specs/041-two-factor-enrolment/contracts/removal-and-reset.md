# Contract: Turning it off, and getting back when everything is lost

Two routes that look similar and are not: one is a person removing their own
second factor and paying what it cost to add, the other is an operator
restoring access to somebody who cannot.

## `POST /api/authentication/2fa/disable`

The deliberate way off (FR-014). It does not exist today — which is the
defect, because `setup/start`'s side effect has been doing the job instead.

```jsonc
// Session required.
{ "password": "…", "code": "492013" }        // or "recovery_code": "4KJH-92MX-QW3T"

// 200
{ "status": "success", "message": "Two-factor authentication is off for this account." }
```

### Rules

1. **Password *and* possession** (FR-012). Either a current TOTP code or an
   unspent recovery code satisfies possession; the password alone never does.
   This is exactly the price of adding one, which is the point.
2. A supplied TOTP code passes the step guard (`verification.md`) like any
   other, and a supplied recovery code is spent like any other.
3. In one transaction: clear `two_factor_enabled`,
   `two_factor_secret_encrypted`, `two_factor_confirmed_at` and
   `two_factor_last_used_step`; delete every recovery code; delete any pending
   enrolment; write a `two_factor_events` `removed` row.
4. **Refused when `required(user)` is true** — an administrator (FR-027), an
   account under the instance-wide policy (FR-019), or one an administrator
   required it of (FR-023). The refusal says which, because unlike a
   verification refusal there is no attacker to keep in the dark: the caller
   has already proved both factors. An administrator who wants off gives up
   `is_admin` first, and that is the honest ordering.
5. The account holder is notified through a route other than the acting
   session (FR-015), best-effort, after commit.
6. Existing sessions are **not** revoked. Session lifetime is spec 036's
   business and this feature does not touch it.

## `POST /api/authentication/admin/users/{user_id}/2fa/reset`

The defined path back for somebody who has lost the authenticator *and* the
recovery codes (FR-024). Administrator only, via `verify_admin_request`
(`auth/admin_setup.rs:394`).

```jsonc
// 200
{ "status": "success", "message": "The second factor for that account has been reset." }
```

### Rules

1. **It does not sign anybody in and it issues nothing.** It clears the
   account's second factor and its recovery codes, so the next sign-in with
   the correct password is met by *enrolment* — the `enrol` challenge of
   `verification.md`, because `required(user)` is still whatever it was.
   An operator never handles a credential belonging to somebody else.
2. **It is recorded, with both parties** (FR-025): a `two_factor_events`
   `reset_by_operator` row whose `actor_user_id` is the operator and whose
   `subject_user_id` is the account. One column could not say both, which is
   why there are two.
3. The account holder is notified best-effort, and sees the event in their own
   security settings regardless — which matters on an instance with no mail.
4. **This replaces the database edit**, and the quickstart says so in as many
   words. If an operator still reaches for `psql`, this route is missing
   something.
5. An administrator may reset **their own** factor through this route; it is
   the same clearing, recorded the same way, with `actor_user_id ==
   subject_user_id`. It is not a way around FR-012 — it removes the factor
   without removing the *requirement*, so the very next sign-in enrols again.

## The dead end this removes (FR-026)

When a challenge cannot be answered — no authenticator, no codes left — the
challenge screen names who can help, from the realm manifest's
`support_email`. Where the instance has no administrator able to act, it says
so plainly rather than offering a contact nobody reads.

This is a **product surface, not a route**: the text is rendered from
configuration that already exists, and spec 040 owns keeping that
configuration real. An instance whose `support_email` is still
`stewards@thunderforge.local` shows the same dead end with better manners,
which is spec 040's problem and is named here so neither feature assumes the
other solved it.

## Failure shapes

| Situation | Result |
|---|---|
| `disable` with a correct password and no code | 401, and the message says possession is required |
| `disable` with a code and a wrong password | 401, one message (FR-018) |
| `disable` on an account with no second factor | 200, idempotent — there is nothing to disclose and nothing to do |
| `disable` on an account where `required(user)` | 409, naming which requirement applies |
| `reset` by a non-administrator | 403, as every admin route already answers |
| `reset` on an unknown user id | 404, as `set_admin_user_two_factor_required` already answers |
| `reset` on an account with no second factor | 200, idempotent, and still recorded |
