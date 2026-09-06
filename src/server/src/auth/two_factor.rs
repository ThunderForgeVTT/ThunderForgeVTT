//! Two-factor enrolment, verification, and the two admin switches that make
//! it mandatory.

use super::*;

pub(crate) async fn two_factor_setup_start(
    State(state): State<AppState>,
    Json(request): Json<TwoFactorSetupStartRequest>,
) -> (StatusCode, Json<TwoFactorSetupStartResponse>) {
    let username = request.username.clone();
    let username_for_query = username.clone();
    let mut conn = state.db_pool.get().expect("Failed to get DB connection");

    let user = tokio::task::spawn_blocking(move || {
        users::table
            .filter(users::username.eq(&username_for_query))
            .select((users::id, users::password_hash))
            .first::<(uuid::Uuid, String)>(&mut conn)
            .optional()
    })
    .await
    .expect("Failed to spawn blocking task")
    .expect("Failed to query DB");

    let Some((user_id, password_hash)) = user else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(TwoFactorSetupStartResponse {
                status: "failure",
                message: "Invalid credentials".to_string(),
                otpauth_url: None,
            }),
        );
    };

    let parsed_hash = PasswordHash::new(&password_hash).expect("Invalid hash in db");
    if Argon2::default()
        .verify_password(request.password.as_bytes(), &parsed_hash)
        .is_err()
    {
        return (
            StatusCode::UNAUTHORIZED,
            Json(TwoFactorSetupStartResponse {
                status: "failure",
                message: "Invalid credentials".to_string(),
                otpauth_url: None,
            }),
        );
    }

    let secret_base32 = {
        let mut secret_bytes = [0u8; 20];
        let mut rng = rand::rng();
        rng.fill(&mut secret_bytes);
        BASE32_NOPAD.encode(&secret_bytes)
    };

    let encryption_key = match encryption_key_from_config_secret(&state.config.secret) {
        Ok(key) => key,
        Err(msg) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(TwoFactorSetupStartResponse {
                    status: "error",
                    message: msg,
                    otpauth_url: None,
                }),
            );
        }
    };

    let encrypted_secret = match encrypt_secret(&secret_base32, &encryption_key) {
        Ok(value) => value,
        Err(msg) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(TwoFactorSetupStartResponse {
                    status: "error",
                    message: msg,
                    otpauth_url: None,
                }),
            );
        }
    };

    let mut conn = state.db_pool.get().expect("Failed to get DB connection");
    tokio::task::spawn_blocking(move || {
        diesel::update(users::table.filter(users::id.eq(user_id)))
            .set((
                users::two_factor_secret_encrypted.eq(Some(encrypted_secret)),
                users::two_factor_enabled.eq(false),
                users::two_factor_confirmed_at.eq::<Option<chrono::NaiveDateTime>>(None),
            ))
            .execute(&mut conn)
    })
    .await
    .expect("Failed to spawn blocking task")
    .expect("Failed to update user 2FA secret");

    let otpauth = format!(
        "otpauth://totp/ThunderForge:{}?secret={}&issuer=ThunderForge",
        username, secret_base32
    );

    (
        StatusCode::OK,
        Json(TwoFactorSetupStartResponse {
            status: "success",
            message: "2FA secret generated. Confirm with one OTP code to enable.".to_string(),
            otpauth_url: Some(otpauth),
        }),
    )
}

pub(crate) async fn two_factor_setup_confirm(
    State(state): State<AppState>,
    Json(request): Json<TwoFactorSetupConfirmRequest>,
) -> (StatusCode, Json<OAuthResponse>) {
    let username = request.username.clone();
    let username_for_query = username.clone();
    let mut conn = state.db_pool.get().expect("Failed to get DB connection");

    let user = tokio::task::spawn_blocking(move || {
        users::table
            .filter(users::username.eq(&username_for_query))
            .select((
                users::id,
                users::password_hash,
                users::two_factor_secret_encrypted,
            ))
            .first::<(uuid::Uuid, String, Option<String>)>(&mut conn)
            .optional()
    })
    .await
    .expect("Failed to spawn blocking task")
    .expect("Failed to query DB");

    let Some((user_id, password_hash, secret_encrypted)) = user else {
        return error_response(StatusCode::UNAUTHORIZED, "failure", "Invalid credentials");
    };

    let parsed_hash = PasswordHash::new(&password_hash).expect("Invalid hash in db");
    if Argon2::default()
        .verify_password(request.password.as_bytes(), &parsed_hash)
        .is_err()
    {
        return error_response(StatusCode::UNAUTHORIZED, "failure", "Invalid credentials");
    }

    let Some(secret_encrypted) = secret_encrypted else {
        return error_response(
            StatusCode::BAD_REQUEST,
            "two_factor_not_setup",
            "Start 2FA setup first",
        );
    };

    let encryption_key = match encryption_key_from_config_secret(&state.config.secret) {
        Ok(key) => key,
        Err(msg) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "two_factor_error",
                msg.as_str(),
            );
        }
    };

    let secret = match decrypt_secret(&secret_encrypted, &encryption_key) {
        Ok(value) => value,
        Err(msg) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "two_factor_error",
                msg.as_str(),
            );
        }
    };

    let now = Utc::now().naive_utc();
    match verify_totp_code(&username, &secret, &request.code) {
        Ok(true) => {
            let mut conn = state.db_pool.get().expect("Failed to get DB connection");
            tokio::task::spawn_blocking(move || {
                diesel::update(users::table.filter(users::id.eq(user_id)))
                    .set((
                        users::two_factor_enabled.eq(true),
                        users::two_factor_confirmed_at.eq(Some(now)),
                    ))
                    .execute(&mut conn)
            })
            .await
            .expect("Failed to spawn blocking task")
            .expect("Failed to enable 2FA");

            (
                StatusCode::OK,
                Json(OAuthResponse {
                    status: "success",
                    message: "2FA enabled".to_string(),
                    challenge_id: None,
                    login_two_factor_challenge_id: None,
                }),
            )
        }
        Ok(false) => error_response(
            StatusCode::UNAUTHORIZED,
            "two_factor_invalid",
            "Invalid 2FA code",
        ),
        Err(msg) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "two_factor_error",
            msg.as_str(),
        ),
    }
}

pub(crate) async fn two_factor_verify(
    cookies: Cookies,
    State(state): State<AppState>,
    Json(request): Json<TwoFactorVerifyRequest>,
) -> (StatusCode, Json<OAuthResponse>) {
    let now = Utc::now().naive_utc();
    let mut conn = state.db_pool.get().expect("Failed to get DB connection");

    let challenge = tokio::task::spawn_blocking(move || {
        login_two_factor_challenges::table
            .filter(login_two_factor_challenges::id.eq(request.challenge_id))
            .select(LoginTwoFactorChallenge::as_select())
            .first::<LoginTwoFactorChallenge>(&mut conn)
            .optional()
    })
    .await
    .expect("Failed to spawn blocking task")
    .expect("Failed to query 2FA challenge");

    let Some(challenge) = challenge else {
        return error_response(
            StatusCode::BAD_REQUEST,
            "two_factor_challenge_invalid",
            "2FA challenge is invalid",
        );
    };

    if challenge.consumed_at.is_some() || challenge.expires_at <= now {
        return error_response(
            StatusCode::BAD_REQUEST,
            "two_factor_challenge_invalid",
            "2FA challenge is expired or already used",
        );
    }

    let user_id = challenge.user_id;
    match verify_two_factor_for_user(&state, user_id, &request.code).await {
        Ok(true) => {
            let mut conn = state.db_pool.get().expect("Failed to get DB connection");
            tokio::task::spawn_blocking(move || {
                diesel::update(
                    login_two_factor_challenges::table
                        .filter(login_two_factor_challenges::id.eq(challenge.id)),
                )
                .set(login_two_factor_challenges::consumed_at.eq(Some(now)))
                .execute(&mut conn)
            })
            .await
            .expect("Failed to spawn blocking task")
            .expect("Failed to consume 2FA challenge");

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
                    message: "2FA verification succeeded".to_string(),
                    challenge_id: None,
                    login_two_factor_challenge_id: None,
                }),
            )
        }
        Ok(false) => error_response(
            StatusCode::UNAUTHORIZED,
            "two_factor_invalid",
            "Invalid 2FA code",
        ),
        Err(msg) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "two_factor_error",
            msg.as_str(),
        ),
    }
}

pub(crate) async fn set_admin_two_factor_requirement(
    cookies: Cookies,
    State(state): State<AppState>,
    Json(request): Json<AdminTwoFactorRequirementRequest>,
) -> (StatusCode, Json<OAuthResponse>) {
    if let Err(resp) = verify_admin_request(&state, &cookies).await {
        return resp;
    }

    let now = Utc::now().naive_utc();
    let mut conn = state.db_pool.get().expect("Failed to get DB connection");
    let required = request.required_for_all_users;

    let result = tokio::task::spawn_blocking(move || {
        diesel::update(auth_security_settings::table.filter(auth_security_settings::id.eq(1)))
            .set((
                auth_security_settings::two_factor_required_for_all_users.eq(required),
                auth_security_settings::updated_at.eq(now),
            ))
            .execute(&mut conn)
    })
    .await
    .expect("Failed to spawn blocking task");

    if result.is_err() {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "settings_error",
            "Failed to update global 2FA requirement",
        );
    }

    (
        StatusCode::OK,
        Json(OAuthResponse {
            status: "success",
            message: "Global 2FA requirement updated".to_string(),
            challenge_id: None,
            login_two_factor_challenge_id: None,
        }),
    )
}

pub(crate) async fn set_admin_user_two_factor_required(
    cookies: Cookies,
    Path(user_id): Path<uuid::Uuid>,
    State(state): State<AppState>,
    Json(request): Json<AdminUserTwoFactorRequiredRequest>,
) -> (StatusCode, Json<OAuthResponse>) {
    if let Err(resp) = verify_admin_request(&state, &cookies).await {
        return resp;
    }

    let mut conn = state.db_pool.get().expect("Failed to get DB connection");
    let required = request.required;
    let result = tokio::task::spawn_blocking(move || {
        diesel::update(users::table.filter(users::id.eq(user_id)))
            .set(users::two_factor_admin_required.eq(required))
            .execute(&mut conn)
    })
    .await
    .expect("Failed to spawn blocking task");

    match result {
        Ok(0) => error_response(StatusCode::NOT_FOUND, "not_found", "User not found"),
        Ok(_) => (
            StatusCode::OK,
            Json(OAuthResponse {
                status: "success",
                message: "User 2FA requirement updated".to_string(),
                challenge_id: None,
                login_two_factor_challenge_id: None,
            }),
        ),
        Err(_) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "settings_error",
            "Failed to update user 2FA requirement",
        ),
    }
}

pub(crate) async fn load_global_two_factor_requirement(state: &AppState) -> Result<bool, String> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection")?;
    let result = tokio::task::spawn_blocking(move || {
        auth_security_settings::table
            .filter(auth_security_settings::id.eq(1))
            .select(AuthSecuritySetting::as_select())
            .first::<AuthSecuritySetting>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())
    .and_then(|r| r.map_err(|_| "Failed to query auth security settings".to_string()))?;

    Ok(result
        .map(|s| s.two_factor_required_for_all_users)
        .unwrap_or(false))
}

pub(crate) async fn is_two_factor_required_for_user(
    state: &AppState,
    user_id: uuid::Uuid,
) -> Result<bool, String> {
    let global_required = load_global_two_factor_requirement(state).await?;
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection")?;
    let local = tokio::task::spawn_blocking(move || {
        users::table
            .filter(users::id.eq(user_id))
            .select((users::two_factor_enabled, users::two_factor_admin_required))
            .first::<(bool, bool)>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())
    .and_then(|r| r.map_err(|_| "Failed to query user 2FA state".to_string()))?;

    let Some((enabled, admin_required)) = local else {
        return Ok(false);
    };

    Ok(global_required || enabled || admin_required)
}

pub(crate) async fn create_login_two_factor_challenge(
    state: &AppState,
    user_id: uuid::Uuid,
) -> Result<uuid::Uuid, String> {
    let now = Utc::now().naive_utc();
    let challenge_id = uuid::Uuid::now_v7();
    let challenge = NewLoginTwoFactorChallenge {
        id: challenge_id,
        user_id,
        expires_at: now + chrono::Duration::minutes(10),
        consumed_at: None,
        created_at: now,
    };

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection")?;
    tokio::task::spawn_blocking(move || {
        diesel::insert_into(login_two_factor_challenges::table)
            .values(&challenge)
            .execute(&mut conn)
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())
    .and_then(|r| r.map_err(|_| "Failed to create 2FA challenge".to_string()))?;

    Ok(challenge_id)
}

pub(crate) async fn verify_two_factor_for_user(
    state: &AppState,
    user_id: uuid::Uuid,
    code: &str,
) -> Result<bool, String> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection")?;
    let user = tokio::task::spawn_blocking(move || {
        users::table
            .filter(users::id.eq(user_id))
            .select((users::username, users::two_factor_secret_encrypted))
            .first::<(String, Option<String>)>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())
    .and_then(|r| r.map_err(|_| "Failed to query user for 2FA".to_string()))?;

    let Some((username, secret_encrypted)) = user else {
        return Ok(false);
    };
    let Some(secret_encrypted) = secret_encrypted else {
        return Ok(false);
    };

    let encryption_key = encryption_key_from_config_secret(&state.config.secret)?;
    let secret = decrypt_secret(&secret_encrypted, &encryption_key)?;
    verify_totp_code(&username, &secret, code)
}
