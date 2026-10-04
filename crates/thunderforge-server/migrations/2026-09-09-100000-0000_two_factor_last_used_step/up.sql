-- Spec 041 FR-016: a code MUST NOT be accepted twice, including within the
-- window.
--
-- A TOTP code is valid for its whole 30-second step and, with the skew of one
-- this product uses, for the neighbouring steps too — so the same six digits
-- stay usable for roughly 30-90 seconds. Anybody who reads them over a
-- shoulder, off a screen share, or out of a logged request body can spend them
-- again inside that window against a fresh challenge.
--
-- Nothing stopped that. `verify_totp_code` threw away the step `totp-rs`
-- returns, and `totp.rs`'s own comment said what that value was for while
-- discarding it. This column is where the spent step is remembered.
--
-- BIGINT because a step is `unix_time / 30` — about 5.9e7 today and rising,
-- which fits an INTEGER now and stops doing so in 2038 along with everything
-- else that made that assumption.
--
-- NULL means an account that has never verified, for which every step is
-- available. That is also what every existing row gets, which is the right
-- default: nobody is locked out by this migration, and the first verification
-- after it establishes the floor.
ALTER TABLE users
  ADD COLUMN two_factor_last_used_step BIGINT;

COMMENT ON COLUMN users.two_factor_last_used_step IS
  'Spec 041 FR-016. The TOTP time step this account last spent. A verification is refused unless its step is strictly greater — strictly, not merely different, because the previous step is still inside the skew window and carries a different number. NULL until the account first verifies.';
