//! The two switches an administrator throws, and the instance setting they
//! read back.
//!
//! Spec 041 FR-019 and FR-023. The *rule* these switches feed —
//! `required(user)` and the three login outcomes — lives next door in
//! [`super::requirement`]; this file is only the handlers that write the two
//! columns and the reader that loads the instance one. Moved here whole from
//! `two_factor.rs` by T005 — pure movement, no behaviour change.

use super::*;

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

    // FR-025 wants *who* as well as *for whom*, and this is the one requirement
    // switch where they differ: an administrator is changing somebody else's
    // account. The instance-wide switch above records nothing here on purpose —
    // it has no single subject, and spec 035's `instance_access_events` is
    // where instance-wide policy changes belong.
    let actor_user_id = resolve_authenticated_user(&state, &cookies)
        .await
        .ok()
        .map(|user| user.user_id);

    let mut conn = state.db_pool.get().expect("Failed to get DB connection");
    let required = request.required;
    let result = tokio::task::spawn_blocking(move || {
        set_user_requirement_sync(&mut conn, user_id, actor_user_id, required)
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

/// The write itself: one column, and the record of who changed it, together.
///
/// Lifted out of the handler so it can be driven without an administrator's
/// cookie. The handler's job is authorisation and shape; this is the part
/// that must be true — and "true" here includes the event, which is why they
/// are one transaction rather than two statements. A requirement placed on
/// somebody's account with no record of who placed it is exactly the audit
/// hole FR-025 exists to close, and a failure between the two writes is how
/// that hole appears without anybody deciding to make one.
///
/// Returns the number of rows updated: zero means no such account, which the
/// handler answers as a 404 rather than as a silent success.
pub(crate) fn set_user_requirement_sync(
    conn: &mut PgConnection,
    user_id: uuid::Uuid,
    actor_user_id: Option<uuid::Uuid>,
    required: bool,
) -> Result<usize, diesel::result::Error> {
    conn.transaction::<_, diesel::result::Error, _>(|conn| {
        let updated = diesel::update(users::table.filter(users::id.eq(user_id)))
            .set(users::two_factor_admin_required.eq(required))
            .execute(conn)?;
        if updated > 0 {
            events::record_sync(
                conn,
                user_id,
                actor_user_id,
                if required {
                    events::event_type::REQUIREMENT_SET
                } else {
                    events::event_type::REQUIREMENT_CLEARED
                },
            )?;
        }
        Ok(updated)
    })
}

#[cfg(test)]
#[path = "policy_tests.rs"]
mod policy_tests;
