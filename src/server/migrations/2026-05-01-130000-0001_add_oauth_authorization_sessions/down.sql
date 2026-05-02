DROP INDEX IF EXISTS idx_oauth_authorization_sessions_expires_at;

DROP INDEX IF EXISTS idx_oauth_authorization_sessions_provider_id;

DROP INDEX IF EXISTS idx_oauth_authorization_sessions_state;

ALTER TABLE oauth_link_challenges
DROP COLUMN IF EXISTS pending_token_expires_at,
DROP COLUMN IF EXISTS pending_refresh_token_encrypted,
DROP COLUMN IF EXISTS pending_access_token_encrypted;

DROP TABLE IF EXISTS oauth_authorization_sessions;