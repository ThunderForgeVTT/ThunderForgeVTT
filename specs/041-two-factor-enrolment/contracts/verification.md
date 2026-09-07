# Contract: Verification — the step guard, the attempt budget, and refusals

What changes at `POST /api/authentication/2fa/verify`
(`src/server/src/auth/two_factor.rs:219`) and in the login path that mints its
challenge (`auth/sessions.rs:416-499`). **The verifier itself does not
change**: SHA1, 6 digits, a 30-second step, a skew of one, issuer
`ThunderForge`, all as `crates/thunderforge-axum-auth-core/src/totp.rs`
already builds them.

## The step guard (FR-016)

`totp.rs` says it in its own comment: totp-rs 6.0 returns `Some(step)` "so a
caller can refuse to accept the same step twice… We only ask whether the code
matched, so the step is dropped here."

The crate gains a step-returning form; the existing `verify_totp_code` and
`verify_totp_code_at` keep their signatures and are re-expressed in terms of
it, so no caller and no parameter moves.

```rust
/// The step `code` matched at `unix_time`, or `None`.
///
/// The testable form. Same rule, same window, and the value the existing
/// boolean forms discard.
pub fn matched_step_at(
    username: &str,
    secret_base32: &str,
    code: &str,
    unix_time: u64,
) -> Result<Option<u64>, String>;
```

The server then advances a high-water mark, and the advance **is** the check:

```sql
UPDATE users SET two_factor_last_used_step = $step
 WHERE id = $user
   AND (two_factor_last_used_step IS NULL OR two_factor_last_used_step < $step)
```

### Rules

1. **Zero rows updated is the refusal.** Not a read followed by a write: the
   case that matters is two requests carrying the same six digits inside one
   30-second window, and only the conditional write serialises them.
2. The guard runs on **every** path that accepts a TOTP code — the login
   challenge, enrolment confirmation, removal, and recovery-code
   regeneration. A code spent proving one thing may not prove another.
3. **Enrolment confirmation is the one exception, and only until it
   commits**: the pending secret has no history, so the first accepted step
   initialises the mark rather than being compared to the old secret's. A new
   secret is a new sequence.
4. A **recovery code sets no mark** — it has no step, and advancing the mark
   would refuse the authenticator code the person may type next.

**Accepted consequence**: one TOTP verification per account per 30-second
step. Signing in twice inside thirty seconds means waiting for the next code.
That is the requirement, not a side effect — the second use is the replay,
and nothing can distinguish the honest one from the intercepted one.

## The attempt budget (FR-017)

`login_two_factor_challenges` gains `failed_attempts INTEGER NOT NULL
DEFAULT 0`. Five failures spend the challenge (`consumed_at` set, no
session). The person returns to the credential step and starts a new sign-in.

### Rules

1. **The budget is on the challenge, never on the account.** An account-level
   lockout hands anybody who knows a username a denial-of-service button; a
   challenge budget lets an attacker burn only challenges they created.
2. **A mistype is not a lockout.** Four wrong codes still leave the challenge
   usable, and today's e2e already asserts that a wrong code does not burn it.
   That assertion stays true.
3. The volumetric limiter (`rate_limit_auth_requests`, 40/60s per
   `{ip}:{path}`) still applies and is unchanged. The e2e bypass
   (`THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT=1`, debug builds only) turns **that**
   off and does not touch the attempt budget — the requirement's own coverage
   must not depend on a switch.

## Refusal shapes (FR-018)

One message. `"That code was not accepted."` — and an HTTP 401 — for every
one of:

- a wrong TOTP code;
- a correct TOTP code whose step has already been used;
- a wrong recovery code;
- a recovery code already spent;
- a recovery code belonging to a different account;
- an account holding no recovery codes at all;
- an account holding no second factor at all.

A separate message, and a 400, for a challenge that is expired, consumed or
unknown — because that is a state the person must act on differently (sign in
again) and it discloses nothing about the account.

**Never disclosed**: whether the account has a second factor, whether a
recovery code would have worked, how close a code was, which step matched,
how many attempts remain, or which of two supplied fields was evaluated. This
follows the rule spec 027's FR-011 set — a dead link gives one message
whatever the cause — and the existing `"Invalid credentials"` for both an
unknown username and a wrong password.

## The login path

`authenticate_password_login` (`auth/sessions.rs:416`) computes today, inline
at line 477, `global_required || two_factor_admin_required ||
two_factor_enabled` — a duplicate of `is_two_factor_required_for_user`, which
this feature collapses into the single `required(user)` of data-model.md § 5.

After a correct password:

| `two_factor_enabled` | `required(user)` | Response |
|---|---|---|
| true | any | `401 two_factor_required` + `login_two_factor_challenge_id` (purpose `verify`) — unchanged |
| false | true | `401 two_factor_enrolment_required` + `login_two_factor_challenge_id` (purpose `enrol`) — **new** |
| false | false | signed in — unchanged |

### Rules

1. **The middle row is the fix for the lockout button.** Today that account
   gets a `verify` challenge it cannot answer, because
   `verify_two_factor_for_user` returns `false` for a user with no stored
   secret. It is refused for not having done something it was never offered.
2. The two statuses are distinct on the wire so the client knows which card to
   render, and neither is returned before the password is correct — so
   neither discloses anything to somebody who does not already hold the
   password.
3. Completing an `enrol` challenge lands the person where they were going
   (FR-020): the `returnTo` the login page already carries survives the
   enrolment step.
4. The same three rows apply on the OAuth path (`auth/oauth.rs:475-505`) with
   one exception the spec states: an account that only ever signs in through
   a provider is governed by that provider's second factor, so the
   instance-wide term does not apply to it. **The administrator term does** —
   an administrator holds the instance however they sign into it.

## What is deliberately absent

- **No "remember this browser".** Out of scope by the spec, and it would be a
  fourth way past the challenge to keep correct.
- **No email or SMS fallback code.** Out of scope, weaker, and it would make
  enrolment depend on mail, which FR-001b forbids.
- **No widening of the skew** to make the step guard less noticeable. The
  window is out of scope and already reasoned about in `totp.rs`.
