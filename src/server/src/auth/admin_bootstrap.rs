//! The one-time bootstrap code that lets the first administrator exist, and
//! the OAuth-provisioned account it can create.

use super::*;

pub async fn ensure_admin_bootstrap_code(state: &AppState) -> Result<(), String> {
    let now = Utc::now().naive_utc();
    let bootstrap_code = random_setup_code();
    let bootstrap_code_hash = hash_password(&bootstrap_code)?;
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;

    let generated_code = tokio::task::spawn_blocking(move || -> Result<Option<String>, String> {
        let admin_exists = users::table
            .filter(users::is_admin.eq(true))
            .select(users::id)
            .first::<uuid::Uuid>(&mut conn)
            .optional()
            .map_err(|_| "Failed to query admin users".to_string())?;

        let existing = admin_bootstrap_setup::table
            .filter(admin_bootstrap_setup::id.eq(1))
            .select(AdminBootstrapSetup::as_select())
            .first::<AdminBootstrapSetup>(&mut conn)
            .optional()
            .map_err(|_| "Failed to load bootstrap setup state".to_string())?;

        if admin_exists.is_some() {
            if existing.is_some() {
                mark_admin_setup_complete_sync(&mut conn, now)?;
            } else {
                let new_row = NewAdminBootstrapSetup {
                    id: 1,
                    setup_completed_at: Some(now),
                    admin_code_hash: None,
                    admin_code_generated_at: None,
                    created_at: now,
                    updated_at: now,
                };
                diesel::insert_into(admin_bootstrap_setup::table)
                    .values(&new_row)
                    .execute(&mut conn)
                    .map_err(|_| "Failed to persist bootstrap setup state".to_string())?;
            }

            return Ok(None);
        }

        if existing.is_some() {
            diesel::update(admin_bootstrap_setup::table.filter(admin_bootstrap_setup::id.eq(1)))
                .set((
                    admin_bootstrap_setup::setup_completed_at
                        .eq::<Option<chrono::NaiveDateTime>>(None),
                    admin_bootstrap_setup::admin_code_hash.eq(Some(bootstrap_code_hash)),
                    admin_bootstrap_setup::admin_code_generated_at.eq(Some(now)),
                    admin_bootstrap_setup::updated_at.eq(now),
                ))
                .execute(&mut conn)
                .map_err(|_| "Failed to update bootstrap setup state".to_string())?;
        } else {
            let new_row = NewAdminBootstrapSetup {
                id: 1,
                setup_completed_at: None,
                admin_code_hash: Some(bootstrap_code_hash),
                admin_code_generated_at: Some(now),
                created_at: now,
                updated_at: now,
            };
            diesel::insert_into(admin_bootstrap_setup::table)
                .values(&new_row)
                .execute(&mut conn)
                .map_err(|_| "Failed to persist bootstrap setup state".to_string())?;
        }

        Ok(Some(bootstrap_code))
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())??;

    if let Some(bootstrap_code) = generated_code {
        tracing::warn!(
            "Initial admin setup is incomplete. To create an admin account, visit: http://127.0.0.1:5173/setup/{}",
            bootstrap_code
        );
    }

    Ok(())
}

pub(crate) async fn ensure_admin_setup_code_valid(
    state: &AppState,
    admin_code: &str,
) -> Result<(), (StatusCode, Json<OAuthResponse>)> {
    let code = admin_code.trim().to_string();
    if code.is_empty() {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Admin setup code is required",
        ));
    }

    let mut conn = state.db_pool.get().map_err(|_| {
        error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "setup_error",
            "Failed to get DB connection",
        )
    })?;

    let result = tokio::task::spawn_blocking(move || -> Result<Result<(), String>, String> {
        let admin_exists = users::table
            .filter(users::is_admin.eq(true))
            .select(users::id)
            .first::<uuid::Uuid>(&mut conn)
            .optional()
            .map_err(|_| "Failed to query admin users".to_string())?;
        if admin_exists.is_some() {
            return Ok(Err("Setup has already been completed".to_string()));
        }

        let setup = admin_bootstrap_setup::table
            .filter(admin_bootstrap_setup::id.eq(1))
            .select(AdminBootstrapSetup::as_select())
            .first::<AdminBootstrapSetup>(&mut conn)
            .optional()
            .map_err(|_| "Failed to load bootstrap setup state".to_string())?;

        let Some(setup) = setup else {
            return Ok(Err("Setup state is not initialized yet".to_string()));
        };

        if setup.setup_completed_at.is_some() {
            return Ok(Err("Setup has already been completed".to_string()));
        }

        let Some(admin_code_hash) = setup.admin_code_hash else {
            return Ok(Err("Bootstrap admin code is not active".to_string()));
        };

        let parsed_hash = PasswordHash::new(&admin_code_hash)
            .map_err(|_| "Stored bootstrap admin code hash is invalid".to_string())?;
        if Argon2::default()
            .verify_password(code.as_bytes(), &parsed_hash)
            .is_err()
        {
            return Ok(Err("Invalid bootstrap admin code".to_string()));
        }

        Ok(Ok(()))
    })
    .await
    .map_err(|_| {
        error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "setup_error",
            "Failed to spawn blocking task",
        )
    })
    .and_then(|r| {
        r.map_err(|msg| {
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "setup_error",
                msg.as_str(),
            )
        })
    })?;

    match result {
        Ok(()) => Ok(()),
        Err(msg) if msg == "Setup has already been completed" => Err(error_response(
            StatusCode::CONFLICT,
            "setup_complete",
            msg.as_str(),
        )),
        Err(msg) if msg == "Invalid bootstrap admin code" => Err(error_response(
            StatusCode::UNAUTHORIZED,
            "invalid_admin_code",
            msg.as_str(),
        )),
        Err(msg) => Err(error_response(
            StatusCode::BAD_REQUEST,
            "setup_unavailable",
            msg.as_str(),
        )),
    }
}

pub(crate) async fn load_and_consume_admin_bootstrap_oauth_session(
    state: &AppState,
    provider_key: &str,
    state_token: &str,
) -> Result<AdminBootstrapOAuthContext, (StatusCode, Json<OAuthResponse>)> {
    let mut conn = state.db_pool.get().expect("Failed to get DB connection");
    let provider_key = provider_key.to_string();
    let state_token = state_token.to_string();
    let now = Utc::now().naive_utc();

    let result = tokio::task::spawn_blocking(
        move || -> Result<Option<AdminBootstrapOAuthContext>, diesel::result::Error> {
            let session = admin_bootstrap_oauth_sessions::table
                .filter(admin_bootstrap_oauth_sessions::oauth_provider_key.eq(&provider_key))
                .filter(admin_bootstrap_oauth_sessions::state.eq(&state_token))
                .select(AdminBootstrapOAuthSession::as_select())
                .first::<AdminBootstrapOAuthSession>(&mut conn)
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
                admin_bootstrap_oauth_sessions::table
                    .filter(admin_bootstrap_oauth_sessions::id.eq(session.id)),
            )
            .set(admin_bootstrap_oauth_sessions::consumed_at.eq(Some(now)))
            .execute(&mut conn)?;

            Ok(Some(AdminBootstrapOAuthContext { provider, session }))
        },
    )
    .await
    .expect("Failed to spawn blocking task")
    .expect("Failed to query bootstrap oauth authorization session");

    result.ok_or_else(|| {
        error_response(
            StatusCode::BAD_REQUEST,
            "invalid_oauth_state",
            "Bootstrap OAuth state is invalid, expired, or already consumed",
        )
    })
}

pub(crate) async fn create_admin_user_from_oauth(
    state: &AppState,
    provider_id: uuid::Uuid,
    provider_user_id: String,
    provider_email: Option<String>,
    desired_username: Option<String>,
    token_response: OAuthTokenResponse,
) -> Result<uuid::Uuid, (StatusCode, Json<OAuthResponse>)> {
    let Some(provider_email) = provider_email.map(|v| v.trim().to_lowercase()) else {
        return Err(error_response(
            StatusCode::BAD_GATEWAY,
            "email_missing",
            "OAuth provider did not return an email address for bootstrap setup",
        ));
    };

    let username = derive_bootstrap_username(desired_username, &provider_email);
    if username.is_empty() {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "invalid_username",
            "A valid username is required for bootstrap OAuth setup",
        ));
    }

    let encryption_key =
        encryption_key_from_config_secret(&state.config.secret).map_err(|msg| {
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "encryption_key_invalid",
                msg.as_str(),
            )
        })?;

    let access_token_encrypted = encrypt_secret(&token_response.access_token, &encryption_key)
        .map_err(|msg| {
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "encryption_failed",
                msg.as_str(),
            )
        })?;
    let refresh_token_encrypted = token_response
        .refresh_token
        .as_deref()
        .map(|value| encrypt_secret(value, &encryption_key))
        .transpose()
        .map_err(|msg| {
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "encryption_failed",
                msg.as_str(),
            )
        })?;
    let token_expires_at = token_response
        .expires_in
        .map(|seconds| Utc::now() + chrono::Duration::seconds(seconds))
        .map(|v| v.naive_utc());
    let random_password_hash = hash_password(&random_urlsafe(48)).map_err(|msg| {
        error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "password_hash_failed",
            msg.as_str(),
        )
    })?;

    let now = Utc::now().naive_utc();
    let mut conn = state.db_pool.get().map_err(|_| {
        error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "setup_error",
            "Failed to get DB connection",
        )
    })?;

    tokio::task::spawn_blocking(move || -> Result<uuid::Uuid, String> {
        let admin_exists = users::table
            .filter(users::is_admin.eq(true))
            .select(users::id)
            .first::<uuid::Uuid>(&mut conn)
            .optional()
            .map_err(|_| "Failed to query existing admins".to_string())?;
        if admin_exists.is_some() {
            return Err("Setup has already been completed".to_string());
        }

        let existing_user_by_email = users::table
            .filter(users::email.eq(&provider_email))
            .select(users::id)
            .first::<uuid::Uuid>(&mut conn)
            .optional()
            .map_err(|_| "Failed to validate provider email".to_string())?;
        if existing_user_by_email.is_some() {
            return Err("Email is already in use".to_string());
        }

        let existing_user_by_username = users::table
            .filter(users::username.eq(&username))
            .select(users::id)
            .first::<uuid::Uuid>(&mut conn)
            .optional()
            .map_err(|_| "Failed to validate username".to_string())?;
        if existing_user_by_username.is_some() {
            return Err("Username is already in use".to_string());
        }

        let existing_link = user_oauth_accounts::table
            .filter(user_oauth_accounts::provider_id.eq(provider_id))
            .filter(user_oauth_accounts::provider_user_id.eq(&provider_user_id))
            .select(UserOAuthAccount::as_select())
            .first::<UserOAuthAccount>(&mut conn)
            .optional()
            .map_err(|_| "Failed to validate OAuth link".to_string())?;
        if existing_link.is_some() {
            return Err("OAuth account is already linked".to_string());
        }

        let user_id = uuid::Uuid::now_v7();
        diesel::insert_into(users::table)
            .values((
                users::id.eq(user_id),
                users::username.eq(username),
                users::email.eq(provider_email.clone()),
                users::is_admin.eq(true),
                users::password_hash.eq(random_password_hash),
                users::created_at.eq(now),
                users::updated_at.eq(now),
                users::two_factor_enabled.eq(false),
                users::two_factor_secret_encrypted.eq::<Option<String>>(None),
                users::two_factor_confirmed_at.eq::<Option<chrono::NaiveDateTime>>(None),
                users::two_factor_admin_required.eq(false),
            ))
            .execute(&mut conn)
            .map_err(|_| "Failed to create admin user".to_string())?;

        let oauth_account = NewUserOAuthAccount {
            id: uuid::Uuid::now_v7(),
            user_id,
            provider_id,
            provider_user_id,
            provider_email: Some(provider_email),
            access_token_encrypted: Some(access_token_encrypted),
            refresh_token_encrypted,
            token_expires_at,
            linked_at: now,
            created_at: now,
            updated_at: now,
        };

        diesel::insert_into(user_oauth_accounts::table)
            .values(&oauth_account)
            .execute(&mut conn)
            .map_err(|_| "Failed to link OAuth account".to_string())?;

        mark_admin_setup_complete_sync(&mut conn, now)?;

        Ok(user_id)
    })
    .await
    .map_err(|_| {
        error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "setup_error",
            "Failed to spawn blocking task",
        )
    })
    .and_then(|r| match r {
        Ok(user_id) => Ok(user_id),
        Err(msg) if msg == "Setup has already been completed" => Err(error_response(
            StatusCode::CONFLICT,
            "setup_complete",
            msg.as_str(),
        )),
        Err(msg)
            if msg == "Email is already in use"
                || msg == "Username is already in use"
                || msg == "OAuth account is already linked" =>
        {
            Err(error_response(
                StatusCode::CONFLICT,
                "setup_conflict",
                msg.as_str(),
            ))
        }
        Err(msg) => Err(error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "setup_error",
            msg.as_str(),
        )),
    })
}

pub(crate) fn mark_admin_setup_complete_sync(
    conn: &mut diesel::PgConnection,
    now: chrono::NaiveDateTime,
) -> Result<(), String> {
    diesel::update(admin_bootstrap_setup::table.filter(admin_bootstrap_setup::id.eq(1)))
        .set((
            admin_bootstrap_setup::setup_completed_at.eq(Some(now)),
            admin_bootstrap_setup::admin_code_hash.eq::<Option<String>>(None),
            admin_bootstrap_setup::admin_code_generated_at
                .eq::<Option<chrono::NaiveDateTime>>(None),
            admin_bootstrap_setup::updated_at.eq(now),
        ))
        .execute(conn)
        .map_err(|_| "Failed to update bootstrap setup state".to_string())?;

    Ok(())
}
