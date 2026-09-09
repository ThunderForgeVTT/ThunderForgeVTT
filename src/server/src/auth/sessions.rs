//! Passwords and sessions: logging in, registering, reading and refreshing a
//! session, logging out, and the cookie that carries it.

use super::*;

/// How many live sessions one account may hold at once (spec 036 FR-004).
///
/// Ten, which is more clients than anybody uses deliberately and few enough
/// that a list of them is readable. Reaching it ends the least recently used
/// rather than refusing the new sign-in — see `issue_session_cookie`.
pub(crate) const MAX_CONCURRENT_SESSIONS: usize = 10;

/// Why a session ended. `revoked_at` records *that* it did; these record
/// which of the several reasons applied, which only became a question once
/// ending one was something a person does deliberately.
///
/// The set is mirrored by a CHECK constraint in the spec-036 migration, so a
/// value added here without adding it there fails at the insert rather than
/// silently widening the column. The constraint also carries `expired`, which
/// nothing writes: expiry is decided by comparing `expires_at` rather than by
/// writing a row. It is in the constraint because the migration is where the
/// vocabulary is fixed, and is not a constant here, because a constant
/// nothing uses is a constant nobody maintains.
pub(crate) mod ended_reason {
    pub(crate) const SIGNED_OUT: &str = "signed_out";
    pub(crate) const BOUND_EXCEEDED: &str = "bound_exceeded";
    pub(crate) const ENDED_BY_USER: &str = "ended_by_user";
    /// FR-008, written by `auth::password_change`. This was in the migration's
    /// vocabulary and in nothing else for as long as the product had no way
    /// to change a password.
    pub(crate) const PASSWORD_CHANGED: &str = "password_changed";
}

pub(crate) async fn basic_authentication(
    cookies: Cookies,
    headers: HeaderMap,
    client: ClientDescription,
    State(state): State<AppState>,
    credentials: String,
) -> (StatusCode, Json<OAuthResponse>) {
    let cred = match Credentials::decode(&credentials) {
        Ok(value) => value,
        Err(message) => {
            return error_response(StatusCode::BAD_REQUEST, "invalid_request", message.as_str());
        }
    };

    let code = headers
        .get("x-2fa-code")
        .and_then(|h| h.to_str().ok())
        .map(|v| v.to_string());

    let (status, response) = authenticate_password_login(
        &state,
        &cookies,
        &cred.username,
        &cred.password,
        code.as_deref(),
        client,
    )
    .await;

    (
        status,
        Json(OAuthResponse {
            status: response.status,
            message: response.message.clone(),
            challenge_id: None,
            login_two_factor_challenge_id: response.login_two_factor_challenge_id,
        }),
    )
}

pub(crate) async fn login(
    cookies: Cookies,
    client: ClientDescription,
    State(state): State<AppState>,
    Json(request): Json<LoginRequest>,
) -> (StatusCode, Json<AuthSessionResponse>) {
    authenticate_password_login(
        &state,
        &cookies,
        &request.identifier,
        &request.password,
        request.two_factor_code.as_deref(),
        client,
    )
    .await
}

pub(crate) async fn register(
    cookies: Cookies,
    client: ClientDescription,
    State(state): State<AppState>,
    Json(request): Json<RegisterRequest>,
) -> (StatusCode, Json<AuthSessionResponse>) {
    // ADR-072. This runs BEFORE the username/email uniqueness probes below,
    // and that ordering is load-bearing: a closed instance must answer
    // identically whether or not the submitted address belongs to a user
    // (FR-011). Reached after the probes, it would be an account-existence
    // oracle for anyone who could read two different status codes.
    let route = AdmissionRoute::Local;
    let admission =
        match ensure_admission_allowed(&state, &route, request.invitation_code.as_deref()).await {
            Ok(value) => value,
            Err(refusal) => {
                let (code, message) = match refusal {
                    AdmissionRefused::Policy(m) => ("registration_blocked", m),
                    AdmissionRefused::InvitationUnusable => (
                        "invitation_unusable",
                        "This invitation is no longer valid".to_string(),
                    ),
                    AdmissionRefused::RateLimited(m) => ("rate_limited", m),
                    AdmissionRefused::Unavailable(m) => ("registration_blocked", m),
                };
                record_refusal(&state, &route).await;
                return auth_session_error(StatusCode::CONFLICT, code, message.as_str());
            }
        };

    let username = request.username.trim().to_string();
    let email = request.email.trim().to_lowercase();

    if let Err(message) = validate_registration_input(&username, &email, &request.password) {
        return auth_session_error(StatusCode::BAD_REQUEST, "invalid_request", message.as_str());
    }

    let password_hash = match hash_password(&request.password) {
        Ok(value) => value,
        Err(message) => {
            return auth_session_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "password_hash_failed",
                message.as_str(),
            );
        }
    };

    let now = Utc::now().naive_utc();
    let mut conn = state.db_pool.get().expect("Failed to get DB connection");
    let create_result =
        tokio::task::spawn_blocking(move || -> Result<uuid::Uuid, RegisterUserError> {
            let username_exists = users::table
                .filter(users::username.eq(&username))
                .select(users::id)
                .first::<uuid::Uuid>(&mut conn)
                .optional()
                .map_err(|_| RegisterUserError::Storage)?;
            if username_exists.is_some() {
                return Err(RegisterUserError::UsernameTaken);
            }

            let email_exists = users::table
                .filter(users::email.eq(&email))
                .select(users::id)
                .first::<uuid::Uuid>(&mut conn)
                .optional()
                .map_err(|_| RegisterUserError::Storage)?;
            if email_exists.is_some() {
                return Err(RegisterUserError::EmailTaken);
            }

            let user_id = uuid::Uuid::now_v7();
            diesel::insert_into(users::table)
                .values((
                    users::id.eq(user_id),
                    users::username.eq(username),
                    users::email.eq(email),
                    users::is_admin.eq(false),
                    users::password_hash.eq(password_hash),
                    users::created_at.eq(now),
                    users::updated_at.eq(now),
                    users::two_factor_enabled.eq(false),
                    users::two_factor_secret_encrypted.eq::<Option<String>>(None),
                    users::two_factor_confirmed_at.eq::<Option<chrono::NaiveDateTime>>(None),
                    users::two_factor_admin_required.eq(false),
                ))
                .execute(&mut conn)
                .map_err(|_| RegisterUserError::Storage)?;

            Ok(user_id)
        })
        .await
        .expect("Failed to spawn blocking task");

    // A failed signup must not burn a use (FR-016). The consume already
    // happened — it has to, so concurrent redeemers see it — so the
    // compensation is explicit on every failure path below.
    if create_result.is_err()
        && let Admission::AllowedByInvitation(invitation_id) = admission
    {
        crate::auth::instance_access::release_invitation_use(&state, invitation_id).await;
    }

    let user_id = match create_result {
        Ok(value) => value,
        Err(RegisterUserError::UsernameTaken) => {
            return auth_session_error(
                StatusCode::CONFLICT,
                "username_taken",
                "Username is already in use",
            );
        }
        Err(RegisterUserError::EmailTaken) => {
            return auth_session_error(
                StatusCode::CONFLICT,
                "email_taken",
                "Email is already in use",
            );
        }
        Err(RegisterUserError::Storage) => {
            return auth_session_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "registration_failed",
                "Failed to create account",
            );
        }
    };

    if let Admission::AllowedByInvitation(invitation_id) = admission {
        let _ =
            crate::auth::instance_access::record_redemption(&state, invitation_id, user_id, &route)
                .await;
    }

    let session = match issue_session_cookie(&state, &cookies, user_id, client).await {
        Ok(value) => value,
        Err(message) => {
            return auth_session_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "session_error",
                message.as_str(),
            );
        }
    };

    match build_session_response(
        &state,
        user_id,
        session.expires_at,
        "success",
        "Account created successfully",
    )
    .await
    {
        Ok(response) => (StatusCode::CREATED, Json(response)),
        Err(message) => auth_session_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "session_error",
            message.as_str(),
        ),
    }
}

pub(crate) async fn current_session(
    cookies: Cookies,
    State(state): State<AppState>,
) -> (StatusCode, Json<AuthSessionResponse>) {
    let authenticated_user = match resolve_authenticated_user(&state, &cookies).await {
        Ok(value) => value,
        Err(StatusCode::UNAUTHORIZED) => {
            return auth_session_error(
                StatusCode::UNAUTHORIZED,
                "unauthenticated",
                "No active session",
            );
        }
        Err(_) => {
            return auth_session_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "session_error",
                "Failed to validate current session",
            );
        }
    };

    match build_session_response(
        &state,
        authenticated_user.user_id,
        authenticated_user.expires_at,
        "authenticated",
        "Active session found",
    )
    .await
    {
        Ok(response) => (StatusCode::OK, Json(response)),
        Err(message) => auth_session_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "session_error",
            message.as_str(),
        ),
    }
}

pub(crate) async fn refresh_session(
    cookies: Cookies,
    client: ClientDescription,
    State(state): State<AppState>,
) -> (StatusCode, Json<AuthSessionResponse>) {
    let authenticated_user = match resolve_authenticated_user(&state, &cookies).await {
        Ok(value) => value,
        Err(StatusCode::UNAUTHORIZED) => {
            return auth_session_error(
                StatusCode::UNAUTHORIZED,
                "unauthenticated",
                "No active session to refresh",
            );
        }
        Err(_) => {
            return auth_session_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "session_error",
                "Failed to validate current session",
            );
        }
    };

    let session =
        match issue_session_cookie(&state, &cookies, authenticated_user.user_id, client).await {
            Ok(value) => value,
            Err(message) => {
                return auth_session_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "session_error",
                    message.as_str(),
                );
            }
        };

    match build_session_response(
        &state,
        authenticated_user.user_id,
        session.expires_at,
        "refreshed",
        "Session rotated successfully",
    )
    .await
    {
        Ok(response) => (StatusCode::OK, Json(response)),
        Err(message) => auth_session_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "session_error",
            message.as_str(),
        ),
    }
}

pub(crate) async fn logout(
    cookies: Cookies,
    State(state): State<AppState>,
) -> (StatusCode, Json<AuthSessionResponse>) {
    if let Some(session_cookie) = cookies.private(&state.key).get("session")
        && let Ok(session_id) = uuid::Uuid::parse_str(session_cookie.value())
    {
        let now = Utc::now().naive_utc();
        if let Ok(mut conn) = state.db_pool.get() {
            let _ = tokio::task::spawn_blocking(move || {
                diesel::update(user_sessions::table.filter(user_sessions::id.eq(session_id)))
                    // Spec 036 FR-006: signing out ends this session and no
                    // other. Saying *why* it ended matters now that there are
                    // several ways for one to.
                    .set((
                        user_sessions::revoked_at.eq(Some(now)),
                        user_sessions::ended_reason.eq(Some(ended_reason::SIGNED_OUT)),
                    ))
                    .execute(&mut conn)
            })
            .await;
        }
    }

    cookies
        .private(&state.key)
        .remove(Cookie::new("session", ""));
    cookies.remove(Cookie::new("csrf_token", ""));

    (
        StatusCode::OK,
        Json(AuthSessionResponse {
            status: "logged_out",
            message: "Session cleared".to_string(),
            session: None,
            login_two_factor_challenge_id: None,
            requires_email_verification: false,
        }),
    )
}

/// Create a session, evict down to the bound, and set the cookies for it.
///
/// `client` is the coarse name the session will be recognised by in its
/// owner's session list (spec 036 FR-005). It is a parameter rather than
/// something derived in here because this function has no request: the
/// handler that does extracts a [`ClientDescription`], and a path that has no
/// browser behind it passes `ClientDescription::unknown()` deliberately.
pub(crate) async fn issue_session_cookie(
    state: &AppState,
    cookies: &Cookies,
    user_id: uuid::Uuid,
    client: ClientDescription,
) -> Result<crate::models::UserSession, String> {
    let now = Utc::now().naive_utc();
    let session_id = uuid::Uuid::now_v7();
    let expires_at = now + chrono::Duration::days(session::SESSION_TTL_DAYS);
    let client_description = client.into_inner();
    let new_session = NewUserSession {
        id: session_id,
        user_id,
        expires_at,
        revoked_at: None,
        created_at: now,
        last_seen_at: now,
        client_description: client_description.clone(),
    };

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection")?;
    tokio::task::spawn_blocking(move || {
        // ADR-073: signing in no longer ends the sessions that came before it.
        //
        // It used to, "to reduce session replay risk" — and the cost was that
        // one account could be signed in in exactly one place, so a second
        // browser signed the first one out. What replaces that protection is
        // a person being able to see their own sessions and end the one they
        // do not recognise, which is a control they can actually operate.
        //
        // What is kept is a bound. An account holds at most
        // `MAX_CONCURRENT_SESSIONS` live sessions; reaching it ends the least
        // recently used rather than refusing the new sign-in, because
        // refusing the new thing is the failure this whole change exists to
        // remove.
        let live: Vec<(uuid::Uuid, chrono::NaiveDateTime)> = user_sessions::table
            .filter(user_sessions::user_id.eq(user_id))
            .filter(user_sessions::revoked_at.is_null())
            .filter(user_sessions::expires_at.gt(now))
            .select((user_sessions::id, user_sessions::last_seen_at))
            .order(user_sessions::last_seen_at.asc())
            .load(&mut conn)?;

        let over = (live.len() + 1).saturating_sub(MAX_CONCURRENT_SESSIONS);
        if over > 0 {
            let evict: Vec<uuid::Uuid> = live.iter().take(over).map(|(id, _)| *id).collect();
            diesel::update(user_sessions::table.filter(user_sessions::id.eq_any(&evict)))
                .set((
                    user_sessions::revoked_at.eq(Some(now)),
                    user_sessions::ended_reason.eq(Some(ended_reason::BOUND_EXCEEDED)),
                ))
                .execute(&mut conn)?;
        }

        diesel::insert_into(user_sessions::table)
            .values(&new_session)
            .execute(&mut conn)
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())
    .and_then(|r| r.map_err(|_| "Failed to persist user session".to_string()))?;

    // The session cookie is encrypted (`.private`); the CSRF cookie is not,
    // because the front end has to be able to read it back. Both shapes are
    // decided in `thunderforge_axum_auth_core::session` so there is one place
    // to look for "is HttpOnly set on that one?".
    cookies
        .private(&state.key)
        .add(cookie_from_spec(session_cookie(
            &session_id.to_string(),
            state.config.secure_cookies,
        )));
    cookies.add(cookie_from_spec(csrf_cookie(
        &uuid::Uuid::now_v7().to_string(),
        state.config.secure_cookies,
    )));
    Ok(crate::models::UserSession {
        id: session_id,
        user_id,
        expires_at,
        revoked_at: None,
        created_at: now,
        last_seen_at: now,
        client_description,
        ended_reason: None,
    })
}

/// Hash a password — or any secret verified the same way — for storage.
///
/// `Argon2::default()` is **Argon2id**, and `hash_password` (as distinct from
/// `hash_password_with_salt`) generates a large random salt per call, so two
/// people with the same password get different stored strings and a stolen
/// table cannot be attacked once for everybody. What comes back is a PHC
/// string carrying the variant, the version, the parameters and the salt
/// alongside the digest, which is what lets `argon2_upgrade` re-hash on
/// verify when the parameters move.
///
/// Both properties are asserted in this module's tests rather than trusted:
/// `hash_password` losing its salt would be a silent, one-character change.
pub(crate) fn hash_password(value: &str) -> Result<String, String> {
    thunderforge_axum_auth_core::hashing::hash(value)
}

pub(crate) async fn authenticate_password_login(
    state: &AppState,
    cookies: &Cookies,
    identifier: &str,
    password: &str,
    two_factor_code: Option<&str>,
    client: ClientDescription,
) -> (StatusCode, Json<AuthSessionResponse>) {
    let identifier = identifier.trim().to_string();
    let email_candidate = identifier.to_lowercase();
    if identifier.is_empty() || password.is_empty() {
        return auth_session_error(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Email or username and password are required",
        );
    }

    let mut conn = state.db_pool.get().expect("Failed to get DB connection");
    let outcome = tokio::task::spawn_blocking(move || {
        users::table
            .filter(
                users::username
                    .eq(&identifier)
                    .or(users::email.eq(&email_candidate)),
            )
            .select((
                users::id,
                users::password_hash,
                users::two_factor_enabled,
                users::two_factor_admin_required,
            ))
            .first::<(uuid::Uuid, String, bool, bool)>(&mut conn)
            .optional()
    })
    .await
    .expect("Failed to spawn blocking task")
    .expect("Failed to query DB");

    let Some((user_id, password_hash, two_factor_enabled, two_factor_admin_required)) = outcome
    else {
        return auth_session_error(StatusCode::UNAUTHORIZED, "failure", "Invalid credentials");
    };

    // Was `PasswordHash::new(..).expect("Invalid hash in db")`, which panicked
    // the request handler on a stored value that would not parse — reachable
    // from a truncated column or a row restored from a partial backup. A hash
    // that is not a hash does not verify; it does not take the server down.
    if !thunderforge_axum_auth_core::hashing::verify(password, &password_hash) {
        return auth_session_error(StatusCode::UNAUTHORIZED, "failure", "Invalid credentials");
    }

    let global_required = match load_global_two_factor_requirement(state).await {
        Ok(value) => value,
        Err(message) => {
            return auth_session_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "settings_error",
                message.as_str(),
            );
        }
    };

    // Spec 041 FR-019 / `contracts/verification.md`. This used to be one
    // boolean — `global || admin_required || enabled` — and one challenge,
    // which meant an account the policy required and had never offered
    // enrolment to was handed a *verification* challenge it could not answer:
    // `verify_two_factor_for_user` returns false for an account with no stored
    // secret, so the policy was a lockout with no way out. The rule is now
    // three outcomes, and none of them is a refusal.
    let step = login_second_factor_step(
        two_factor_enabled,
        global_required,
        two_factor_admin_required,
    );

    if step == LoginSecondFactorStep::Enrol {
        // FR-019. Not conditional on `two_factor_code`: there is no secret for
        // a code to prove, so a client that sent one is answered the same way
        // as one that did not — go and enrol, with the ticket that lets you.
        let challenge_id = match create_login_two_factor_challenge(state, user_id).await {
            Ok(value) => value,
            Err(message) => {
                return auth_session_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "two_factor_error",
                    message.as_str(),
                );
            }
        };

        return (
            StatusCode::UNAUTHORIZED,
            Json(AuthSessionResponse {
                // Distinct on the wire from `two_factor_required` so the sign-in
                // screen can tell "enrol now" from "your code was wrong", and
                // neither is said before the password is correct, so neither
                // discloses anything to somebody who does not already hold it.
                status: "two_factor_enrolment_required",
                message: "This instance requires a second factor. Set one up to finish signing in."
                    .to_string(),
                session: None,
                login_two_factor_challenge_id: Some(challenge_id),
                requires_email_verification: false,
            }),
        );
    }

    if step == LoginSecondFactorStep::Verify && two_factor_code.is_none() {
        let challenge_id = match create_login_two_factor_challenge(state, user_id).await {
            Ok(value) => value,
            Err(message) => {
                return auth_session_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "two_factor_error",
                    message.as_str(),
                );
            }
        };

        return (
            StatusCode::UNAUTHORIZED,
            Json(AuthSessionResponse {
                status: "two_factor_required",
                message: "2FA code required to complete sign-in".to_string(),
                session: None,
                login_two_factor_challenge_id: Some(challenge_id),
                requires_email_verification: false,
            }),
        );
    }

    if let Some(code) = two_factor_code {
        // FR-017 through `guarded`, which is the point of `guarded` existing.
        // This route takes a code directly, so before the bound moved under
        // the verification it was a way to guess codes without ever touching
        // `/2fa/verify` — the throttle guarded one door and this was another.
        match crate::auth::two_factor::throttle::guarded(state, user_id, || {
            verify_two_factor_for_user(state, user_id, code)
        })
        .await
        {
            Ok(crate::auth::two_factor::throttle::SecondFactor::Held) => {}
            Ok(crate::auth::two_factor::throttle::SecondFactor::Throttled) => {
                return auth_session_error(
                    StatusCode::TOO_MANY_REQUESTS,
                    "two_factor_throttled",
                    "Too many incorrect codes. Wait a moment and try again.",
                );
            }
            Ok(crate::auth::two_factor::throttle::SecondFactor::Refused) => {
                return auth_session_error(
                    StatusCode::UNAUTHORIZED,
                    "two_factor_invalid",
                    "Invalid 2FA code",
                );
            }
            Err(message) => {
                return auth_session_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "two_factor_error",
                    message.as_str(),
                );
            }
        }
    }

    let session = match issue_session_cookie(state, cookies, user_id, client).await {
        Ok(value) => value,
        Err(message) => {
            return auth_session_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "session_error",
                message.as_str(),
            );
        }
    };

    match build_session_response(
        state,
        user_id,
        session.expires_at,
        "success",
        "Authenticated successfully",
    )
    .await
    {
        Ok(response) => (StatusCode::OK, Json(response)),
        Err(message) => auth_session_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "session_error",
            message.as_str(),
        ),
    }
}

pub(crate) async fn build_session_response(
    state: &AppState,
    user_id: uuid::Uuid,
    session_expires_at: chrono::NaiveDateTime,
    status: &'static str,
    message: &str,
) -> Result<AuthSessionResponse, String> {
    let user = load_public_user(state, user_id).await?;
    Ok(AuthSessionResponse {
        status,
        message: message.to_string(),
        session: Some(SessionStateResponse {
            authenticated: true,
            user,
            session_expires_at,
        }),
        login_two_factor_challenge_id: None,
        requires_email_verification: false,
    })
}

pub(crate) fn auth_session_error(
    status_code: StatusCode,
    status: &'static str,
    message: &str,
) -> (StatusCode, Json<AuthSessionResponse>) {
    (
        status_code,
        Json(AuthSessionResponse {
            status,
            message: message.to_string(),
            session: None,
            login_two_factor_challenge_id: None,
            requires_email_verification: false,
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::user_sessions;
    use crate::test_support::{insert_test_user, test_app_state};
    use tower_cookies::Cookies;

    /// Every live session row for one account, oldest use first.
    fn live_sessions(state: &AppState, user_id: uuid::Uuid) -> Vec<(uuid::Uuid, Option<String>)> {
        let mut conn = state.db_pool.get().unwrap();
        user_sessions::table
            .filter(user_sessions::user_id.eq(user_id))
            .filter(user_sessions::revoked_at.is_null())
            .order(user_sessions::last_seen_at.asc())
            .select((user_sessions::id, user_sessions::ended_reason))
            .load(&mut conn)
            .unwrap()
    }

    /// Insert a live session whose last use was `minutes_ago`, so the eviction
    /// order under test is decided by the data rather than by insert order.
    fn seed_session(state: &AppState, user_id: uuid::Uuid, minutes_ago: i64) -> uuid::Uuid {
        let id = uuid::Uuid::now_v7();
        let now = Utc::now().naive_utc();
        let mut conn = state.db_pool.get().unwrap();
        diesel::insert_into(user_sessions::table)
            .values((
                user_sessions::id.eq(id),
                user_sessions::user_id.eq(user_id),
                user_sessions::expires_at.eq(now + chrono::Duration::hours(1)),
                user_sessions::created_at.eq(now - chrono::Duration::minutes(minutes_ago)),
                user_sessions::last_seen_at.eq(now - chrono::Duration::minutes(minutes_ago)),
            ))
            .execute(&mut conn)
            .expect("failed to seed a session");
        id
    }

    /// Argon2id, salted per call, and not by accident.
    ///
    /// The whole of password storage is one expression, and every property
    /// worth having comes from which method is called on it —
    /// `hash_password` salts, `hash_password_with_salt` does not choose for
    /// you, and a plain digest would satisfy neither. Asserting the shape
    /// means a change that dropped the salt fails here rather than in a
    /// breach report.
    #[test]
    fn a_password_is_stored_argon2id_and_individually_salted() {
        let first = hash_password("correct horse battery staple").expect("a hash");
        let second = hash_password("correct horse battery staple").expect("a hash");

        assert!(
            first.starts_with("$argon2id$"),
            "expected an Argon2id PHC string, got {first}"
        );
        assert_ne!(
            first, second,
            "two hashes of one password must differ — equal means unsalted, \
             and unsalted means one attack breaks every account that shares a password"
        );

        // And both still verify, so the salt is carried in the string rather
        // than being an input the verifier has to be told about separately.
        for stored in [&first, &second] {
            let parsed = PasswordHash::new(stored).expect("a parseable PHC string");
            assert!(
                Argon2::default()
                    .verify_password("correct horse battery staple".as_bytes(), &parsed)
                    .is_ok()
            );
        }
    }

    /// ADR-073 / FR-001. The defect this replaced: a second sign-in revoked
    /// every session that came before it, so one account could be signed in
    /// in exactly one place.
    #[tokio::test]
    async fn a_second_sign_in_leaves_the_first_session_live() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let user_id = insert_test_user(&mut conn);
        drop(conn);

        let first = issue_session_cookie(
            &state,
            &Cookies::default(),
            user_id,
            ClientDescription::unknown(),
        )
        .await
        .expect("the first sign-in should issue a session");
        let second = issue_session_cookie(
            &state,
            &Cookies::default(),
            user_id,
            ClientDescription::unknown(),
        )
        .await
        .expect("the second sign-in should issue a session");

        assert_ne!(first.id, second.id);
        let live: Vec<uuid::Uuid> = live_sessions(&state, user_id)
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        assert!(
            live.contains(&first.id) && live.contains(&second.id),
            "both sessions must be live; got {live:?}"
        );
    }

    /// FR-004. The bound exists, and reaching it ends the least recently used
    /// rather than refusing the new sign-in — refusing the newest is the
    /// failure this whole change removes, and it would simply reappear at a
    /// higher number.
    #[tokio::test]
    async fn the_bound_ends_the_least_recently_used_and_only_that_one() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let user_id = insert_test_user(&mut conn);
        drop(conn);

        // Fill the account to the bound, oldest use first.
        let seeded: Vec<uuid::Uuid> = (0..MAX_CONCURRENT_SESSIONS)
            .map(|i| seed_session(&state, user_id, (MAX_CONCURRENT_SESSIONS - i) as i64))
            .collect();
        let oldest = seeded[0];

        let fresh = issue_session_cookie(
            &state,
            &Cookies::default(),
            user_id,
            ClientDescription::unknown(),
        )
        .await
        .expect("a sign-in at the bound must still succeed");

        let live: Vec<uuid::Uuid> = live_sessions(&state, user_id)
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        assert_eq!(
            live.len(),
            MAX_CONCURRENT_SESSIONS,
            "the account should sit at the bound, not above it"
        );
        assert!(live.contains(&fresh.id), "the new session must be live");
        assert!(
            !live.contains(&oldest),
            "the least recently used session should have been ended"
        );
        for kept in &seeded[1..] {
            assert!(
                live.contains(kept),
                "no session other than the least recently used may be ended"
            );
        }

        let mut conn = state.db_pool.get().unwrap();
        let reason: Option<String> = user_sessions::table
            .filter(user_sessions::id.eq(oldest))
            .select(user_sessions::ended_reason)
            .first(&mut conn)
            .unwrap();
        assert_eq!(reason.as_deref(), Some(ended_reason::BOUND_EXCEEDED));
    }
}
