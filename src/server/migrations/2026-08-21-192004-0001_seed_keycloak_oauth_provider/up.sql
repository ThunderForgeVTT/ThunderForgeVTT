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
        '3f1a6e2c-8b0d-4e5a-9c7f-2d4b6a8e0c1f',
        'keycloak',
        'Keycloak',
        '',
        '',
        NULL,
        ARRAY['openid', 'profile', 'email']::TEXT[],
        NULL,
        NULL,
        FALSE,
        FALSE,
        CURRENT_TIMESTAMP,
        CURRENT_TIMESTAMP
    )
ON CONFLICT (provider_key) DO NOTHING;
