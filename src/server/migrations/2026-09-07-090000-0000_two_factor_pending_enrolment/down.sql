ALTER TABLE users
    DROP COLUMN IF EXISTS two_factor_pending_started_at,
    DROP COLUMN IF EXISTS two_factor_pending_secret_encrypted;
