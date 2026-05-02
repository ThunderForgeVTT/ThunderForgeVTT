ALTER TABLE oauth_providers
DROP COLUMN IF EXISTS configured,
DROP COLUMN IF EXISTS oauth_client_secret,
DROP COLUMN IF EXISTS oauth_client_id;