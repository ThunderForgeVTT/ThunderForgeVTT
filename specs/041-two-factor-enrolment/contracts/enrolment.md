# Contract: Enrolment — one flow, three authorisations

Auth in this codebase is REST, not GraphQL (`src/server/src/auth/`, merged
under `/api`), so these are HTTP routes and they inherit the existing
`rate_limit_auth_requests` layer by living under `/authentication/`
(40 requests / 60s per `{ip}:{path}`, `auth_middleware.rs:100`). No new
limiter is added.

## The authorisation, and what changes about it

Today `POST /api/authentication/2fa/setup/start` takes
`{username, password}` and **no session** (`auth/types.rs:44`). That is the
hole: a stranger holding a leaked password can mint a new secret for the
account and, in the same statement, clear the confirmed one
(`auth/two_factor.rs:91`).

Both enrolment routes now take **exactly one** authorisation, and never a
username:

| Authorisation | Sent as | Entrance | Requirement |
|---|---|---|---|
| A live session | the existing `session` cookie + CSRF header | account settings | FR-001 |
| A `setup` ticket | `{"ticket": "<uuid>"}` | first-run setup | FR-028 |
| An `enrol` ticket | `{"ticket": "<uuid>"}` | a sign-in that requires enrolment | FR-019, FR-030, FR-031 |

A ticket is a `login_two_factor_challenges` row with `purpose` `setup` or
`enrol` — single use, expiring, bound to one account, and minted by the
server only after it has itself checked something (a bootstrap code, or a
correct password). The account is read off the authorisation. **There is no
form of these routes that identifies an account by name.**

Sending both a session and a ticket is refused. One caller, one identity.

## `POST /api/authentication/2fa/setup/start`

Begins or restarts an enrolment. **Writes nothing on `users`.**

```jsonc
// Request: a ticket, or nothing at all when a session cookie is present.
{ "ticket": "018f…" }

// 200
{
  "status": "success",
  "otpauth_url": "otpauth://totp/ThunderForge:wizard?secret=GEZD…&issuer=ThunderForge",
  "secret": "GEZD GNBV GY3T QOJQ GEZD GNBV GY3T QOJQ",
  "qr": { "size": 33, "modules": ["101…1", "…"] },
  "two_factor_enabled": true,          // what is in force RIGHT NOW, not what is pending
  "expires_at": "2026-09-07T12:30:00Z"
}
```

### Rules

1. **The response is the only place the secret exists in the clear.** The
   pending secret is stored in `two_factor_enrolments`, encrypted with the
   same AES-256-GCM envelope as the live one (`src/server/src/crypto.rs`).
2. **`users.two_factor_enabled`, `two_factor_secret_encrypted` and
   `two_factor_confirmed_at` are not written** (FR-013). `two_factor_enabled`
   is *reported* so the client can say "you already have one; this will
   replace it once you confirm" rather than implying it has been removed.
3. **Restarting replaces the pending row** and nothing else
   (`ON CONFLICT (user_id) DO UPDATE`).
4. `secret` is the same base32 as the URI's `secret` parameter, **grouped in
   fours for transcription** (FR-002). The client must strip whitespace before
   comparing; the server never accepts the grouped form back, because it never
   receives a secret back at all.
5. `qr` is a **module matrix, not markup** — `size` and one string of `0`/`1`
   per row (research.md § R10). The client draws rectangles. A server-rendered
   SVG string would have to reach the page through
   `dangerouslySetInnerHTML`, on a security screen.
6. **A failure to build the QR is not a failure to enrol.** If `qr` is absent
   the client shows the typeable secret alone and enrolment completes
   normally. The image is a convenience; the secret is the credential.

## `POST /api/authentication/2fa/setup/confirm`

The only writer of `users`' two-factor columns.

```jsonc
// Request
{ "ticket": "018f…", "code": "492013" }

// 200 — and this body is the ONLY time these codes exist outside a hash
{
  "status": "success",
  "confirmed_at": "2026-09-07T12:04:11Z",
  "recovery_codes": ["4KJH-92MX-QW3T", "…nine more…"],
  "recovery_codes_notice": "Keep these somewhere other than the device you just set up. Each works once. They will not be shown again."
}
```

### Rules

1. In **one transaction**: verify the code against the pending secret, copy
   the secret onto `users`, set `two_factor_enabled` and
   `two_factor_confirmed_at`, delete every outstanding recovery code, insert
   ten new hashes, delete the pending row, write a `two_factor_events`
   `enrolled` row.
2. **A wrong code changes nothing** — the pending row survives, the same
   ticket may be retried, and no re-scan is needed (FR-001c). The ticket's
   `failed_attempts` increments; five spends it (FR-017).
3. **The codes are returned once.** There is no endpoint that can produce
   them again; only a fresh set (`recovery-codes.md`) exists as a remedy.
   FR-009 holds because there is nothing left to return, not because a rule
   forbids returning it.
4. **The session is issued here** for the `setup` and `enrol` entrances —
   after the codes are in the response body, so a person can save them before
   they go anywhere (FR-029). For the account-settings entrance the session
   already exists and is untouched.
5. **Mail is not consulted.** Notification is a best-effort call *after*
   commit whose failure changes nothing (FR-001b, FR-015). A first
   administrator on an instance with no mail server must be able to finish.
6. `code` is matched against the **pending** secret only. A code from the
   currently-live authenticator does not confirm a new one.

## `GET /api/authentication/2fa/status`

What the account holder's own security page reads. Session only.

```jsonc
{
  "enabled": true,
  "confirmed_at": "2026-09-07T12:04:11Z",
  "recovery_codes_remaining": 8,
  "recovery_codes_low": false,
  "required": true,
  "required_reason": "administrator",   // "administrator" | "instance_policy" | "administrator_required" | null
  "required_by": "quartermaster",       // present only for "administrator_required"
  "enrolment_pending": false
}
```

### Rules

1. `required_reason` exists so the page can say *why* rather than only *that*
   (FR-005, FR-023). `"administrator"` is the one with no remedy offered,
   because FR-027 says there is none.
2. This route answers for the caller only. There is no form of it that takes
   an account id; the operator's view is `requirement-policy.md`.

## What is deliberately absent

- **No `username` or `password` field, anywhere on these routes.** That form
  is the defect.
- **No "disable" side effect.** Removal is its own route with its own price
  (`removal-and-reset.md`), because FR-014 says it must not be reachable only
  as a consequence of something else.
- **No second authenticator.** One per account, per the spec's assumption;
  nothing here precludes adding more later, and `two_factor_enrolments`'
  `UNIQUE (user_id)` is the only line that would move.
