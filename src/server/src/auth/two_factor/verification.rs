//! Proving a second factor already held: the challenge, the TOTP step it
//! spends, and the login ticket that carries a half-finished sign-in.
//!
//! Spec 041 US2 and FR-016. Moved here whole from `two_factor.rs` by T005 —
//! pure movement, no behaviour change.

use super::*;

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
        (None, None) => Ok(throttle::SecondFactor::Refused),
        // FR-017: the bound is inside `guarded`, so it applies to both kinds
        // of credential and to every route that verifies one.
        (Some(code), None) => {
            throttle::guarded(&state, user_id, || {
                verify_two_factor_for_user(&state, user_id, code)
            })
            .await
        }
        // FR-008: spending is the conditional write inside this call, so a
        // code that two requests present at once is spent by exactly one.
        (None, Some(recovery_code)) => {
            throttle::guarded(&state, user_id, || {
                consume_recovery_code(&state, user_id, recovery_code)
            })
            .await
        }
    };

    if matches!(held, Ok(throttle::SecondFactor::Throttled)) {
        return verify_error(
            StatusCode::TOO_MANY_REQUESTS,
            "two_factor_throttled",
            "Too many incorrect codes. Wait a moment and try again.",
        );
    }

    match held.map(|outcome| outcome == throttle::SecondFactor::Held) {
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

/// Claim a TOTP time step for this account, or refuse it (FR-016).
///
/// # Why a conditional UPDATE rather than read-then-write
///
/// The same shape recovery codes use: **zero rows affected means refuse.** Two
/// requests carrying the same code can arrive at once — that is precisely what
/// a replay looks like — and a read followed by a write would let both see an
/// unspent step and both proceed. The condition lives in the WHERE clause so
/// the database decides, once, under its own row lock.
///
/// The predicate mirrors [`step_is_unspent`], which is where the rule is
/// stated and tested in isolation. **Strictly less-than**, not "different":
/// with a skew of one the *previous* step's code is still cryptographically
/// valid and carries a lower step number, so a `<>` comparison would admit it.
pub(crate) fn claim_totp_step_sync(
    conn: &mut PgConnection,
    user_id: uuid::Uuid,
    step: i64,
) -> Result<bool, diesel::result::Error> {
    let claimed = diesel::update(
        users::table.filter(users::id.eq(user_id)).filter(
            users::two_factor_last_used_step
                .is_null()
                .or(users::two_factor_last_used_step.lt(step)),
        ),
    )
    .set(users::two_factor_last_used_step.eq(Some(step)))
    .execute(conn)?;

    Ok(claimed > 0)
}

/// Verify a code and spend the step it matched, in one go.
///
/// Returns false for a code that does not match *and* for one that matches a
/// step already spent. The caller cannot tell those apart, which is right: a
/// replayed code and a wrong code are both "that did not work", and saying
/// which would tell an attacker their intercepted code was genuine.
pub(crate) async fn verify_and_spend_step(
    state: &AppState,
    user_id: uuid::Uuid,
    username: &str,
    secret: &str,
    code: &str,
) -> Result<bool, String> {
    let Some(step) = thunderforge_axum_auth_core::totp::matched_step(username, secret, code)?
    else {
        return Ok(false);
    };
    // A step is `unix_time / 30`; it outgrew i32 in 1972 and will not reach
    // i64 in any timeframe this comment survives.
    let step = i64::try_from(step).map_err(|_| "TOTP step out of range".to_string())?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;
    tokio::task::spawn_blocking(move || claim_totp_step_sync(&mut conn, user_id, step))
        .await
        .map_err(|_| "Failed to spawn blocking task".to_string())?
        .map_err(|_| "Failed to record the code as spent".to_string())
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
    verify_and_spend_step(state, user_id, &username, &secret, code).await
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
