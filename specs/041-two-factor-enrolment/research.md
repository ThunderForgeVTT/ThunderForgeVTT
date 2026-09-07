# Phase 0 Research: Two-Factor Enrolment, Recovery and Removal

Eleven decisions. Each records what the codebase already does, what was
chosen, and what was rejected — because in this feature almost every
question has a plausible wrong answer that looks like less work, and three
of them are the defects the spec exists to close.

The cryptography is not among them. `crates/thunderforge-axum-auth-core/src/totp.rs`
is finished and correct — SHA1, 6 digits, a 30-second step, a skew of one,
issuer `ThunderForge`, and every rule taking an explicit Unix timestamp so
the window is testable. Nothing below changes an algorithm, a digit count, a
period or the skew. What changes is what the server does with the answer.

## R1 — Where a pending secret lives (the FR-013 fix)

**What is there today**: `src/server/src/auth/two_factor.rs:91` — inside
`two_factor_setup_start`, one `diesel::update` on `users` writes the freshly
generated secret **and** sets `two_factor_enabled = false` and
`two_factor_confirmed_at = NULL`. The handler is unauthenticated; the
password is the only thing it checks. So a stranger holding a leaked password
calls `setup/start`, never confirms, and the account's confirmed second
factor stops being required. `apps/web/e2e/two-factor.spec.ts`'s
`test.fail()` case is exactly this.

**Decision**: a pending enrolment is **its own row in its own table**,
`two_factor_enrolments`. `users.two_factor_secret_encrypted`,
`two_factor_enabled` and `two_factor_confirmed_at` are written **only** at
confirmation, in one transaction, and never by starting. Beginning an
enrolment becomes a read-only act as far as the live factor is concerned.

**Rationale**: FR-004 ("an abandoned enrolment leaves the account exactly as
it was"), FR-013 ("starting must not disable, clear or weaken") and FR-001c
("a mistyped code must not discard the enrolment or require re-scanning") are
three statements of one property: *the pending secret and the live secret are
different things and must not share a column*. Once they are separate rows,
all three fall out of the schema instead of being enforced by remembering to.
It also makes the confirm step idempotent to retry — the pending row survives
a wrong code, so FR-001c needs no extra state.

This is what makes `apps/web/e2e/two-factor.spec.ts`'s final case
("a fresh enrolment request must not silently strip the confirmed factor")
pass. That test asserts the correct behaviour today and is marked
`test.fail()`; when this lands, Playwright reports "expected to fail but
passed" and the `test.fail()` line is deleted. **The fix is in the product,
not in the test.**

**Alternatives rejected**:

- *Keep one column and add a `two_factor_pending` boolean.* The confirmed
  secret is still overwritten by `setup/start`, so re-enrolling and
  abandoning still destroys the working authenticator — the account is
  challenged for a secret nobody holds. Strictly worse than the defect.
- *Refuse `setup/start` while a factor is confirmed.* Closes the hole and
  breaks re-enrolment onto a new phone, which is the ordinary case. FR-013
  says the old one stays in force *until a replacement is confirmed*, not
  that a replacement is forbidden.
- *Keep the pending secret in the session / in memory.* Enrolment is entered
  during first-run setup and at a login challenge, where there is no session
  yet (R3), and an in-memory secret dies with the process mid-enrolment.

## R2 — Starting enrolment stops being unauthenticated

**What is there today**: `POST /api/authentication/2fa/setup/start` and
`/setup/confirm` take `{username, password}` and no session
(`src/server/src/auth/types.rs:44-62`). The e2e file's own comment names the
consequence: "`setup/start` is unauthenticated, so the password is the *only*
thing standing between a stranger and resetting an account's second factor."

**Decision**: enrolment requires one of three **enrolment authorisations**
(R3), all of which are stronger than a bare password:

1. a live session (account settings — FR-001);
2. a first-run setup ticket (FR-028);
3. a login-challenge ticket issued after a correct password *when the account
   is required to enrol and has not* (FR-019, FR-030, FR-031).

The `username` + `password` body form is removed. The identity comes from the
authorisation, so enrolment can no longer be started *for* somebody.

**Rationale**: FR-001 says "from their own account settings", which means a
session. The other two entrances exist precisely because there is no session
yet, and each is a credential the server minted for one account after
checking a password — which is what the current handler pretends the raw
password is, without the single use, the expiry or the binding.

**Alternatives rejected**:

- *Keep the password form and require a session as well.* Two credentials for
  one act, and the password adds nothing a session has not already proved.
  Also breaks entrances 2 and 3, where no password is in hand any more.
- *Keep the unauthenticated form for backwards compatibility.* The only
  caller is the e2e file, which is being rewritten in this feature anyway.
  Nothing in `apps/web/src` calls it — that is the whole point of the spec.

## R3 — One flow, three authorisations (FR-001a)

**What is there today**: `login_two_factor_challenges` (`schema.rs:236`) is
`id, user_id, expires_at, consumed_at, created_at` — a single-use, expiring,
account-bound ticket minted at `auth/sessions.rs:480` after a correct
password. `two_factor_verify` (`two_factor.rs:219`) trades it plus a code for
a session cookie. It already is exactly the credential entrances 2 and 3
need; it just only knows one job.

**Decision**: give it a `purpose` — `verify` (what it does today) or `enrol`
(new) — and let the enrolment endpoints accept an `enrol` ticket in place of
a session. First-run setup gets the same row with purpose `enrol`, issued by
`admin_setup_basic` instead of the session cookie it issues today (R8).

So the flow is **one set of endpoints**; what differs between the three
entrances is only which credential opened them and where the client goes
afterwards. FR-001a's "identical in steps and wording" becomes a property of
having one implementation, not a promise to keep three in sync.

**Rationale**: three entrances built three times is how the second one ends
up subtly worse than the first, which the spec's own checklist says in as
many words. The ticket already exists, already expires, already is single-use
and already is bound to one account — the alternative designs all reinvent
some subset of that.

**Alternatives rejected**:

- *Issue a real session flagged "enrolment only".* Every other route in the
  product then has to remember to refuse it. One forgotten route is a
  full-privilege session for an account that has not satisfied the
  requirement. The ticket is refused by everything by default because
  nothing else accepts it.
- *A separate `two_factor_enrolment_tickets` table.* Same columns, same
  lifetime, same single use, in a second place — and then two expiry sweeps.
- *Reuse the admin bootstrap code for entrance 2.* It is instance-scoped and
  consumed by `admin_setup_basic` (`admin_bootstrap.rs:446`); it says "this
  person may create the first admin", not "this account may enrol".

**Checked, and fine as it stands**: the challenge is identified by its own
`Uuid::now_v7()` (`two_factor.rs`, `create_login_two_factor_challenge`), and a
v7 UUID is time-derived. ADR-049's "never v7" rule applies to codes made by
**truncating** a UUID — spec 005's collision came from taking leading hex
characters, which on a v7 value are mostly a millisecond timestamp
(`graphql/share_codes.rs:12-24`). A whole v7 UUID keeps its ~74 random bits, so
the ticket is not guessable and this feature changes nothing about it. Noted
because "v7 appears in an auth path" is the kind of thing that looks like a
finding until somebody reads the ADR.

## R4 — Binding the matched TOTP step (FR-016)

**What is there today**: `verify_totp_code` in `totp.rs` calls
`totp.check_current(code)` and its own comment explains the value it then
throws away: totp-rs 6.0 returns `Some(step)` "so a caller can refuse to
accept the same step twice… We only ask whether the code matched, so the step
is dropped here." Nothing acts on it. The skew of one means six digits stay
usable for roughly 30–90 seconds, against a *fresh* challenge. The consumed
challenge protects one sign-in; it does not protect the code.

**Decision**: the auth crate gains a step-returning form — the existing
`verify_totp_code` / `verify_totp_code_at` keep their signatures and are
re-expressed in terms of it, so nothing about the verifier is redesigned —
and the server stores the highest step it has accepted for an account in a
new `users.two_factor_last_used_step`. A code whose matched step is **less
than or equal to** the stored one is refused.

The write is the guard, not a separate check: a conditional
`UPDATE users SET two_factor_last_used_step = $step WHERE id = $id AND
(two_factor_last_used_step IS NULL OR two_factor_last_used_step < $step)`,
where **zero rows updated means refuse**. Two concurrent requests carrying
the same six digits cannot both see zero rows.

**Rationale**: this is RFC 6238 §5.2's own recommendation, it is what the
source comment anticipated, and doing it in the `UPDATE`'s `WHERE` clause
makes it correct under concurrency without a lock — which a read-then-write
would not be, and the interesting attack is precisely two requests racing
inside one 30-second step.

**Consequence accepted, and it is the right trade**: one account can complete
at most one TOTP verification per 30-second step. Signing in twice inside
thirty seconds means waiting for the next code. That is the intended
behaviour — the second use *is* the replay this requirement forbids, and the
product cannot tell the honest one from the intercepted one.

**Not an ADR.** It changes no boundary, adds no subsystem, and implements an
intent already recorded in the source it modifies. Recording that judgement
here rather than minting a fourth ADR is deliberate; Principle IV asks for
ADRs where decisions diverge across files, not for one per requirement.

**Alternatives rejected**:

- *Store every consumed `(user, step)` pair in a table.* A row per sign-in,
  a sweep to write, and it answers a question a single high-water mark
  already answers — steps only move forwards.
- *Reduce the skew to zero.* Would shrink the window rather than close it,
  and `totp.rs` has already reasoned about why one step is right for a person
  whose phone clock drifts. The spec puts skew explicitly out of scope.
- *Bind the code to the challenge id.* The challenge is already single-use;
  the leak is that a *new* challenge accepts the *old* digits, so binding to
  the challenge changes nothing.

## R5 — How recovery codes are stored (FR-009)

**What is there today**: no recovery, backup or break-glass concept anywhere
in the codebase — but three storage precedents, and picking between them *is*
the decision:

- **Passwords**: Argon2 (`argon2 = "0.6.0"`, Argon2id, `m=19456, t=2, p=1`).
  `hash_password` lives at `src/server/src/auth/sessions.rs:409` and is
  `pub(crate)`; verification is inline `Argon2::default().verify_password(…)`
  at five call sites, through no helper at all.
- **The admin bootstrap code** — and this is the precedent that settles it.
  `random_setup_code()`
  (`crates/thunderforge-axum-auth-core/src/random.rs:24`) makes a 12-character
  code from an unambiguous alphabet (no I, O, 0 or 1) formatted
  `XXXX-XXXX-XXXX`; `admin_bootstrap.rs:8` stores it as an **Argon2 hash** via
  the very same `hash_password`, and `ensure_admin_setup_code_valid`
  (`admin_bootstrap.rs:92`) verifies it. A single-use, human-transcribed,
  account-granting secret, hashed with the password helper. That is a recovery
  code in every respect but the name.
- **Link codes**: `graphql/share_codes.rs::generate_link_code()`, 20 uppercase
  hex characters (~80 bits) from a v4 UUID, stored **raw** in
  `world_invites.invite_code`, `instance_invitations.invite_code` and the
  share tables — because a share link is a capability whose whole purpose is
  to be shown again.

**Decision**: recovery codes are stored the way **the bootstrap code** is —
which is to say the way passwords are — and not the way share codes are. Ten
codes, generated by `random_setup_code()`'s alphabet and grouping so a person
can transcribe one off a printed sheet without confusing `O` for `0`, each
stored as an **Argon2 hash** in `user_recovery_codes`. `hash_password` is
promoted out of `auth/sessions.rs` into a shared helper on the way past,
because a fifth inline `Argon2::default()` is how parameters drift. The plaintext is
returned exactly once, in the response that issues it, and exists nowhere on
the server afterwards. There is no endpoint that can display an issued code,
because there is nothing left to display — which is FR-009 by construction
rather than by an access rule somebody could relax.

**Verification is a full scan of the account's unused codes, without an early
exit.** All ten are verified even after one matches. That costs ten Argon2
verifications, sequentially — peak memory is one hash's worth (~19 MiB at
`Argon2::default()`), not ten — and it removes the timing oracle that would
otherwise distinguish "matched the second code" from "matched nothing"
(FR-018).

**Rationale**: the spec's own assumption says "stored the way passwords are —
verifiable, not readable". A recovery code is a credential that signs somebody
in; a share code is a link. Matching the wrong precedent here is how a
break-glass credential ends up readable by anyone with database access, which
is the exact posture the encrypted-at-rest TOTP secret already rejects — and
the instance already made this call once, correctly, for the bootstrap code.

**Cost, and why it is acceptable**: ten Argon2 verifications is roughly ten
times a login, on a path that (a) is reached only when somebody has lost
their authenticator, (b) already required a correct password to reach, and
(c) runs on the blocking pool. It is rate-limited with everything else on
this router (R9).

**Alternatives rejected**:

- *SHA-256 of the code.* Defensible on the entropy alone — 80 random bits are
  not brute-forced from a hash — and it would make verification free. Rejected
  because it makes the storage rule *depend on the generator staying strong*:
  the day somebody shortens a code to be friendlier to type, a fast hash
  becomes a crackable one, silently. Argon2 is correct whatever the code
  looks like, and the codebase already reaches for it for exactly this kind of
  secret. Recorded here so the trade is visible rather than rediscovered.
- *Storing them raw, as invitation and share codes are.* Consistent with the
  nearest-looking neighbour and wrong: `instance_invitations.invite_code` is
  matched by equality in a conditional `UPDATE`
  (`auth/instance_access.rs:181`), which is only possible because the value is
  readable. FR-009 forbids exactly the property that design depends on.
- *Encrypting the codes with the instance key, as the TOTP secret is.* The
  TOTP secret must be **recoverable** — the server has to compute codes from
  it. A recovery code never needs reading back, so encryption would preserve a
  capability whose absence is the requirement.
- *Storing a hash of the whole set.* Cannot express "this one is spent"
  (FR-008) without invalidating the rest.

## R6 — Single use, whole-set replacement, and running low

**Decision**: a `user_recovery_codes` row is spent by stamping `used_at`, and
the stamp is the guard — a conditional
`UPDATE … SET used_at = now() WHERE id = $id AND used_at IS NULL`, with zero
rows updated meaning refuse. Generating a fresh set **deletes** every
outstanding row for the account before inserting the new ten. "Running low"
is `count(*) WHERE used_at IS NULL <= 3`.

**Rationale**: the same conditional-write shape as R4, for the same reason —
FR-008 ("exactly once") is a concurrency claim, and two simultaneous
presentations of one code is precisely the case a read-then-write loses.
Deleting rather than marking on regeneration means FR-010's "every earlier
code stops working" is true even if a later query forgets a filter; there is
nothing left to match.

**Alternatives rejected**:

- *Mark old rows superseded instead of deleting.* Keeps hashes of dead
  credentials for no reader. The `two_factor_events` record (R7) already says
  a set was replaced, and says it without keeping the material.
- *Top up the set as codes are spent.* Then "how many do I have" has no
  stable answer and the person is never handed a full sheet to file away.

## R7 — What is recorded, and how the account holder is told

**What is there today**: two things, one useful and one a trap.

- `instance_access_events` (spec 035,
  `migrations/2026-09-06-000000-0000_instance_access/up.sql:85`) is the only
  structured audit sink in the product: `event_type` under a CHECK
  constraint, `occurred_at`, a nullable `actor_user_id`, and the policy
  before and after. Its migration and `models.rs:203` both state the rule
  out loud — **no email or identifier column, and none may be added** — and
  the insert helper `record_access_event` (`auth/instance_access.rs:126`) is
  deliberately unable to express one. That is the shape to copy.
- `record_auth_audit_event` (`src/server/src/users/mod.rs:135`) looks like the
  obvious home and **writes nothing**: it discards its `state` argument
  (`let _ = state;`) and emits a `tracing::info!`. Three callers believe they
  are recording something. Worth knowing before reaching for it.

And there is **no mail subsystem at all** — no `lettre`, no SMTP, no queue,
no template, no outbox. Spec 040 says so in as many words and owns building
one.

**Decision**: a `two_factor_events` table records enrolment, removal,
recovery-code use, recovery-code regeneration and operator reset — who, for
whom, when, and which kind. It is written on the same transaction as the
change it describes. Notification (FR-015, FR-025) is a **best-effort call to
a seam spec 040 fills**, made after the transaction commits, whose failure
changes nothing about the outcome.

**Rationale**: FR-001b is the load-bearing requirement here — "enrolment MUST
NOT require the instance to be able to send a message", because the first
administrator of a fresh instance enrols before mail exists (FR-029, SC-010).
If notification is in the transaction, or can fail the request, then the
instance that most needs enrolment to work is the one where it cannot. So the
record is the durable half and the message is the best-effort half, and where
no message can be sent the person still sees the event in their own security
settings — which is FR-026's "told who can help" as well.

**Alternatives rejected**:

- *Reuse spec 035's access-event table.* Different subject: that records
  reaching content, this records changes to a credential. Overloading it
  makes both queries harder to read and couples two features' retention
  rules.
- *Block removal until a notification is delivered.* Turns an unconfigured
  mail server into a lock on the security settings of every account.

## R8 — The administrator rule, and where spec 040's boundary falls

**What is there today**: `users.is_admin` is a boolean; there is no roles
table. First-run setup is `admin_setup_basic`
(`src/server/src/auth/admin_setup.rs:65`): it validates the bootstrap code,
inserts a user with `is_admin = true` and **all four 2FA columns at their
empty defaults** (`admin_setup.rs:129-144`), calls
`mark_admin_setup_complete_sync` (`admin_bootstrap.rs:446`) and issues a
session cookie. And `setup_status` computes completion as
`admin_exists || setup_completed_at.is_some()` (`admin_setup.rs:37`).

**That second clause matters and is easy to miss**: deferring
`setup_completed_at` alone would not defer completion, because the admin row
already exists by then. Both halves have to move.

**Decision**:

- `admin_setup_basic` creates the administrator and, instead of a session,
  returns an **`enrol` ticket** (R3). `mark_admin_setup_complete_sync` moves
  to the confirmation step. The session cookie is issued when the second
  factor is confirmed, in the same transaction that issues the recovery
  codes (FR-028, FR-029).
- `setup_status`'s completion test becomes "an administrator exists **and**
  holds a confirmed second factor", so a half-finished setup reads as
  unfinished from every direction.
- The requirement itself is computed, not stored: an account is required to
  hold a second factor when `is_admin` — full stop, with no column and no
  switch that could turn it off (FR-027). `two_factor_admin_required` and the
  instance-wide flag stay exactly what they are, for everybody else (FR-033).
- Granting `is_admin` therefore requires nothing new: the next sign-in finds
  the rule unsatisfied and issues an `enrol` ticket (FR-030). The same
  sentence covers an upgraded instance whose administrators predate the rule
  (FR-031) — they are walked through, never refused.
- Losing `is_admin` touches no 2FA column, so the factor survives (FR-032) by
  doing nothing, which is the only way to be sure.

**Boundary with spec 040**, stated so neither feature has to guess: **040
owns the setup wizard** — its steps, its other fields, mail, the operator
identity, the notice contact. **041 owns the enrolment step inside it and the
condition under which setup is complete.** Spec 040's FR-002a already states
the same gate; two specs stating it is deliberate, them disagreeing later
would not be.

**Alternatives rejected**:

- *A `two_factor_role_required` column set when `is_admin` is set.* Two
  sources of truth for one rule, and the column is a switch — FR-027 says
  there must not be one. A derived rule cannot be turned off by an `UPDATE`.
- *Let setup complete and challenge the admin at next sign-in.* SC-009 says a
  fresh instance cannot be brought up with an unprotected administrator, and
  the moment after setup is exactly when nobody comes back.
- *Require it of administrators only when the instance-wide policy is on.*
  That is the thing FR-027 explicitly is not.

## R9 — Refusals, and limiting guesses without creating a lockout

**What is there today**: `rate_limit_auth_requests`
(`src/server/src/auth_middleware.rs:100`) layers the whole authentication
router (`src/app/src/main.rs:635-642`). It keys on `"{ip}:{path}"` and allows
**15 per 60s** on `/authentication/{basic,login,register}` and **40 per 60s**
on every other `/authentication/*` path — so the 2FA routes are already at 40,
and every route added below inherits that number by existing. The bypass the
e2e harness uses (`THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT=1`) compiles only
under `#[cfg(debug_assertions)]`; the release build has a `false`-returning
stub (`auth_middleware.rs:96`).

Refusals on this path already use one message for two causes —
`"Invalid credentials"` for both an unknown username and a wrong password in
`two_factor_setup_start` — and spec 027's FR-011 set the house rule that a
dead link gives one message whatever the cause.

**Decision**: the new surfaces inherit that layer rather than adding their
own — being under `/authentication/` is the whole of the wiring — and add a
**per-challenge attempt counter** rather than a per-account one: a challenge
is spent after five failed codes, and the person begins a new sign-in. Refusals say "that code was not accepted" and
nothing else — not whether the account has a second factor, not whether a
recovery code would have worked, not how close the code was (FR-018).

**Rationale**: FR-017 requires the limit not turn an honest mistype into a
lockout, and the existing e2e explicitly asserts that a wrong code leaves the
challenge usable. Counting on the challenge rather than the account gives an
attacker no way to lock somebody out by guessing at them — the worst they can
do is burn challenges they created. Locking the *account* is precisely the
denial-of-service the requirement is written against.

**Alternatives rejected**:

- *Lock the account after N failures.* Hands anybody who knows a username a
  lockout button.
- *Exponential backoff per account.* Same lever, softer.
- *Nothing beyond the existing IP-keyed limiter.* `"{ip}:{path}"` does not
  bound guesses against one account from many callers, and it is in-process,
  so it bounds nothing at all across replicas.
- *Copy `share_rate_limit`, which ignores the bypass.* Tempting — that
  limiter is deliberately non-bypassable and has a test to prove it
  (`graphql/share_rate_limit.rs:113`). Rejected here because the e2e suite
  must be able to drive dozens of deliberate wrong codes, and a limiter the
  suite cannot turn off turns FR-017's own coverage into 429s. The
  **challenge counter is not bypassable** and carries the requirement; the
  volumetric layer stays bypassable, as it is for every other auth route.

## R10 — Rendering the QR code without a heavy dependency

**What is there today**: `apps/web/package.json` has **nothing that can draw
a QR code** — the dependencies are React, Radix/shadcn, CodeMirror,
lucide-react, marked, dompurify, flexsearch, graphql-ws, sonner. And no Rust
QR crate is in the workspace either. So this is a real choice, not a lookup.
The house precedent is right next door: `apps/web/e2e/two-factor.spec.ts`
hand-wrote six lines of base32 rather than add a package, and says why.

**Decision**: **the server encodes, the client draws.** The enrolment
response carries the `otpauth://` URI it already produces *plus* the QR
matrix — a size and one string of `0`/`1` per row — and React renders it as
inline SVG rectangles. The encoder is a small pure-Rust QR crate on the
server side only.

**Rationale**, in the order the alternatives lose:

- A **matrix, not markup**, because a server-returned SVG string would reach
  the page through `dangerouslySetInnerHTML`. A grid of booleans cannot carry
  a script tag, and it is directly assertable in a Rust test: this exact
  string encodes to this exact matrix.
- **On the server**, because the server already holds the secret and already
  builds the URI (`two_factor.rs:96`), so nothing new crosses a boundary, and
  because it keeps the web bundle unchanged — the engine already dominates
  first-load bytes and a QR encoder is not the place to spend more.
- **A crate, not hand-rolled**, because QR encoding is Reed–Solomon over
  GF(256) with version/mask selection. The base32 precedent holds for six
  lines of bit-shifting; it does not extend to an error-correcting code.

**Alternatives rejected**:

- *A JavaScript QR library.* A dependency, a bundle cost, and a second
  implementation of a string the server already owns — for one screen.
- *An external QR image service.* Would put the TOTP secret in a URL sent to
  a third party. Disqualifying on its own, and it would also break the
  air-gapped first-run instance FR-001b is written for.
- *Server-rendered SVG markup.* Fine encoding, wrong delivery: it makes a
  security screen the one place in the app that injects server HTML.

**And it is never the only route in** (FR-002): the base32 secret is
presented in typeable, grouped, copyable form beside the code, because a
desktop authenticator, a password manager or a person with no camera must be
able to enrol. If the QR fails to render at all, enrolment still completes.

## R11 — Where the code goes, and the 1000-line rule

**What is there today**: `src/server/src/auth/two_factor.rs` is 491 lines and
holds enrolment, verification and both admin switches.
`scripts/check-file-length.sh` fails any tracked `.rs` file over 1000 lines,
and names `auth/` itself as the established pattern for splitting.

**Decision**: `two_factor.rs` becomes the module directory
`src/server/src/auth/two_factor/` — `enrolment.rs`, `recovery.rs`,
`verification.rs`, `policy.rs`, `events.rs`, `mod.rs` — as part of this
feature rather than as a follow-up.

**Rationale**: this feature adds recovery codes, a removal path, enrolment
tickets, the step guard and the event record to a file that is already half
the cap. Splitting after the fact means one commit that is entirely movement
and impossible to review against the commit that added behaviour. Splitting
first means every task below names a small file.

**Alternatives rejected**:

- *Add to `two_factor.rs` and split when it fails the gate.* The gate fails
  mid-feature, and the split lands under time pressure.
- *A new sibling `two_factor_recovery.rs`.* Two top-level modules that share
  every helper, which is the shape `auth/` was split out of.
