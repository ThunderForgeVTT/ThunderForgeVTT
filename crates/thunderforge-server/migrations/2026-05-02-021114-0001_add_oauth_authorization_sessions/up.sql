CREATE TABLE oauth_authorization_sessions (
    id UUID PRIMARY KEY,
    provider_id UUID NOT NULL REFERENCES oauth_providers (id) ON DELETE CASCADE,
    oauth_provider_key VARCHAR NOT NULL,
    oauth_client_id VARCHAR NOT NULL,
    state VARCHAR NOT NULL UNIQUE,
    code_verifier VARCHAR NOT NULL,
    redirect_uri VARCHAR NOT NULL,
    return_to VARCHAR,
    expires_at TIMESTAMP NOT NULL,
    consumed_at TIMESTAMP,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

ALTER TABLE oauth_link_challenges
ADD COLUMN pending_access_token_encrypted TEXT,
ADD COLUMN pending_refresh_token_encrypted TEXT,
ADD COLUMN pending_token_expires_at TIMESTAMP;

CREATE INDEX idx_oauth_authorization_sessions_state ON oauth_authorization_sessions (state);

CREATE INDEX idx_oauth_authorization_sessions_provider_id ON oauth_authorization_sessions (provider_id);

CREATE INDEX idx_oauth_authorization_sessions_expires_at ON oauth_authorization_sessions (expires_at);