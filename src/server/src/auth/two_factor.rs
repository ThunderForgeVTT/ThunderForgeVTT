//! Two-factor enrolment, verification, and the two admin switches that make
//! it mandatory.

use super::*;

/// Spec 041 US2 (FR-006 … FR-011): the recovery codes issued at confirmation
/// and spent at the challenge. First tenant of the `two_factor/` module
/// directory research R11 calls for; the rest of this file follows it there as
/// the remaining stories land.
#[path = "two_factor/recovery.rs"]
pub(crate) mod recovery;

/// Spec 041 US4 (FR-012, FR-014): the deliberate way off, which did not exist
/// — `setup/start`'s side effect had been doing the job instead.
#[path = "two_factor/disable.rs"]
pub(crate) mod disable;
pub(crate) use recovery::*;

/// Spec 041 US4 (FR-019 … FR-022): who must hold a factor, what a sign-in does
/// about it, and the ticket that lets an account caught by a requirement enrol
/// at the moment it is asked to.
#[path = "two_factor/requirement.rs"]
pub(crate) mod requirement;
pub(crate) use requirement::*;

pub(crate) async fn two_factor_setup_start(
    State(state): State<AppState>,
    Json(request): Json<TwoFactorSetupStartRequest>,
) -> (StatusCode, Json<TwoFactorSetupStartResponse>) {
    // One flow, three entrances (FR-001a). Account settings and first-run
    // setup arrive with a username and a password; a sign-in that requires
    // enrolment arrives with the login challenge it was just handed, because
    // it has a correct password and no session and re-posting the password
    // from a challenge screen is not what the contract asks for. See
    // `authorise_enrolment`.
    let authorised = match authorise_enrolment(
        &state,
        request.username.as_deref(),
        request.password.as_deref(),
        request.challenge_id,
    )
    .await
    {
        Ok(value) => value,
        Err(error) => {
            return (
                error.code,
                Json(TwoFactorSetupStartResponse {
                    status: error.status,
                    message: error.message.to_string(),
                    otpauth_url: None,
                }),
            );
        }
    };
    let user_id = authorised.user_id;
    let username = authorised.username;

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
    let started_at = Utc::now().naive_utc();
    tokio::task::spawn_blocking(move || {
        // ADR-081 / spec 041 FR-013: an enrolment in progress lives *beside*
        // the live second factor, never on top of it.
        //
        // This used to write the new secret over the live one and set
        // `two_factor_enabled = false` in the same statement, on a password
        // alone — which made starting an enrolment a way to remove a
        // confirmed factor without ever proving possession of it. Nothing
        // live is touched here now; `two_factor_setup_confirm` promotes the
        // pending secret once a code has proved it works.
        diesel::update(users::table.filter(users::id.eq(user_id)))
            .set((
                users::two_factor_pending_secret_encrypted.eq(Some(encrypted_secret)),
                users::two_factor_pending_started_at.eq(Some(started_at)),
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
    cookies: Cookies,
    client: ClientDescription,
    State(state): State<AppState>,
    Json(request): Json<TwoFactorSetupConfirmRequest>,
) -> (StatusCode, Json<TwoFactorSetupConfirmResponse>) {
    let authorised = match authorise_enrolment(
        &state,
        request.username.as_deref(),
        request.password.as_deref(),
        request.challenge_id,
    )
    .await
    {
        Ok(value) => value,
        Err(error) => return confirm_error(error.code, error.status, error.message),
    };
    let user_id = authorised.user_id;
    let username = authorised.username;
    let ticket = authorised.ticket;

    let mut conn = state.db_pool.get().expect("Failed to get DB connection");
    let secret_encrypted = tokio::task::spawn_blocking(move || {
        users::table
            .filter(users::id.eq(user_id))
            .select(users::two_factor_pending_secret_encrypted)
            .first::<Option<String>>(&mut conn)
            .optional()
    })
    .await
    .expect("Failed to spawn blocking task")
    .expect("Failed to query DB")
    .flatten();

    // No enrolment in progress. Deliberately the same answer whether the
    // account has a live second factor or none at all: confirming is about the
    // enrolment that was started, and there was not one.
    let Some(secret_encrypted) = secret_encrypted else {
        return confirm_error(
            StatusCode::BAD_REQUEST,
            "two_factor_not_setup",
            "Start 2FA setup first",
        );
    };

    let encryption_key = match encryption_key_from_config_secret(&state.config.secret) {
        Ok(key) => key,
        Err(msg) => {
            return confirm_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "two_factor_error",
                msg.as_str(),
            );
        }
    };

    let secret = match decrypt_secret(&secret_encrypted, &encryption_key) {
        Ok(value) => value,
        Err(msg) => {
            return confirm_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "two_factor_error",
                msg.as_str(),
            );
        }
    };

    let now = Utc::now().naive_utc();
    match verify_totp_code(&username, &secret, &request.code) {
        Ok(true) => {
            // Spec 041 FR-006: confirmation is where the recovery codes are
            // issued. Hashing happens before the transaction opens — ten Argon2
            // hashes is not work to hold a row lock through — and the plaintext
            // goes into the response below and nowhere else.
            let (codes, rows) = match generate_recovery_code_rows(user_id) {
                Ok(pair) => pair,
                Err(msg) => {
                    return confirm_error(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "two_factor_error",
                        msg.as_str(),
                    );
                }
            };

            let mut conn = state.db_pool.get().expect("Failed to get DB connection");
            let committed = tokio::task::spawn_blocking(move || {
                conn.transaction::<_, diesel::result::Error, _>(|conn| {
                    // FR-019/FR-020: when a login challenge authorised this,
                    // spending it is part of the same commit that turns the
                    // factor on. Zero rows means somebody else spent it
                    // between the authorisation and here, and the enrolment
                    // rolls back rather than half-happening.
                    if let Some(ticket) = ticket
                        && consume_enrolment_ticket_sync(conn, ticket, now)? == 0
                    {
                        return Err(diesel::result::Error::RollbackTransaction);
                    }

                    // The code proved the pending secret works, so it becomes
                    // the live one and the pending slot is emptied. Until this
                    // statement runs, whatever the account had before is still
                    // what it has.
                    diesel::update(users::table.filter(users::id.eq(user_id)))
                        .set((
                            users::two_factor_secret_encrypted
                                .eq(users::two_factor_pending_secret_encrypted.nullable()),
                            users::two_factor_enabled.eq(true),
                            users::two_factor_confirmed_at.eq(Some(now)),
                            users::two_factor_pending_secret_encrypted.eq::<Option<String>>(None),
                            users::two_factor_pending_started_at
                                .eq::<Option<chrono::NaiveDateTime>>(None),
                        ))
                        .execute(conn)?;

                    // Same transaction as the promotion: an account is never
                    // enabled without a way back in, and never handed codes for
                    // an enrolment that did not take. Re-confirming replaces
                    // the previous set outright (FR-010).
                    replace_recovery_codes_sync(conn, user_id, &rows)
                })
            })
            .await
            .expect("Failed to spawn blocking task");

            if committed.is_err() {
                return confirm_error(
                    StatusCode::BAD_REQUEST,
                    "two_factor_challenge_invalid",
                    "2FA challenge is expired or already used",
                );
            }

            // FR-020: the person was signing in when they were sent here, so
            // finishing enrolment finishes the sign-in. Only the ticket
            // entrance gets a session — the settings entrance already has one
            // and the request that started it proved nothing about a browser.
            let signed_in = if ticket.is_some() {
                if let Err(msg) = issue_session_cookie(&state, &cookies, user_id, client).await {
                    return confirm_error(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "session_error",
                        msg.as_str(),
                    );
                }
                Some(true)
            } else {
                None
            };

            (
                StatusCode::OK,
                Json(TwoFactorSetupConfirmResponse {
                    status: "success",
                    message: "2FA enabled".to_string(),
                    confirmed_at: Some(now),
                    recovery_codes: Some(codes),
                    recovery_codes_notice: Some(RECOVERY_CODES_NOTICE.to_string()),
                    signed_in,
                }),
            )
        }
        Ok(false) => confirm_error(
            StatusCode::UNAUTHORIZED,
            "two_factor_invalid",
            "Invalid 2FA code",
        ),
        Err(msg) => confirm_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "two_factor_error",
            msg.as_str(),
        ),
    }
}

pub(crate) async fn two_factor_verify(
    cookies: Cookies,
    client: ClientDescription,
    State(state): State<AppState>,
    Json(request): Json<TwoFactorVerifyRequest>,
) -> (StatusCode, Json<TwoFactorVerifyResponse>) {
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
        return verify_error(
            StatusCode::BAD_REQUEST,
            "two_factor_challenge_invalid",
            "2FA challenge is invalid",
        );
    };

    if challenge.consumed_at.is_some() || challenge.expires_at <= now {
        return verify_error(
            StatusCode::BAD_REQUEST,
            "two_factor_challenge_invalid",
            "2FA challenge is expired or already used",
        );
    }

    let user_id = challenge.user_id;

    // FR-007: a challenge takes an authenticator code **or** a recovery code.
    // Both together is refused before either is evaluated — a client sending
    // both is asking for two chances counted as one attempt — and neither is
    // refused the same way a wrong code is, so a probe learns nothing about
    // whether the account has recovery codes at all (FR-018).
    let held = match (
        request.code.as_deref().filter(|v| !v.trim().is_empty()),
        request
            .recovery_code
            .as_deref()
            .filter(|v| !v.trim().is_empty()),
    ) {
        (Some(_), Some(_)) => {
            return verify_error(
                StatusCode::BAD_REQUEST,
                "two_factor_invalid",
                "Invalid 2FA code",
            );
        }
        (None, None) => Ok(false),
        (Some(code), None) => verify_two_factor_for_user(&state, user_id, code).await,
        // FR-008: spending is the conditional write inside this call, so a
        // code that two requests present at once is spent by exactly one.
        (None, Some(recovery_code)) => consume_recovery_code(&state, user_id, recovery_code).await,
    };

    match held {
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

            if let Err(msg) = issue_session_cookie(&state, &cookies, user_id, client).await {
                return verify_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "session_error",
                    msg.as_str(),
                );
            }

            // FR-011: how many codes are left, said at the sign-in rather than
            // saved for a settings page the person may never open. A failure
            // to count is not a reason to fail a sign-in that has already
            // succeeded — the fields simply go unsaid.
            let remaining = count_unspent_recovery_codes(&state, user_id).await.ok();

            (
                StatusCode::OK,
                Json(TwoFactorVerifyResponse {
                    status: "success",
                    message: "2FA verification succeeded".to_string(),
                    recovery_codes_remaining: remaining,
                    recovery_codes_low: remaining.map(recovery_codes_low),
                }),
            )
        }
        Ok(false) => verify_error(
            StatusCode::UNAUTHORIZED,
            "two_factor_invalid",
            "Invalid 2FA code",
        ),
        Err(msg) => verify_error(
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

/// Confirmation's refusals. Same `status`/`message` pair the shared
/// `error_response` produces, in the shape that can also carry recovery codes
/// on the one path that has them.
fn confirm_error(
    code: StatusCode,
    status: &'static str,
    message: &str,
) -> (StatusCode, Json<TwoFactorSetupConfirmResponse>) {
    (
        code,
        Json(TwoFactorSetupConfirmResponse {
            status,
            message: message.to_string(),
            confirmed_at: None,
            recovery_codes: None,
            recovery_codes_notice: None,
            signed_in: None,
        }),
    )
}

/// The challenge's refusals, in the shape that can also carry the
/// recovery-code count on the one path that has earned it.
fn verify_error(
    code: StatusCode,
    status: &'static str,
    message: &str,
) -> (StatusCode, Json<TwoFactorVerifyResponse>) {
    (
        code,
        Json(TwoFactorVerifyResponse {
            status,
            message: message.to_string(),
            recovery_codes_remaining: None,
            recovery_codes_low: None,
        }),
    )
}
