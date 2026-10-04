CREATE TABLE oauth_providers (
    id UUID PRIMARY KEY,
    provider_key VARCHAR NOT NULL UNIQUE,
    display_name VARCHAR NOT NULL,
    authorization_url VARCHAR NOT NULL,
    token_url VARCHAR NOT NULL,
    userinfo_url VARCHAR,
    scopes TEXT[] NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE user_oauth_accounts (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    provider_id UUID NOT NULL REFERENCES oauth_providers (id) ON DELETE CASCADE,
    provider_user_id VARCHAR NOT NULL,
    provider_email VARCHAR,
    access_token_encrypted TEXT,
    refresh_token_encrypted TEXT,
    token_expires_at TIMESTAMP,
    linked_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT uq_user_oauth_accounts_provider_subject UNIQUE (provider_id, provider_user_id),
    CONSTRAINT uq_user_oauth_accounts_user_provider UNIQUE (user_id, provider_id)
);

CREATE TABLE oauth_link_challenges (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    provider_id UUID NOT NULL REFERENCES oauth_providers (id) ON DELETE CASCADE,
    provider_user_id VARCHAR NOT NULL,
    provider_email VARCHAR,
    challenge_code VARCHAR NOT NULL UNIQUE,
    expires_at TIMESTAMP NOT NULL,
    consumed_at TIMESTAMP,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_oauth_providers_provider_key ON oauth_providers (provider_key);

CREATE INDEX idx_user_oauth_accounts_user_id ON user_oauth_accounts (user_id);

CREATE INDEX idx_user_oauth_accounts_provider_id ON user_oauth_accounts (provider_id);

CREATE INDEX idx_oauth_link_challenges_user_id ON oauth_link_challenges (user_id);

CREATE INDEX idx_oauth_link_challenges_expires_at ON oauth_link_challenges (expires_at);