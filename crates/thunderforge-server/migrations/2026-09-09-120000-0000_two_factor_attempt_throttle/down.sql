ALTER TABLE users
  DROP COLUMN two_factor_failed_attempts,
  DROP COLUMN two_factor_locked_until;
