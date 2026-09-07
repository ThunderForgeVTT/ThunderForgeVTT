# Quickstart: proving Two-Factor Enrolment, Recovery and Removal

How to demonstrate each user story against a running stack, and which
automated case covers it afterwards. Every scenario is written so a person can
do it by hand — which for this feature is the whole point, because the thing
being built is the part a person can reach.

You will need a real authenticator app on a phone, or any desktop TOTP client.
Scenarios B, D and E can be done without one by typing the secret into a
desktop client; Scenario A is more honest with a phone camera.

## Prerequisites

```bash
make dev          # services, migrations, seeds; admin/admin, user1/user1, user2/user2
```

Full suite, when you want the automated answer:

```bash
node scripts/e2e-parallel.mjs --shards=2
```

Per-target checks, per Principle V:

```bash
cargo test --workspace -j 4        # server + crates + packs
make lint                          # lint-host + lint-wasm + file length
pnpm --filter @thunderforge/web test
```

**Note on the e2e stack**: the suite needs
`THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT=1` and `--workers=1` against an external
stack, or 429s from `rate_limit_auth_requests` masquerade as flaky tests. That
bypass compiles only under `debug_assertions`, and it does **not** disable the
five-attempt challenge budget — FR-017's coverage must not depend on a switch.

## Scenario A — Enrolling, from an account that has never heard of this (US1)

1. Sign in as `user1`. Go to **/settings/security**.
2. Choose to add a second factor.
3. **Expected**: a QR code *and* the secret in grouped, copyable text, side by
   side. Not the code alone.
4. Scan it. Enter the six digits.
5. **Mistype one first**, on purpose. **Expected**: "that code was not
   accepted", the QR is still on screen, the same secret still works. You do
   not re-scan (FR-001c).
6. Enter a correct code. **Expected**: ten recovery codes, shown once, with a
   plain sentence saying what they are for and that they will not be shown
   again. Save them — you need them for Scenario C.
7. Return to /settings/security. **Expected**: it says the factor is on and
   when it was confirmed.
8. Sign out and back in. **Expected**: the challenge.

**Covered by**: `apps/web/e2e/two-factor.spec.ts`, "enrolment through the
interface".

**Try it without a camera** (FR-002, acceptance scenario 6): copy the secret
into a desktop authenticator and complete the same flow. If the secret is only
available as an image, this is broken however well the QR renders.

## Scenario B — Starting again does not disarm what you have (US4, FR-013)

The defect this feature exists to close.

1. As the enrolled `user1`, begin a **new** enrolment from
   /settings/security. Do not confirm it.
2. Close the tab.
3. Sign out and back in.

**Expected**: you are challenged, and **your original authenticator still
works**. Before this feature, step 1 cleared `two_factor_enabled` and the
account was back to password-only — reachable by anybody holding the password,
because `setup/start` needed no session at all.

**Covered by**: `apps/web/e2e/two-factor.spec.ts`. This is the case currently
marked `test.fail()`. When this feature lands, Playwright reports "expected to
fail but passed" — **delete the `test.fail()` line then**. The assertion is
already correct; the product is what moves.

**Also try**: with a confirmed factor, complete the new enrolment. **Expected**:
the new authenticator works and the old one stops. A confirmed factor is
replaced, never disarmed.

## Scenario C — Getting back in with no phone (US2)

1. Sign out. Sign in as `user1` with the correct password.
2. At the challenge, use a **recovery code** from Scenario A.
3. **Expected**: you are signed in.
4. Sign out. Try **the same code** again.
5. **Expected**: refused, with the same message a wrong code gets — not "that
   code was already used".
6. Spend codes until three remain. **Expected**: at sign-in you are told how
   many are left and offered a new set.
7. Take a new set. **Expected**: every code from the old sheet is refused.

**Covered by**: `apps/web/e2e/two-factor.spec.ts`, "recovery codes".

**And check the thing you cannot see**: there is no screen, route or admin
control anywhere that shows an issued code again. If you find one, FR-009 is
broken.

## Scenario D — Turning it off costs what turning it on cost (US4)

1. As the enrolled `user1`, go to /settings/security and remove the second
   factor **with the password only**.
2. **Expected**: refused. Possession is required.
3. Remove it with the password **and** a current code.
4. **Expected**: removed, said plainly, and the removal appears in the account's
   own security record.
5. Sign in again. **Expected**: no challenge.

**Covered by**: `apps/web/e2e/two-factor.spec.ts`, "removing it deliberately".

## Scenario E — The same code, twice, inside the window (FR-016, SC-005)

1. Sign in as an enrolled account and note the six digits you used.
2. Sign out **immediately** and start a new sign-in, within the same
   30-second step.
3. Enter the same six digits.

**Expected**: refused. They are still cryptographically valid — that is the
point — and they are refused anyway, because the step they matched has been
spent. Wait for the next code and it works.

**Covered by**: a server test in `auth/two_factor/verification.rs` using
`matched_step_at`'s explicit clock, so it does not wait thirty seconds; and
one e2e case that does the real thing.

## Scenario F — The instance-wide switch stops being a lockout button (US5)

1. As `admin`, go to **/admin/security**. **Expected**: beside the switch, how
   many accounts have a factor, how many do not, and how many will be asked to
   enrol.
2. Turn the requirement on.
3. In another browser, sign in as `user2`, who has never enrolled.
4. **Expected**: after the correct password, `user2` is **taken through
   enrolment** — the same QR, the same typeable secret, the same recovery
   codes — and then arrives where they were going.
5. Turn the requirement off. **Expected**: `user2` keeps their second factor.

**Covered by**: `apps/web/e2e/two-factor.spec.ts`, "the instance-wide policy".
The existing policy test asserts enforcement and its comment says plainly that
it "deliberately does not pretend the lockout is fine". That comment is deleted
in the same change, because the lockout is gone.

## Scenario G — A fresh instance cannot come up unprotected (US3, SC-009)

Needs an empty database.

```bash
make dev-reset     # or drop and recreate the database, then `make dev`
```

1. Open the instance. Follow the setup link from the server's startup log.
2. Create the administrator account.
3. **Try to stop here.** Close the tab, reopen the instance.
4. **Expected**: setup is **not** complete. You are returned to the enrolment
   step, not to a working instance and not to a locked one.
5. Complete enrolment. **Expected**: the recovery codes appear **before** setup
   ends, and you can save them.
6. **Expected**: only now are you signed in and only now does setup read as
   complete.

**Do it with no mail configured at all** — which on a fresh instance is the
only option, since no mail subsystem exists yet. **Expected**: every step
above works. If any of it waits on, or fails because of, a message that cannot
be sent, FR-001b is broken and the first administrator of every new instance
is locked out of their own deployment.

**Covered by**: `apps/web/e2e/two-factor-setup.spec.ts`.

## Scenario H — Made an administrator, and an upgraded instance (US3, FR-030/031)

1. As `admin`, grant administrator to `user2`.
2. Sign in as `user2`.

**Expected**: taken through enrolment, not refused. Nothing was migrated and
no column was set — the rule is computed from `is_admin`, so an instance that
upgrades into it behaves identically for administrators that predate it.

3. Remove administrator from `user2`.

**Expected**: `user2` keeps their second factor (FR-032). Losing a role never
removes protection.

4. Look for a switch that turns the administrator rule off.

**Expected**: there is not one (FR-027). If you find a column, an environment
variable or a toggle, this is wrong.

## Scenario I — Somebody has lost everything (US7)

1. As `user1`, exhaust or discard every recovery code and remove the
   authenticator.
2. Try to sign in. **Expected**: the challenge names who can help.
3. As `admin`, reset that account's second factor from the administration
   surface.
4. **Expected**: no credential is handed to the operator, and nobody is signed
   in by the reset.
5. Sign in as `user1`. **Expected**: correct password, then enrolment.
6. Look at the record. **Expected**: who reset it, for whom, and when.
7. `user1` sees the same event in their own security settings.

**Expected throughout**: no `psql`. If any step needed one, FR-024 is not
satisfied.

**Covered by**: `apps/web/e2e/two-factor-admin.spec.ts`.

## Making the guards fail on purpose

House habit, and five of these earn it. Before believing any of them, break
the thing once and watch the test bite:

- restore `two_factor_enabled.eq(false)` to the `setup/start` update →
  Scenario B must fail;
- drop the `AND (two_factor_last_used_step IS NULL OR … < $step)` clause →
  Scenario E must fail;
- drop the `AND used_at IS NULL` from the recovery-code update → Scenario C
  step 5 must fail;
- let `disable` accept the password alone → Scenario D step 2 must fail;
- restore `mark_admin_setup_complete_sync` to `admin_setup_basic` →
  Scenario G step 4 must fail;
- add an early `return` to the recovery-code scan on first match → the
  constant-work test in `thunderforge-axum-auth-core` must fail.

The last one is the one people skip. It is also the only guard in this feature
whose absence is invisible from the outside.
