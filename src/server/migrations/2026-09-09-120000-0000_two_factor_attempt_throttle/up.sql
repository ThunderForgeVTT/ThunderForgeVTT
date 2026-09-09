-- Spec 041 FR-017: repeated incorrect codes are limited, without turning an
-- honest mistype into a lockout.
--
-- # Why the per-IP limiter is not enough
--
-- `rate_limit_auth_requests` already caps `/authentication/` at 40 requests a
-- minute **per address**. That is a bound on one attacker from one place; it is
-- not a bound on attempts against one *account*, and the difference matters
-- here more than almost anywhere else in the product. A TOTP code is six
-- digits — a million possibilities — and it stays valid for 30-90 seconds.
-- Spread across rotating addresses, a per-IP cap scales linearly with the
-- number of addresses and bounds nothing at all.
--
-- FR-017 is about the account, so the counter is on the account.
--
-- # Why a cooling-off and not a lock
--
-- The second half of FR-017: an honest mistype must not cost somebody their
-- account. So repeated failures buy a short, self-clearing pause rather than a
-- state an administrator has to undo. Five consecutive failures — well past
-- fumbling a code, far short of a search — and the pause clears on its own.
-- Any success resets the counter, so a person who mistypes twice and then gets
-- it right is never closer to a pause than somebody who never mistyped.
--
-- # Why in the database
--
-- The existing limiter is a process-local `HashMap`, which means a second
-- server process is a second budget, and a restart is a fresh one. Neither is
-- acceptable for the thing standing in front of a second factor.
ALTER TABLE users
  ADD COLUMN two_factor_failed_attempts INTEGER NOT NULL DEFAULT 0,
  ADD COLUMN two_factor_locked_until TIMESTAMP;

COMMENT ON COLUMN users.two_factor_failed_attempts IS
  'Spec 041 FR-017. Consecutive failed second-factor attempts. Reset to zero by any success, and by the start of a new cooling-off period.';

COMMENT ON COLUMN users.two_factor_locked_until IS
  'Spec 041 FR-017. While in the future, second-factor verification is refused without evaluating the code. Self-clearing: a cooling-off, never a lockout an administrator must undo.';
