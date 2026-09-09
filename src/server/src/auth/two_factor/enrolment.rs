//! Turning a second factor **on**: the pending secret, the QR code, and the
//! confirmation that promotes one into a factor in force.
//!
//! Spec 041 US1 and US3. Moved here whole from `two_factor.rs` by T005 — pure
//! movement, no behaviour change — so that every task after it edits a file
//! small enough to read in one sitting.

use super::*;

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
                    qr: None,
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
                    qr: None,
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
                    qr: None,
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
    // FR-002. A `None` here costs the person a scan and nothing else: the
    // grouped, typeable secret is on the same screen, which is what a desktop
    // authenticator, a password manager or somebody with no camera uses.
    let qr = crate::qr::encode(&otpauth);

    (
        StatusCode::OK,
        Json(TwoFactorSetupStartResponse {
            status: "success",
            message: "2FA secret generated. Confirm with one OTP code to enable.".to_string(),
            otpauth_url: Some(otpauth),
            qr,
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
    // FR-016 applies here too, and this is the easiest place to forget it: the
    // code that *confirms* an enrolment is a live TOTP code, and one still
    // inside its window a moment later. Without spending its step, the code a
    // person types to finish enrolling could immediately be replayed to sign
    // in as them.
    match verify_and_spend_step(&state, user_id, &username, &secret, &request.code).await {
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
                    replace_recovery_codes_sync(conn, user_id, &rows)?;

                    // FR-015, in the transaction it describes. Two events,
                    // because they are two facts: a factor was enabled, and a
                    // set of codes was issued. Re-confirming an existing
                    // factor writes both again, which is correct — the codes
                    // it replaced are dead and the account holder should see
                    // that happen.
                    events::record_sync(
                        conn,
                        user_id,
                        Some(user_id),
                        events::event_type::ENROLLED,
                    )?;
                    events::record_sync(
                        conn,
                        user_id,
                        Some(user_id),
                        events::event_type::RECOVERY_CODES_ISSUED,
                    )
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
