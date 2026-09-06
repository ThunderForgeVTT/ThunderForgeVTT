//! First-run admin setup: the status probe, the basic-credentials path, the
//! OAuth path, and the check every admin-only request runs.

use super::*;

pub(crate) async fn setup_status(
    State(state): State<AppState>,
) -> (StatusCode, Json<SetupStatusResponse>) {
    let mut conn = state.db_pool.get().expect("Failed to get DB connection");

    let result = tokio::task::spawn_blocking(move || {
        let admin_exists = users::table
            .filter(users::is_admin.eq(true))
            .select(users::id)
            .first::<uuid::Uuid>(&mut conn)
            .optional()?;

        let setup = admin_bootstrap_setup::table
            .filter(admin_bootstrap_setup::id.eq(1))
            .select(AdminBootstrapSetup::as_select())
            .first::<AdminBootstrapSetup>(&mut conn)
            .optional()?;

        let providers = oauth_providers::table
            .filter(oauth_providers::enabled.eq(true))
            .filter(oauth_providers::configured.eq(true))
            .select((oauth_providers::provider_key, oauth_providers::display_name))
            .load::<(String, String)>(&mut conn)?;

        Ok::<_, diesel::result::Error>((admin_exists.is_some(), setup, providers))
    })
    .await
    .expect("Failed to spawn blocking task")
    .expect("Failed to query setup status");

    let (admin_exists, setup, providers) = result;
    let setup_completed = admin_exists || setup.and_then(|v| v.setup_completed_at).is_some();

    (
        StatusCode::OK,
        Json(SetupStatusResponse {
            setup_required: !setup_completed,
            setup_completed,
            configured_oauth_providers: providers
                .into_iter()
                .map(|(provider_key, display_name)| SetupOAuthProvider {
                    provider_key,
                    display_name,
                })
                .collect(),
        }),
    )
}

pub(crate) async fn admin_setup_basic(
    cookies: Cookies,
    State(state): State<AppState>,
    Json(request): Json<AdminSetupBasicRequest>,
) -> (StatusCode, Json<OAuthResponse>) {
    if let Err(resp) = ensure_admin_setup_code_valid(&state, &request.admin_code).await {
        return resp;
    }

    let username = request.username.trim().to_string();
    let email = request.email.trim().to_lowercase();
    if username.is_empty() || email.is_empty() || request.password.is_empty() {
        return error_response(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Username, email, and password are required",
        );
    }

    let password_hash = match hash_password(&request.password) {
        Ok(v) => v,
        Err(msg) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "password_hash_failed",
                msg.as_str(),
            );
        }
    };

    let now = Utc::now().naive_utc();
    let mut conn = state.db_pool.get().expect("Failed to get DB connection");
    let result = tokio::task::spawn_blocking(move || -> Result<uuid::Uuid, String> {
        let admin_exists = users::table
            .filter(users::is_admin.eq(true))
            .select(users::id)
            .first::<uuid::Uuid>(&mut conn)
            .optional()
            .map_err(|_| "Failed to query existing admins".to_string())?;
        if admin_exists.is_some() {
            return Err("Setup has already been completed".to_string());
        }

        let username_exists = users::table
            .filter(users::username.eq(&username))
            .select(users::id)
            .first::<uuid::Uuid>(&mut conn)
            .optional()
            .map_err(|_| "Failed to validate username".to_string())?;
        if username_exists.is_some() {
            return Err("Username is already in use".to_string());
        }

        let email_exists = users::table
            .filter(users::email.eq(&email))
            .select(users::id)
            .first::<uuid::Uuid>(&mut conn)
            .optional()
            .map_err(|_| "Failed to validate email".to_string())?;
        if email_exists.is_some() {
            return Err("Email is already in use".to_string());
        }

        let user_id = uuid::Uuid::now_v7();
        diesel::insert_into(users::table)
            .values((
                users::id.eq(user_id),
                users::username.eq(username),
                users::email.eq(email),
                users::is_admin.eq(true),
                users::password_hash.eq(password_hash),
                users::created_at.eq(now),
                users::updated_at.eq(now),
                users::two_factor_enabled.eq(false),
                users::two_factor_secret_encrypted.eq::<Option<String>>(None),
                users::two_factor_confirmed_at.eq::<Option<chrono::NaiveDateTime>>(None),
                users::two_factor_admin_required.eq(false),
            ))
            .execute(&mut conn)
            .map_err(|_| "Failed to create admin user".to_string())?;

        mark_admin_setup_complete_sync(&mut conn, now)?;

        Ok(user_id)
    })
    .await
    .expect("Failed to spawn blocking task");

    let user_id = match result {
        Ok(v) => v,
        Err(msg) if msg == "Setup has already been completed" => {
            return error_response(StatusCode::CONFLICT, "setup_complete", msg.as_str());
        }
        Err(msg) if msg == "Username is already in use" || msg == "Email is already in use" => {
            return error_response(StatusCode::CONFLICT, "setup_conflict", msg.as_str());
        }
        Err(msg) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "setup_error",
                msg.as_str(),
            );
        }
    };

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
            message: "Initial admin account created successfully".to_string(),
            challenge_id: None,
            login_two_factor_challenge_id: None,
        }),
    )
}

pub(crate) async fn admin_setup_oauth_start(
    Path(provider_key): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<AdminSetupOAuthStartRequest>,
) -> Result<(StatusCode, Json<AdminSetupOAuthStartResponse>), (StatusCode, Json<OAuthResponse>)> {
    ensure_admin_setup_code_valid(&state, &request.admin_code).await?;

    let mut conn = state.db_pool.get().expect("Failed to get DB connection");
    let provider_key_clone = provider_key.clone();
    let now = Utc::now().naive_utc();
    let state_token = generate_state();
    let code_verifier = generate_code_verifier();

    let provider = tokio::task::spawn_blocking(move || {
        oauth_providers::table
            .filter(oauth_providers::provider_key.eq(provider_key_clone))
            .filter(oauth_providers::enabled.eq(true))
            .filter(oauth_providers::configured.eq(true))
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

    let Some(provider_client_id) = provider.oauth_client_id.clone() else {
        return Err(error_response(
            StatusCode::CONFLICT,
            "provider_not_configured",
            "Provider client id is not set",
        ));
    };

    let session = NewAdminBootstrapOAuthSession {
        id: uuid::Uuid::now_v7(),
        provider_id: provider.id,
        oauth_provider_key: provider_key.clone(),
        oauth_client_id: provider_client_id.clone(),
        state: state_token.clone(),
        code_verifier: code_verifier.clone(),
        redirect_uri: request.redirect_uri.clone(),
        desired_username: request.username,
        return_to: request.return_to,
        expires_at: now + chrono::Duration::minutes(10),
        consumed_at: None,
        created_at: now,
    };

    let mut conn = state.db_pool.get().expect("Failed to get DB connection");
    tokio::task::spawn_blocking(move || {
        diesel::insert_into(admin_bootstrap_oauth_sessions::table)
            .values(&session)
            .execute(&mut conn)
    })
    .await
    .expect("Failed to spawn blocking task")
    .expect("Failed to persist bootstrap oauth session");

    let authorization_url = build_authorize_url(&AuthorizeRequest {
        authorization_url: &provider.authorization_url,
        client_id: &provider_client_id,
        redirect_uri: &request.redirect_uri,
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

    Ok((
        StatusCode::OK,
        Json(AdminSetupOAuthStartResponse { authorization_url }),
    ))
}

pub(crate) async fn admin_setup_oauth_callback(
    Path(provider_key): Path<String>,
    Query(query): Query<OAuthCallbackQuery>,
    cookies: Cookies,
    State(state): State<AppState>,
) -> axum::response::Response {
    if let Some(err) = query.error {
        let message = format!(
            "Provider returned error '{}': {}",
            err,
            query
                .error_description
                .unwrap_or_else(|| "unknown".to_string())
        );
        return bootstrap_error_redirect(&message);
    }

    let Some(code) = query.code else {
        return bootstrap_error_redirect("Missing 'code' query parameter");
    };
    let Some(state_token) = query.state else {
        return bootstrap_error_redirect("Missing 'state' query parameter");
    };

    let auth_ctx =
        match load_and_consume_admin_bootstrap_oauth_session(&state, &provider_key, &state_token)
            .await
        {
            Ok(v) => v,
            Err(_) => {
                return bootstrap_error_redirect("Bootstrap OAuth state is invalid or expired");
            }
        };

    let token_response = match exchange_authorization_code_with_provider(
        &auth_ctx.provider,
        &auth_ctx.session.redirect_uri,
        &auth_ctx.session.code_verifier,
        &code,
    )
    .await
    {
        Ok(tokens) => tokens,
        Err(msg) => {
            return bootstrap_error_redirect(msg.as_str());
        }
    };

    let userinfo = if let Some(userinfo_url) = auth_ctx.provider.userinfo_url.clone() {
        match fetch_userinfo(userinfo_url, token_response.access_token.clone()).await {
            Ok(v) => Some(v),
            Err(msg) => {
                return bootstrap_error_redirect(msg.as_str());
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
        return bootstrap_error_redirect(
            "Could not extract provider user id from provider response",
        );
    };

    let provider_email = userinfo.as_ref().and_then(extract_provider_email);
    let desired_username = auth_ctx.session.desired_username.clone();
    let return_to = auth_ctx.session.return_to.clone();
    let user_id = match create_admin_user_from_oauth(
        &state,
        auth_ctx.provider.id,
        provider_user_id,
        provider_email,
        desired_username,
        token_response,
    )
    .await
    {
        Ok(v) => v,
        Err((_, payload)) => return bootstrap_error_redirect(payload.message.as_str()),
    };

    if let Err(msg) = issue_session_cookie(&state, &cookies, user_id).await {
        return bootstrap_error_redirect(msg.as_str());
    }

    if let Some(return_to) = return_to
        && let Ok(url) = Url::parse(&return_to)
    {
        return Redirect::temporary(url.as_str()).into_response();
    }

    (
        StatusCode::OK,
        Json(OAuthResponse {
            status: "success",
            message: "Initial admin account created successfully via OAuth".to_string(),
            challenge_id: None,
            login_two_factor_challenge_id: None,
        }),
    )
        .into_response()
}

pub(crate) fn bootstrap_error_redirect(message: &str) -> axum::response::Response {
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    serializer.append_pair("oauth_error", message);
    let query = serializer.finish();
    let target = format!("/setup/callback?{query}");
    Redirect::temporary(target.as_str()).into_response()
}

pub(crate) async fn verify_admin_request(
    state: &AppState,
    cookies: &Cookies,
) -> Result<(), (StatusCode, Json<OAuthResponse>)> {
    let Some(session_cookie) = cookies.private(&state.key).get("session") else {
        return Err(error_response(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "Authentication required",
        ));
    };

    let Ok(session_id) = uuid::Uuid::parse_str(session_cookie.value()) else {
        return Err(error_response(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "Invalid session",
        ));
    };

    let now = Utc::now().naive_utc();
    let mut conn = state.db_pool.get().map_err(|_| {
        error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "session_error",
            "Failed to get DB connection",
        )
    })?;

    let is_admin = tokio::task::spawn_blocking(move || {
        user_sessions::table
            .inner_join(users::table.on(users::id.eq(user_sessions::user_id)))
            .filter(user_sessions::id.eq(session_id))
            .filter(user_sessions::revoked_at.is_null())
            .filter(user_sessions::expires_at.gt(now))
            .select(users::is_admin)
            .first::<bool>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| {
        error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "session_error",
            "Failed to verify admin session",
        )
    })
    .and_then(|r| {
        r.map_err(|_| {
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "session_error",
                "Failed to verify admin session",
            )
        })
    })?;

    if is_admin != Some(true) {
        return Err(error_response(
            StatusCode::FORBIDDEN,
            "forbidden",
            "Admin privileges required",
        ));
    }

    Ok(())
}
