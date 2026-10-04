ALTER TABLE oauth_providers
ADD COLUMN config_source VARCHAR NOT NULL DEFAULT 'admin';
