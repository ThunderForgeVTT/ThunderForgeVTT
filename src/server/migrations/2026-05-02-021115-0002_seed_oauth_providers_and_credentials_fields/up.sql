ALTER TABLE oauth_providers
ADD COLUMN oauth_client_id VARCHAR,
ADD COLUMN oauth_client_secret VARCHAR,
ADD COLUMN configured BOOLEAN NOT NULL DEFAULT FALSE;

INSERT INTO oauth_providers (
    id,
    provider_key,
    display_name,
    authorization_url,
    token_url,
    userinfo_url,
    scopes,
    oauth_client_id,
    oauth_client_secret,
    configured,
    enabled,
    created_at,
    updated_at
)
VALUES
    (
        '0dbe77f4-5ec7-4cf3-b36f-153be6bb84f7',
        'discord',
        'Discord',
        'https://discord.com/api/oauth2/authorize',
        'https://discord.com/api/oauth2/token',
        'https://discord.com/api/users/@me',
        ARRAY['identify', 'email']::TEXT[],
        NULL,
        NULL,
        FALSE,
        FALSE,
        CURRENT_TIMESTAMP,
        CURRENT_TIMESTAMP
    ),
    (
        '84ccf4e8-2d3d-4ad2-a0f2-9adf460df6bd',
        'google',
        'Google',
        'https://accounts.google.com/o/oauth2/v2/auth',
        'https://oauth2.googleapis.com/token',
        'https://openidconnect.googleapis.com/v1/userinfo',
        ARRAY['openid', 'profile', 'email']::TEXT[],
        NULL,
        NULL,
        FALSE,
        FALSE,
        CURRENT_TIMESTAMP,
        CURRENT_TIMESTAMP
    ),
    (
        '6c53a95a-827f-42ea-bec0-7d030f4cbf1c',
        'github',
        'GitHub',
        'https://github.com/login/oauth/authorize',
        'https://github.com/login/oauth/access_token',
        'https://api.github.com/user',
        ARRAY['read:user', 'user:email']::TEXT[],
        NULL,
        NULL,
        FALSE,
        FALSE,
        CURRENT_TIMESTAMP,
        CURRENT_TIMESTAMP
    )
ON CONFLICT (provider_key) DO UPDATE
SET
    display_name = EXCLUDED.display_name,
    authorization_url = EXCLUDED.authorization_url,
    token_url = EXCLUDED.token_url,
    userinfo_url = EXCLUDED.userinfo_url,
    scopes = EXCLUDED.scopes,
    updated_at = CURRENT_TIMESTAMP;