ALTER TABLE users
ADD COLUMN two_factor_enabled BOOLEAN NOT NULL DEFAULT FALSE,
ADD COLUMN two_factor_secret_encrypted TEXT,
ADD COLUMN two_factor_confirmed_at TIMESTAMP,
ADD COLUMN two_factor_admin_required BOOLEAN NOT NULL DEFAULT FALSE;

CREATE TABLE auth_security_settings (
    id INT PRIMARY KEY,
    two_factor_required_for_all_users BOOLEAN NOT NULL DEFAULT FALSE,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO
    auth_security_settings (
        id,
        two_factor_required_for_all_users,
        updated_at
    )
VALUES (1, FALSE, CURRENT_TIMESTAMP) ON CONFLICT (id) DO NOTHING;

CREATE TABLE login_two_factor_challenges (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    expires_at TIMESTAMP NOT NULL,
    consumed_at TIMESTAMP,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_login_two_factor_challenges_user_id ON login_two_factor_challenges (user_id);

CREATE INDEX idx_login_two_factor_challenges_expires_at ON login_two_factor_challenges (expires_at);