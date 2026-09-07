# Contract: Recovery codes

Ten codes, single use, issued at confirmation, shown once, and stored the way
the admin bootstrap code is stored — Argon2, never readable again.

## Shape

`4KJH-92MX-QW3T` — twelve characters in three groups, from
`random_setup_code()`'s alphabet
(`crates/thunderforge-axum-auth-core/src/random.rs:24`): no `I`, `O`, `0` or
`1`, because these get written on paper and read back at the worst possible
moment. ~60 bits each, and ten of them.

Reusing that generator is deliberate: it is already the shape the product
asks a person to transcribe, and it is already the shape that is Argon2-hashed
rather than stored.

## Where they are issued

**Only two places, and both return them in the same body that creates them:**

1. `POST /api/authentication/2fa/setup/confirm` — enrolment (`enrolment.md`).
2. `POST /api/authentication/2fa/recovery-codes` — a fresh set.

```jsonc
// POST /api/authentication/2fa/recovery-codes
// Session required, plus proof of possession — the same price as removal.
{ "code": "492013" }            // or { "recovery_code": "4KJH-92MX-QW3T" }

// 200
{
  "status": "success",
  "recovery_codes": ["…ten…"],
  "recovery_codes_notice": "Every code from your previous set has stopped working."
}
```

### Rules

1. **Issuing deletes.** Every outstanding row for the account is deleted
   before the ten new hashes are inserted, in one transaction (FR-010). Not
   marked superseded — deleted, so "every earlier code stops working" cannot
   be undone by a query that forgets a filter.
2. **Regenerating costs possession**, exactly as removal does. A fresh set is
   a fresh way in; handing one out on the password alone would rebuild the
   hole FR-012 closes, one indirection further away.
3. **A `two_factor_events` `recovery_codes_issued` row is written**, and the
   account holder is notified best-effort (FR-015).
4. There is no `GET` that returns codes. There is no admin route that returns
   codes. The plaintext exists in exactly one response body and then does not
   exist (FR-009).

## Where they are spent

At the login challenge, in place of an authenticator code — the same route
that takes a TOTP code today (`verification.md`).

```jsonc
// POST /api/authentication/2fa/verify
{ "challenge_id": "018f…", "recovery_code": "4KJH-92MX-QW3T" }
```

### Rules

1. **One field or the other, never both.** A request carrying `code` and
   `recovery_code` is refused without evaluating either — a client that sends
   both is asking to have two chances counted as one attempt.
2. **Matching is a full scan without an early exit.** All of the account's
   unspent codes are verified even after one matches, sequentially. Constant
   work removes the timing difference between "matched the second one" and
   "matched none" (FR-018), and sequential keeps peak memory at one Argon2
   hash rather than ten.
3. **Spending is the conditional write, not a prior read**:
   `UPDATE user_recovery_codes SET used_at = now() WHERE id = $1 AND used_at IS NULL`.
   **Zero rows updated means refuse** (FR-008). Two simultaneous presentations
   of one code cannot both succeed.
4. Input is normalised before matching — case-folded, hyphens and whitespace
   removed — because a person retyping off paper will not reproduce the
   grouping.
5. A spent code writes a `two_factor_events` `recovery_code_used` row that
   records **that** a code was used and never **which** — the same rule spec
   035 wrote into `instance_access_events`.
6. Success issues a session exactly as a correct TOTP code does. A recovery
   code is a second factor, not a lesser one.
7. **A recovery code does not touch `two_factor_last_used_step`.** The step
   guard is about TOTP; a recovery code has no step, and advancing the mark
   would refuse the authenticator code a person may be about to type.

## Running low

`GET /api/authentication/2fa/status` reports
`recovery_codes_remaining` and `recovery_codes_low` (three or fewer unspent),
and the session response after a successful sign-in carries the same two
fields so the person is told at the moment it matters (FR-011) rather than
only if they visit a settings page.

The client offers a fresh set alongside the warning. It never generates one
automatically: a set the person did not ask for is a set they did not save,
and it would have silently invalidated the sheet in their drawer.

## Failure shapes

| Situation | Result |
|---|---|
| Code already spent | Refused, same message as a wrong code (FR-018) |
| Code belongs to another account | Refused, same message; never "wrong account" |
| Account has no codes at all | Refused, same message; never "this account has no recovery codes" |
| Both `code` and `recovery_code` sent | Refused before either is evaluated |
| Five failed attempts on one challenge | Challenge spent; start a new sign-in (FR-017) |
| All ten spent | Refused; the person is pointed at the operator reset path (FR-026) |
