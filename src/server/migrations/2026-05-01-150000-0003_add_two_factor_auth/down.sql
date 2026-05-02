DROP INDEX IF EXISTS idx_login_two_factor_challenges_expires_at;

DROP INDEX IF EXISTS idx_login_two_factor_challenges_user_id;

DROP TABLE IF EXISTS login_two_factor_challenges;

DROP TABLE IF EXISTS auth_security_settings;

ALTER TABLE users
DROP COLUMN IF EXISTS two_factor_admin_required,
DROP COLUMN IF EXISTS two_factor_confirmed_at,
DROP COLUMN IF EXISTS two_factor_secret_encrypted,
DROP COLUMN IF EXISTS two_factor_enabled;