//! The OAuth/OIDC login flow: start, callback, code exchange, and resolving
//! what an external identity means for an account that may or may not exist.

use super::*;

pub(crate) async fn oauth_start(
    Path(provider_key): Path<String>,
    Query(query): Query<OAuthStartQuery>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, Json<OAuthResponse>)> {
    let mut conn = state.db_pool.get().expect("Failed to get DB connection");
    let provider_key_clone = provider_key.clone();
    let now = Utc::now().naive_utc();
    let state_token = generate_state();
    let code_verifier = generate_code_verifier();

    let provider = tokio::task::spawn_blocking(move || {
        oauth_providers::table
            .filter(oauth_providers::provider_key.eq(provider_key_clone))
            .filter(oauth_providers::enabled.eq(true))
            .select(OAuthProvider::as_select())
            .first::<OAuthProvider>(&mut conn)
            .optional()
    })
    .await
    .expect("Failed to spawn blocking task")
    .expect("Failed to query DB")
    .ok_or_else(|| {
        error_response(
            StatusCode::NOT_FOUND,
            "provider_not_found",
            "OAuth provider is not configured or disabled",
        )
    })?;

    if !provider.configured {
        return Err(error_response(
            StatusCode::CONFLICT,
            "provider_not_configured",
            "Provider exists but is not configured with client credentials",
        ));
    }

    let Some(provider_client_id) = provider.oauth_client_id.clone() else {
        return Err(error_response(
            StatusCode::CONFLICT,
            "provider_not_configured",
            "Provider client id is not set",
        ));
    };

    let session = NewOAuthAuthorizationSession {
        id: uuid::Uuid::now_v7(),
        provider_id: provider.id,
        oauth_provider_key: provider_key.clone(),
        oauth_client_id: provider_client_id.clone(),
        state: state_token.clone(),
        code_verifier: code_verifier.clone(),
        redirect_uri: query.redirect_uri.clone(),
        return_to: query.return_to,
        expires_at: now + chrono::Duration::minutes(10),
        consumed_at: None,
        created_at: now,
    };

    let mut conn = state.db_pool.get().expect("Failed to get DB connection");
    tokio::task::spawn_blocking(move || {
        diesel::insert_into(oauth_authorization_sessions::table)
            .values(&session)
            .execute(&mut conn)
    })
    .await
    .expect("Failed to spawn blocking task")
    .expect("Failed to persist oauth authorization session");

    let authorization_url = build_authorize_url(&AuthorizeRequest {
        authorization_url: &provider.authorization_url,
        client_id: &provider_client_id,
        redirect_uri: &query.redirect_uri,
        scopes: &provider_scopes(&provider),
        state: &state_token,
        code_challenge: &code_challenge_from_verifier(&code_verifier),
    })
    .map_err(|_| {
        error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "provider_misconfigured",
            "Provider authorization URL is invalid",
        )
    })?;

    Ok(Redirect::temporary(&authorization_url))
}

pub(crate) async fn oauth_callback(
    Path(provider_key): Path<String>,
    Query(query): Query<OAuthCallbackQuery>,
    cookies: Cookies,
    State(state): State<AppState>,
) -> (StatusCode, Json<OAuthResponse>) {
    // An `error` present means the provider refused, even if it also sent a
    // `code`: redeeming that code would complete a login the provider just
    // declined.
    if let Some(provider_error) = provider_error_from_callback(query.error, query.error_description)
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(OAuthResponse {
                status: "oauth_error",
                message: provider_error.message(),
                challenge_id: None,
                login_two_factor_challenge_id: None,
            }),
        );
    }

    let Some(code) = query.code else {
        return error_response(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Missing 'code' query parameter",
        );
    };
    let Some(state_token) = query.state else {
        return error_response(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Missing 'state' query parameter",
        );
    };

    handle_oauth_code_flow(state, cookies, provider_key, state_token, code).await
}

pub(crate) async fn oauth_token_exchange(
    Path(provider_key): Path<String>,
    State(state): State<AppState>,
    cookies: Cookies,
    Json(payload): Json<OAuthTokenExchangeRequest>,
) -> (StatusCode, Json<OAuthResponse>) {
    handle_oauth_code_flow(state, cookies, provider_key, payload.state, payload.code).await
}

pub(crate) async fn oauth_resolve(
    cookies: Cookies,
    State(state): State<AppState>,
    Json(request): Json<OAuthResolveRequest>,
) -> (StatusCode, Json<OAuthResponse>) {
    resolve_oauth_login(state, cookies, request).await
}

pub(crate) async fn handle_oauth_code_flow(
    state: AppState,
    cookies: Cookies,
    provider_key: String,
    state_token: String,
    code: String,
) -> (StatusCode, Json<OAuthResponse>) {
    let auth_ctx =
        match load_and_consume_authorization_session(&state, &provider_key, &state_token).await {
            Ok(ctx) => ctx,
            Err(resp) => return resp,
        };

    let token_response = match exchange_authorization_code(&auth_ctx, &code).await {
        Ok(tokens) => tokens,
        Err(msg) => {
            return error_response(
                StatusCode::BAD_GATEWAY,
                "token_exchange_failed",
                msg.as_str(),
            );
        }
    };

    let userinfo = if let Some(userinfo_url) = auth_ctx.provider.userinfo_url.clone() {
        match fetch_userinfo(userinfo_url, token_response.access_token.clone()).await {
            Ok(v) => Some(v),
            Err(msg) => {
                return error_response(StatusCode::BAD_GATEWAY, "userinfo_failed", msg.as_str());
            }
        }
    } else {
        None
    };

    let provider_user_id = userinfo
        .as_ref()
        .and_then(extract_provider_user_id)
        .or_else(|| extract_provider_user_id_from_token(&token_response));

    let Some(provider_user_id) = provider_user_id else {
        return error_response(
            StatusCode::BAD_GATEWAY,
            "identity_missing",
            "Could not extract provider user id from provider response",
        );
    };

    let provider_email = userinfo.as_ref().and_then(extract_provider_email);
    let expires_at = token_response
        .expires_in
        .map(|seconds| Utc::now() + chrono::Duration::seconds(seconds));

    let resolve_request = OAuthResolveRequest {
        provider_key,
        provider_user_id,
        provider_email,
        access_token: Some(token_response.access_token),
        refresh_token: token_response.refresh_token,
        token_expires_at: expires_at,
    };

    resolve_oauth_login(state, cookies, resolve_request).await
}

pub(crate) async fn resolve_oauth_login(
    state: AppState,
    cookies: Cookies,
    request: OAuthResolveRequest,
) -> (StatusCode, Json<OAuthResponse>) {
    let encryption_key = match encryption_key_from_config_secret(&state.config.secret) {
        Ok(key) => key,
        Err(msg) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "encryption_key_invalid",
                msg.as_str(),
            );
        }
    };

    let access_token_encrypted = match request
        .access_token
        .as_deref()
        .map(|v| encrypt_secret(v, &encryption_key))
        .transpose()
    {
        Ok(v) => v,
        Err(msg) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "encryption_failed",
                msg.as_str(),
            );
        }
    };

    let refresh_token_encrypted = match request
        .refresh_token
        .as_deref()
        .map(|v| encrypt_secret(v, &encryption_key))
        .transpose()
    {
        Ok(v) => v,
        Err(msg) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "encryption_failed",
                msg.as_str(),
            );
        }
    };

    let mut conn = state.db_pool.get().expect("Failed to get DB connection");
    let now = Utc::now().naive_utc();
    let provider_key = request.provider_key;
    let provider_key_for_audit = provider_key.clone();
    let provider_user_id = request.provider_user_id;
    let provider_email = request
        .provider_email
        .map(|v| v.trim().to_lowercase())
        .filter(|v| !v.is_empty());
    let token_expires_at = request.token_expires_at.map(|v| v.naive_utc());

    let outcome =
        tokio::task::spawn_blocking(move || -> Result<ResolveOutcome, diesel::result::Error> {
            let provider = oauth_providers::table
                .filter(oauth_providers::provider_key.eq(&provider_key))
                .filter(oauth_providers::enabled.eq(true))
                .select(OAuthProvider::as_select())
                .first::<OAuthProvider>(&mut conn)
                .optional()?;

            let Some(provider) = provider else {
                return Ok(ResolveOutcome::ProviderNotFound);
            };

            let existing_link = user_oauth_accounts::table
                .filter(user_oauth_accounts::provider_id.eq(provider.id))
                .filter(user_oauth_accounts::provider_user_id.eq(&provider_user_id))
                .select(UserOAuthAccount::as_select())
                .first::<UserOAuthAccount>(&mut conn)
                .optional()?;

            if let Some(existing_link) = existing_link {
                diesel::update(
                    user_oauth_accounts::table.filter(user_oauth_accounts::id.eq(existing_link.id)),
                )
                .set((
                    user_oauth_accounts::provider_email.eq(provider_email.clone()),
                    user_oauth_accounts::access_token_encrypted.eq(access_token_encrypted.clone()),
                    user_oauth_accounts::refresh_token_encrypted
                        .eq(refresh_token_encrypted.clone()),
                    user_oauth_accounts::token_expires_at.eq(token_expires_at),
                    user_oauth_accounts::updated_at.eq(now),
                ))
                .execute(&mut conn)?;

                return Ok(ResolveOutcome::LinkedUser(existing_link.user_id));
            }

            let Some(provider_email) = provider_email else {
                return Ok(ResolveOutcome::NoMatchingUser);
            };

            let existing_user_id = users::table
                .filter(users::email.eq(&provider_email))
                .select(users::id)
                .first::<uuid::Uuid>(&mut conn)
                .optional()?;

            let Some(existing_user_id) = existing_user_id else {
                // ADR-011: unlike an existing local account (which still
                // requires password confirmation below before linking), a
                // first-time OAuth identity with no local account at all is
                // auto-provisioned. The provider already vouched for this
                // email, so there is no password to protect and no ambiguity
                // about which account to link.
                let username = unique_username_from_email_sync(&mut conn, &provider_email)?;
                let random_password_hash = hash_password(&random_urlsafe(48))
                    .expect("Failed to hash random password for auto-provisioned OAuth user");

                let new_user_id = uuid::Uuid::now_v7();
                diesel::insert_into(users::table)
                    .values((
                        users::id.eq(new_user_id),
                        users::username.eq(&username),
                        users::email.eq(&provider_email),
                        users::is_admin.eq(false),
                        users::password_hash.eq(random_password_hash),
                        users::created_at.eq(now),
                        users::updated_at.eq(now),
                        users::two_factor_enabled.eq(false),
                        users::two_factor_secret_encrypted.eq::<Option<String>>(None),
                        users::two_factor_confirmed_at.eq::<Option<chrono::NaiveDateTime>>(None),
                        users::two_factor_admin_required.eq(false),
                    ))
                    .execute(&mut conn)?;

                let oauth_account = NewUserOAuthAccount {
                    id: uuid::Uuid::now_v7(),
                    user_id: new_user_id,
                    provider_id: provider.id,
                    provider_user_id,
                    provider_email: Some(provider_email),
                    access_token_encrypted,
                    refresh_token_encrypted,
                    token_expires_at,
                    linked_at: now,
                    created_at: now,
                    updated_at: now,
                };

                diesel::insert_into(user_oauth_accounts::table)
                    .values(&oauth_account)
                    .execute(&mut conn)?;

                return Ok(ResolveOutcome::LinkedUser(new_user_id));
            };

            let challenge_id = uuid::Uuid::now_v7();
            let challenge = NewOAuthLinkChallenge {
                id: challenge_id,
                user_id: existing_user_id,
                provider_id: provider.id,
                provider_user_id,
                provider_email: Some(provider_email),
                challenge_code: uuid::Uuid::now_v7().to_string(),
                expires_at: now + chrono::Duration::minutes(10),
                consumed_at: None,
                pending_access_token_encrypted: access_token_encrypted,
                pending_refresh_token_encrypted: refresh_token_encrypted,
                pending_token_expires_at: token_expires_at,
                created_at: now,
            };

            diesel::insert_into(oauth_link_challenges::table)
                .values(&challenge)
                .execute(&mut conn)?;

            Ok(ResolveOutcome::PasswordRequired(challenge_id))
        })
        .await
        .expect("Failed to spawn blocking task")
        .expect("Failed to resolve oauth login");

    match outcome {
        ResolveOutcome::ProviderNotFound => error_response(
            StatusCode::NOT_FOUND,
            "provider_not_found",
            "OAuth provider is not configured or disabled",
        ),
        ResolveOutcome::LinkedUser(user_id) => {
            let two_factor_required = match is_two_factor_required_for_user(&state, user_id).await {
                Ok(v) => v,
                Err(msg) => {
                    return error_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "two_factor_error",
                        msg.as_str(),
                    );
                }
            };

            if two_factor_required {
                let challenge_id = match create_login_two_factor_challenge(&state, user_id).await {
                    Ok(v) => v,
                    Err(msg) => {
                        return error_response(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "two_factor_error",
                            msg.as_str(),
                        );
                    }
                };

                return (
                    StatusCode::UNAUTHORIZED,
                    Json(OAuthResponse {
                        status: "two_factor_required",
                        message: "2FA code required to complete sign-in".to_string(),
                        challenge_id: None,
                        login_two_factor_challenge_id: Some(challenge_id),
                    }),
                );
            }

            if let Err(msg) = issue_session_cookie(&state, &cookies, user_id).await {
                return error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "session_error",
                    msg.as_str(),
                );
            }

            (
                StatusCode::OK,
                Json(OAuthResponse {
                    status: "success",
                    message: "OAuth account is already linked and signed in".to_string(),
                    challenge_id: None,
                    login_two_factor_challenge_id: None,
                }),
            )
        }
        ResolveOutcome::PasswordRequired(challenge_id) => {
            let _ = record_auth_audit_event(
                &state,
                None,
                "oauth_link_challenge_issued",
                None,
                Some(serde_json::json!({
                    "challenge_id": challenge_id,
                    "provider_key": provider_key_for_audit,
                })),
            )
            .await;

            (
                StatusCode::CONFLICT,
                Json(OAuthResponse {
                    status: "password_required",
                    message:
                        "Existing account detected; confirm password to link this OAuth account"
                            .to_string(),
                    challenge_id: Some(challenge_id),
                    login_two_factor_challenge_id: None,
                }),
            )
        }
        ResolveOutcome::NoMatchingUser => error_response(
            StatusCode::NOT_FOUND,
            "no_matching_user",
            "The OAuth provider did not return an email address, so this identity cannot be linked or auto-provisioned",
        ),
    }
}

pub(crate) async fn oauth_link_confirm(
    cookies: Cookies,
    State(state): State<AppState>,
    Json(request): Json<OAuthLinkConfirmRequest>,
) -> (StatusCode, Json<OAuthResponse>) {
    let challenge_id = request.challenge_id;
    let mut conn = state.db_pool.get().expect("Failed to get DB connection");
    let now = Utc::now().naive_utc();

    let outcome = tokio::task::spawn_blocking(
        move || -> Result<LinkConfirmOutcome, diesel::result::Error> {
            let challenge = oauth_link_challenges::table
                .filter(oauth_link_challenges::id.eq(request.challenge_id))
                .select(OAuthLinkChallenge::as_select())
                .first::<OAuthLinkChallenge>(&mut conn)
                .optional()?;

            let Some(challenge) = challenge else {
                return Ok(LinkConfirmOutcome::ChallengeInvalid);
            };

            if challenge.consumed_at.is_some() {
                return Ok(LinkConfirmOutcome::ChallengeInvalid);
            }

            if challenge.expires_at <= now {
                return Ok(LinkConfirmOutcome::ChallengeExpired);
            }

            let password_hash = users::table
                .filter(users::id.eq(challenge.user_id))
                .select(users::password_hash)
                .first::<String>(&mut conn)
                .optional()?;

            let Some(password_hash) = password_hash else {
                return Ok(LinkConfirmOutcome::ChallengeInvalid);
            };

            let parsed_hash = PasswordHash::new(&password_hash).expect("Invalid hash in db");
            if Argon2::default()
                .verify_password(request.password.as_bytes(), &parsed_hash)
                .is_err()
            {
                return Ok(LinkConfirmOutcome::PasswordMismatch);
            }

            let account_for_subject = user_oauth_accounts::table
                .filter(user_oauth_accounts::provider_id.eq(challenge.provider_id))
                .filter(user_oauth_accounts::provider_user_id.eq(&challenge.provider_user_id))
                .select(UserOAuthAccount::as_select())
                .first::<UserOAuthAccount>(&mut conn)
                .optional()?;

            if let Some(account_for_subject) = account_for_subject.as_ref()
                && account_for_subject.user_id != challenge.user_id
            {
                return Ok(LinkConfirmOutcome::LinkConflict);
            }

            let account_for_user_provider = user_oauth_accounts::table
                .filter(user_oauth_accounts::user_id.eq(challenge.user_id))
                .filter(user_oauth_accounts::provider_id.eq(challenge.provider_id))
                .select(UserOAuthAccount::as_select())
                .first::<UserOAuthAccount>(&mut conn)
                .optional()?;

            if account_for_user_provider.is_none() && account_for_subject.is_none() {
                let account = NewUserOAuthAccount {
                    id: uuid::Uuid::now_v7(),
                    user_id: challenge.user_id,
                    provider_id: challenge.provider_id,
                    provider_user_id: challenge.provider_user_id,
                    provider_email: challenge.provider_email,
                    access_token_encrypted: challenge.pending_access_token_encrypted,
                    refresh_token_encrypted: challenge.pending_refresh_token_encrypted,
                    token_expires_at: challenge.pending_token_expires_at,
                    linked_at: now,
                    created_at: now,
                    updated_at: now,
                };

                diesel::insert_into(user_oauth_accounts::table)
                    .values(&account)
                    .execute(&mut conn)?;
            }

            diesel::update(
                oauth_link_challenges::table.filter(oauth_link_challenges::id.eq(challenge.id)),
            )
            .set(oauth_link_challenges::consumed_at.eq(Some(now)))
            .execute(&mut conn)?;

            Ok(LinkConfirmOutcome::Linked(challenge.user_id))
        },
    )
    .await
    .expect("Failed to spawn blocking task")
    .expect("Failed to confirm oauth link");

    match outcome {
        LinkConfirmOutcome::ChallengeInvalid => error_response(
            StatusCode::BAD_REQUEST,
            "invalid_challenge",
            "Challenge is invalid or already consumed",
        ),
        LinkConfirmOutcome::ChallengeExpired => error_response(
            StatusCode::BAD_REQUEST,
            "challenge_expired",
            "Challenge has expired; restart OAuth sign-in",
        ),
        LinkConfirmOutcome::PasswordMismatch => error_response(
            StatusCode::UNAUTHORIZED,
            "password_mismatch",
            "Password verification failed",
        ),
        LinkConfirmOutcome::LinkConflict => error_response(
            StatusCode::CONFLICT,
            "link_conflict",
            "That OAuth identity is already linked to a different account",
        ),
        LinkConfirmOutcome::Linked(user_id) => {
            if let Err(msg) = issue_session_cookie(&state, &cookies, user_id).await {
                return error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "session_error",
                    msg.as_str(),
                );
            }

            let _ = record_auth_audit_event(
                &state,
                Some(user_id),
                "oauth_link_confirmed",
                None,
                Some(serde_json::json!({
                    "challenge_id": challenge_id,
                })),
            )
            .await;

            (
                StatusCode::OK,
                Json(OAuthResponse {
                    status: "success",
                    message: "OAuth account linked successfully".to_string(),
                    challenge_id: None,
                    login_two_factor_challenge_id: None,
                }),
            )
        }
    }
}

pub(crate) async fn load_and_consume_authorization_session(
    state: &AppState,
    provider_key: &str,
    state_token: &str,
) -> Result<OAuthAuthorizationContext, (StatusCode, Json<OAuthResponse>)> {
    let mut conn = state.db_pool.get().expect("Failed to get DB connection");
    let provider_key = provider_key.to_string();
    let state_token = state_token.to_string();
    let now = Utc::now().naive_utc();

    let result = tokio::task::spawn_blocking(
        move || -> Result<Option<OAuthAuthorizationContext>, diesel::result::Error> {
            let session = oauth_authorization_sessions::table
                .filter(oauth_authorization_sessions::oauth_provider_key.eq(&provider_key))
                .filter(oauth_authorization_sessions::state.eq(&state_token))
                .select(OAuthAuthorizationSession::as_select())
                .first::<OAuthAuthorizationSession>(&mut conn)
                .optional()?;

            let Some(session) = session else {
                return Ok(None);
            };

            if session.consumed_at.is_some() || session.expires_at <= now {
                return Ok(None);
            }

            let provider = oauth_providers::table
                .filter(oauth_providers::id.eq(session.provider_id))
                .filter(oauth_providers::enabled.eq(true))
                .filter(oauth_providers::configured.eq(true))
                .select(OAuthProvider::as_select())
                .first::<OAuthProvider>(&mut conn)
                .optional()?;

            let Some(provider) = provider else {
                return Ok(None);
            };

            diesel::update(
                oauth_authorization_sessions::table
                    .filter(oauth_authorization_sessions::id.eq(session.id)),
            )
            .set(oauth_authorization_sessions::consumed_at.eq(Some(now)))
            .execute(&mut conn)?;

            Ok(Some(OAuthAuthorizationContext { provider, session }))
        },
    )
    .await
    .expect("Failed to spawn blocking task")
    .expect("Failed to query oauth authorization session");

    result.ok_or_else(|| {
        error_response(
            StatusCode::BAD_REQUEST,
            "invalid_oauth_state",
            "OAuth state is invalid, expired, or already consumed",
        )
    })
}

pub(crate) async fn exchange_authorization_code(
    auth_ctx: &OAuthAuthorizationContext,
    code: &str,
) -> Result<OAuthTokenResponse, String> {
    exchange_authorization_code_with_provider(
        &auth_ctx.provider,
        &auth_ctx.session.redirect_uri,
        &auth_ctx.session.code_verifier,
        code,
    )
    .await
}

pub(crate) async fn exchange_authorization_code_with_provider(
    provider: &OAuthProvider,
    redirect_uri: &str,
    code_verifier: &str,
    code: &str,
) -> Result<OAuthTokenResponse, String> {
    let Some(client_id) = provider.oauth_client_id.as_deref() else {
        return Err("Provider client id is not configured".to_string());
    };
    let Some(client_secret) = provider.oauth_client_secret.as_deref() else {
        return Err("Provider client secret is not configured".to_string());
    };

    let client = reqwest::Client::new();
    let response = client
        .post(&provider.token_url)
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("code_verifier", code_verifier),
        ])
        .send()
        .await
        .map_err(|e| format!("Token request failed: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "<no body>".to_string());
        return Err(format!(
            "Token exchange failed with status {status}: {body}"
        ));
    }

    response
        .json::<OAuthTokenResponse>()
        .await
        .map_err(|e| format!("Invalid token response format: {e}"))
}

pub(crate) async fn fetch_userinfo(
    url: String,
    access_token: String,
) -> Result<serde_json::Value, String> {
    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|e| format!("Userinfo request failed: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "<no body>".to_string());
        return Err(format!("Userinfo failed with status {status}: {body}"));
    }

    response
        .json::<serde_json::Value>()
        .await
        .map_err(|e| format!("Invalid userinfo response format: {e}"))
}

/// The provider's user id, read out of the ID token instead of userinfo.
///
/// This used to be `let _ = token; None` — a stub. It is the `.or_else(...)`
/// fallback for the case where the userinfo endpoint yields no subject, which
/// is exactly what happens with an OpenID Connect provider that publishes
/// identity only in the ID token, or none at all. With the stub in place such
/// a provider could configure cleanly, redirect correctly, exchange its code
/// successfully, and then fail every login with "identity_missing".
///
/// **The signature is not verified**, and that is deliberate and bounded:
/// `token` here is the body of an HTTPS response to a request *we* made, to
/// the provider's configured token endpoint, authenticated with our own
/// client secret. TLS has already established who sent these bytes, which is
/// why OpenID Connect Core §3.1.3.7 permits skipping signature validation for
/// tokens obtained directly from the token endpoint. Every other origin — a
/// token posted by a browser, forwarded by another service, or read out of a
/// URL fragment — is attacker-chosen, and reading `sub` from one of those
/// unverified would let the attacker choose whose account they log into. The
/// `_unverified` suffix on the callee is there so a future call site from one
/// of those places has to be written on purpose.
pub(crate) fn extract_provider_user_id_from_token(token: &OAuthTokenResponse) -> Option<String> {
    subject_from_id_token_unverified(token.id_token.as_deref()?)
}

/// The provider's configured scopes, with the SQL nulls dropped.
///
/// `oauth_providers.scopes` is a `text[]`, so Diesel hands it back as
/// `Vec<Option<String>>`. A null element is a row nobody meant to write, and
/// forwarding it as an empty scope makes some providers reject the whole
/// authorization request.
pub(crate) fn provider_scopes(provider: &OAuthProvider) -> Vec<String> {
    provider.scopes.iter().flatten().cloned().collect()
}

/// Turn a policy-level [`CookieSpec`] into the cookie the jar wants.
///
/// The single point where the rules in `thunderforge_axum_auth_core::session`
/// become a real cookie. Anything that builds a `Cookie` for authentication
/// by hand elsewhere has stepped around those rules.
pub(crate) fn cookie_from_spec(spec: CookieSpec) -> Cookie<'static> {
    let mut cookie = Cookie::new(spec.name, spec.value);
    cookie.set_path(spec.path);
    cookie.set_http_only(spec.http_only);
    if spec.same_site_strict {
        cookie.set_same_site(SameSite::Strict);
    }
    cookie.set_secure(spec.secure);
    cookie
}
