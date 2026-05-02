CREATE TABLE admin_bootstrap_setup (
    id INT PRIMARY KEY,
    setup_completed_at TIMESTAMP,
    admin_code_hash VARCHAR,
    admin_code_generated_at TIMESTAMP,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW()
);

INSERT INTO
    admin_bootstrap_setup (id, created_at, updated_at)
VALUES (1, NOW(), NOW()) ON CONFLICT (id) DO NOTHING;

CREATE TABLE admin_bootstrap_oauth_sessions (
    id UUID PRIMARY KEY,
    provider_id UUID NOT NULL REFERENCES oauth_providers (id) ON DELETE CASCADE,
    oauth_provider_key VARCHAR NOT NULL,
    oauth_client_id VARCHAR NOT NULL,
    state VARCHAR NOT NULL UNIQUE,
    code_verifier VARCHAR NOT NULL,
    redirect_uri VARCHAR NOT NULL,
    desired_username VARCHAR,
    return_to VARCHAR,
    expires_at TIMESTAMP NOT NULL,
    consumed_at TIMESTAMP,
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_admin_bootstrap_oauth_sessions_state ON admin_bootstrap_oauth_sessions (state);