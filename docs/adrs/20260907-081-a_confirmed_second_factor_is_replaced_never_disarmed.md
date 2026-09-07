# A Confirmed Second Factor Is Replaced, Never Disarmed

- **Date**: 2026-09-07
- **Status**: Accepted
- **Spec**: `specs/041-two-factor-enrolment/` (FR-004, FR-013, FR-001c)
- **Follows**: ADR-044's instinct that the party who may decide a thing must
  prove they are entitled to
- **Governs**: Constitution Principle III — authorization at the data boundary

## The decision

Starting a two-factor enrolment **never** changes the second factor an account
already has. An enrolment in progress is held beside the live one and promoted
only when a code proves the new secret works.

## What was there

`two_factor_setup_start` authenticated on the password alone and then, in one
statement, wrote a fresh secret over the live one, set `two_factor_enabled` to
false and cleared `two_factor_confirmed_at`.

There is no deliberate way to turn a second factor off in this product. So
that side effect was the only route off two-factor — **and it was also a route
round it.** Anybody holding the password of an enrolled account could call
`setup/start`, never confirm, and leave the account back on passwords alone.
The second factor was removed by somebody who had proved only the first, which
is precisely the thing a second factor exists to make impossible.

It was an accident doing the job of a decision: the *absence* of a removal
feature standing in for one.

## Why this shape

Two columns hold the enrolment in progress —
`two_factor_pending_secret_encrypted` and `two_factor_pending_started_at` —
and the live columns are not touched until confirmation. Three requirements
then fall out of the shape rather than having to be remembered:

- **FR-013**: a new enrolment cannot disarm the old factor, because it never
  writes to it.
- **FR-004**: an abandoned enrolment leaves the account exactly as it was,
  because nothing was changed to abandon.
- **FR-001c**: a mistyped confirming code costs nothing, because the pending
  secret is still there to try again against.

Columns rather than a table, deliberately: this shape needs no expiry, no
history and no second row per account, and `users` already carries the four
two-factor columns these sit beside. A pending-enrolment table is the right
home the day the flow needs attempts or a lifetime, and it does not today.

Promotion is one statement. Postgres evaluates every `SET` expression against
the old row, so copying the pending secret into the live column and clearing
the pending one in the same `UPDATE` is correct rather than a sequence that
could half-apply.

## Consequences

- **Positive.** Possession of the second factor is now required to stop being
  protected by it, which was the point of having one.
- **Positive.** `apps/web/e2e/two-factor.spec.ts` had this assertion written
  as an expected failure, deliberately stating the correct behaviour rather
  than the current one. The product moved; the assertion did not change, and
  the `test.fail()` marker was simply removed.
- **What this does NOT fix.** There is still no deliberate way to turn a second
  factor off. FR-012 and FR-014 specify one — password *plus* proof of
  possession — and it is unbuilt. Closing the accidental route makes that gap
  visible rather than filling it, and an account that enrols today cannot yet
  un-enrol at all. That is the safer of the two failures and it is not the end
  state.
- **Cost.** An account can sit with a pending enrolment indefinitely. It is
  inert — it grants nothing and blocks nothing — but it is untidy, and it is
  the first thing the table version would fix.
