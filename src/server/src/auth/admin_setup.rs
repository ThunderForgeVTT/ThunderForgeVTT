//! First-run admin setup: the status probe, the basic-credentials path, the
//! OAuth path, the two endpoints that carry the wizard's answers, and the
//! check every admin-only request runs.
//!
//! # Where setup finishes (ADR-093)
//!
//! It used to finish at `/authentication/setup/basic`: creating the first
//! administrator wrote `setup_completed_at` in the same statement. FR-002a
//! makes that wrong — an instance whose administrator holds only a password is
//! not set up — so the account step now leaves setup open and
//! `/authentication/setup/complete` closes it, once every `RequiredAtSetup`
//! declaration resolves and that administrator's second factor is confirmed.
//!
//! The predicate is in `setup_requirements`; this file is the transport.

use super::*;
use crate::auth::setup_requirements::{RequiredSetting, SetupRefusal, evaluate, required_settings};
use crate::settings::changes::{ChangeSource, write_setting};
use crate::settings::registry::declaration;
use crate::settings::resolver::resolve_all;
use crate::settings::validate::validate;

/// The shapes `contracts/setup.md` specifies for the two new endpoints.
///
/// Declared here rather than in `types.rs` because they are this file's
/// vocabulary and nothing else speaks it — the OAuth-shaped `OAuthResponse`
/// the older setup routes answer with has no room for a `field`, a `missing`
/// list or a `fixed_by`, and widening it would have made every other auth
/// route carry four always-null members.
#[derive(Debug, Deserialize)]
pub(crate) struct SetupSettingsRequest {
    pub(crate) admin_code: String,
    /// Key to value, exactly as declared in `settings::registry`. One step's
    /// worth: the wizard posts each step as it is completed (FR-006), so
    /// resumability is a property of the storage rather than of a session.
    pub(crate) values: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SetupCompleteRequest {
    pub(crate) admin_code: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct SetupSettingsResponse {
    pub(crate) status: &'static str,
    /// What is still unset after this write. The wizard's next question.
    pub(crate) remaining: Vec<&'static str>,
}

#[derive(Debug, Serialize)]
pub(crate) struct SetupErrorResponse {
    pub(crate) status: &'static str,
    pub(crate) code: &'static str,
    pub(crate) message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) field: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) fixed_by: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) missing: Option<Vec<&'static str>>,
}

impl SetupErrorResponse {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        SetupErrorResponse {
            status: "error",
            code,
            message: message.into(),
            field: None,
            fixed_by: None,
            missing: None,
        }
    }
}

fn setup_error(code: StatusCode, body: SetupErrorResponse) -> axum::response::Response {
    (code, Json(body)).into_response()
}

/// The error type `admin_setup_basic`'s transaction rolls back with.
///
/// Diesel requires `From<diesel::result::Error>` on a transaction's error type
/// so that a failed statement can abort the transaction. The refusals in that
/// closure are sentence-shaped strings the handler maps to status codes, so the
/// two are carried as separate variants rather than by stringifying the
/// database error and re-parsing it.
enum TransactionError {
    Refused(String),
    Database(diesel::result::Error),
}

impl From<diesel::result::Error> for TransactionError {
    fn from(value: diesel::result::Error) -> Self {
        TransactionError::Database(value)
    }
}

impl From<String> for TransactionError {
    fn from(value: String) -> Self {
        TransactionError::Refused(value)
    }
}

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
    // ADR-093: an administrator existing is no longer the same fact as setup
    // being finished — the account step now happens in the middle of the
    // wizard rather than at the end of it.
    let setup_completed =
        crate::auth::admin_bootstrap::setup_is_completed(admin_exists, setup.as_ref());

    // FR-002/FR-003/FR-009: what setup still needs, straight from the
    // registry, so adding a declaration adds a field to the wizard and nothing
    // else changes.
    //
    // Reported only while setup is open. On a running instance this endpoint
    // answers anybody at all, and "which of this operator's settings are
    // unset" is not something an anonymous caller has any business reading —
    // the same reasoning that limits `access_policy` to the one fact the
    // signed-out surface needs.
    // Resolved once for this response, and used twice: the wizard's remaining
    // questions while setup is open, and `support_email` always. `App.tsx`
    // calls this endpoint on every page load, so a second `resolve_all` here
    // would be a whole redundant read of the row-set, the manifest and the
    // access policy per navigation.
    let settings = resolve_all(&state).await.ok();

    // FR-026. Published to anybody, including — especially — somebody who
    // cannot sign in, which is the only audience this field has. It is
    // declared `RequiredAtSetup` as "an address people can reach for help
    // with this instance", so this is what it is for.
    let support_email = settings.as_ref().and_then(|settings| {
        settings
            .value("support_email")
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    });

    let (required, second_factor_confirmed) = if setup_completed {
        (Vec::new(), true)
    } else {
        // `unwrap_or_default()` is "no requirements known, no second factor
        // confirmed". A settings read that fails must not take the status
        // probe with it: `App.tsx` calls this on every page load, and
        // answering 500 would make an instance unreachable because one table
        // was slow. Defaulting `second_factor_confirmed` to false is the safe
        // direction to be wrong in — it keeps setup open rather than letting
        // it finish on a read that did not happen.
        match settings.as_ref() {
            Some(settings) => evaluate_for_status(&state, settings)
                .await
                .unwrap_or_default(),
            None => Default::default(),
        }
    };

    // FR-003. Read per request, so a policy change takes effect without a
    // restart (FR-002) — which is what makes SC-004's three minutes possible.
    let access_policy = crate::auth::instance_access::load_policy(&state)
        .await
        .unwrap_or(crate::auth::instance_access::InstanceAccessPolicy::Closed);

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
            access_policy: access_policy.as_db_str().to_string(),
            // US3 is not built. Constant rather than absent so the front end's
            // shape does not change when it is.
            accepting_access_requests: false,
            required_settings: required,
            second_factor_confirmed,
            support_email,
        }),
    )
}

/// The two registry-driven halves of the status response, or nothing if the
/// settings could not be read.
///
/// One `resolve_all` and one query, not `evaluate`'s: `App.tsx` calls this
/// endpoint on **every page load**, so the difference between resolving the
/// settings once and resolving them twice is a whole redundant read of the
/// row-set, the manifest and the access policy per navigation.
async fn evaluate_for_status(
    state: &AppState,
    settings: &crate::settings::Settings,
) -> Option<(Vec<RequiredSetting>, bool)> {
    let mut conn = state.db_pool.get().ok()?;
    let second_factor_confirmed = tokio::task::spawn_blocking(move || {
        crate::auth::setup_requirements::first_administrator_second_factor_confirmed(&mut conn)
    })
    .await
    .ok()?
    .ok()?;

    Some((required_settings(settings), second_factor_confirmed))
}

/// FR-006: one step of the pass, written as it is completed.
///
/// Every value goes through `settings::validate` and `settings::changes`, so a
/// value typed into the wizard is refused on exactly the same grounds as one
/// typed into the administration surface (FR-004) and leaves the same record
/// (FR-008) — the alternative was a second write path that could disagree with
/// the first about what an acceptable notice address is.
pub(crate) async fn setup_settings(
    State(state): State<AppState>,
    Json(request): Json<SetupSettingsRequest>,
) -> axum::response::Response {
    if let Err((code, payload)) = ensure_admin_setup_code_valid(&state, &request.admin_code).await {
        return (code, payload).into_response();
    }

    // Resolved once for the whole step rather than once per key: `write_setting`
    // resolves again for its own record, and adding a third read per value on
    // top of that turned a five-field step into fifteen loads of the row-set.
    let resolved = match resolve_all(&state).await {
        Ok(v) => v,
        Err(message) => {
            return setup_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                SetupErrorResponse::new("setup_error", message),
            );
        }
    };

    for (key, value) in &request.values {
        let Some(d) = declaration(key) else {
            let mut body = SetupErrorResponse::new(
                "unknown_setting",
                format!("`{key}` is not a setting this instance declares."),
            );
            body.field = Some(key.clone());
            return setup_error(StatusCode::BAD_REQUEST, body);
        };

        // FR-009, checked before validating rather than after: telling somebody
        // their value is malformed and *then* that it would have been ignored
        // anyway is two refusals where one is true.
        if let Some(name) = resolved.get(d.key).and_then(|r| r.fixed_by) {
            let mut body = SetupErrorResponse::new(
                "fixed_by_environment",
                format!(
                    "`{}` is fixed by the environment variable `{name}`. Setup does not \
                     ask for it and cannot change it.",
                    d.key
                ),
            );
            body.field = Some(d.key.to_string());
            body.fixed_by = Some(name);
            return setup_error(StatusCode::CONFLICT, body);
        }

        if let Err(message) = validate(d, value) {
            let mut body = SetupErrorResponse::new("invalid_value", message);
            body.field = Some(d.key.to_string());
            return setup_error(StatusCode::BAD_REQUEST, body);
        }

        // `actor: None` — nobody is signed in for the first steps of the
        // wizard, and the bootstrap code is what authenticated this. The record
        // says `setup`, which is the fact worth keeping.
        if let Err(message) =
            write_setting(&state, d.key, Some(value), None, ChangeSource::Setup).await
        {
            let mut body = SetupErrorResponse::new("invalid_value", message);
            body.field = Some(d.key.to_string());
            return setup_error(StatusCode::BAD_REQUEST, body);
        }
    }

    let remaining = match resolve_all(&state).await {
        Ok(settings) => crate::auth::setup_requirements::missing_required_settings(&settings),
        Err(message) => {
            return setup_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                SetupErrorResponse::new("setup_error", message),
            );
        }
    };

    (
        StatusCode::OK,
        Json(SetupSettingsResponse {
            status: "success",
            remaining,
        }),
    )
        .into_response()
}

/// FR-002a and FR-006: the one place setup finishes, and it finishes once.
pub(crate) async fn setup_complete(
    State(state): State<AppState>,
    Json(request): Json<SetupCompleteRequest>,
) -> axum::response::Response {
    if let Err((code, payload)) = ensure_admin_setup_code_valid(&state, &request.admin_code).await {
        return (code, payload).into_response();
    }

    let completion = match evaluate(&state).await {
        Ok(v) => v,
        Err(message) => {
            return setup_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                SetupErrorResponse::new("setup_error", message),
            );
        }
    };

    match completion.refusal() {
        Some(SetupRefusal::Incomplete(missing)) => {
            let mut body = SetupErrorResponse::new(
                "incomplete",
                "Setup still needs values it cannot publish a page without.",
            );
            body.missing = Some(missing);
            return setup_error(StatusCode::CONFLICT, body);
        }
        Some(SetupRefusal::SecondFactorRequired) => {
            return setup_error(
                StatusCode::CONFLICT,
                SetupErrorResponse::new(
                    "second_factor_required",
                    "The first administrator has not confirmed a second factor. Nothing is \
                     rolled back — enrol one and finish setup.",
                ),
            );
        }
        None => {}
    }

    let now = Utc::now().naive_utc();
    let mut conn = match state.db_pool.get() {
        Ok(v) => v,
        Err(_) => {
            return setup_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                SetupErrorResponse::new("setup_error", "Failed to get DB connection"),
            );
        }
    };

    let written =
        tokio::task::spawn_blocking(move || complete_setup_exclusively(&mut conn, now)).await;

    match written {
        Ok(Ok(true)) => (
            StatusCode::OK,
            Json(OAuthResponse {
                status: "success",
                message: "Setup is complete.".to_string(),
                challenge_id: None,
                login_two_factor_challenge_id: None,
            }),
        )
            .into_response(),
        Ok(Ok(false)) => setup_error(
            StatusCode::CONFLICT,
            SetupErrorResponse::new("setup_complete", "Setup has already been completed"),
        ),
        Ok(Err(message)) => setup_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            SetupErrorResponse::new("setup_error", message),
        ),
        Err(_) => setup_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            SetupErrorResponse::new("setup_error", "Failed to spawn blocking task"),
        ),
    }
}

/// Writes `setup_completed_at` for whichever caller gets there first, and tells
/// the other one it lost. `Ok(true)` wrote it; `Ok(false)` is `409
/// setup_complete`.
///
/// One transaction, with the singleton row taken `FOR UPDATE`, for the reason
/// `instance_identity::instance_id` gives about check-then-insert: first launch
/// is exactly when several requests arrive at once, and a window between the
/// check and the write is a window in which both of them pass. Without the
/// lock the loser of the race got a generic `500` out of a unique-constraint
/// violation rather than the `409` that says what actually happened.
///
/// The row-absent branch uses `ON CONFLICT DO NOTHING` rather than a second
/// check, for the same reason: there is nothing to take `FOR UPDATE` when the
/// row does not exist yet, so the unique index on the singleton key is what
/// serialises the two callers, and an insert that affects no rows is the loser.
pub(crate) fn complete_setup_exclusively(
    conn: &mut diesel::PgConnection,
    now: chrono::NaiveDateTime,
) -> Result<bool, String> {
    conn.transaction::<bool, diesel::result::Error, _>(|conn| {
        let existing = admin_bootstrap_setup::table
            .filter(admin_bootstrap_setup::id.eq(1))
            .for_update()
            .select(AdminBootstrapSetup::as_select())
            .first::<AdminBootstrapSetup>(conn)
            .optional()?;

        match existing {
            Some(row) if row.setup_completed_at.is_some() => Ok(false),
            Some(_) => {
                diesel::update(
                    admin_bootstrap_setup::table.filter(admin_bootstrap_setup::id.eq(1)),
                )
                .set((
                    admin_bootstrap_setup::setup_completed_at.eq(Some(now)),
                    admin_bootstrap_setup::admin_code_hash.eq::<Option<String>>(None),
                    admin_bootstrap_setup::admin_code_generated_at
                        .eq::<Option<chrono::NaiveDateTime>>(None),
                    admin_bootstrap_setup::updated_at.eq(now),
                ))
                .execute(conn)?;
                Ok(true)
            }
            None => {
                let inserted = diesel::insert_into(admin_bootstrap_setup::table)
                    .values(&NewAdminBootstrapSetup {
                        id: 1,
                        setup_completed_at: Some(now),
                        admin_code_hash: None,
                        admin_code_generated_at: None,
                        created_at: now,
                        updated_at: now,
                    })
                    .on_conflict(admin_bootstrap_setup::id)
                    .do_nothing()
                    .execute(conn)?;
                Ok(inserted == 1)
            }
        }
    })
    .map_err(|e| format!("Failed to complete setup: {e}"))
}

pub(crate) async fn admin_setup_basic(
    cookies: Cookies,
    client: ClientDescription,
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
    // One transaction around the whole check-then-insert, and the singleton
    // setup row taken `FOR UPDATE` first so that two people posting this at the
    // same instant serialise on it. Without that, both callers passed the
    // "does an administrator exist" check and the loser hit
    // `users_username_key` — a generic `500` where the truth is `409
    // setup_complete`. `instance_identity::instance_id` documents the same
    // window; first launch is exactly when several requests arrive at once.
    let result = tokio::task::spawn_blocking(move || -> Result<uuid::Uuid, String> {
        // `TransactionError` rather than `String` because Diesel needs to be
        // able to turn its own error into the closure's error type in order to
        // roll back; `String` cannot, and this is the smallest wrapper that
        // keeps the refusal messages below as plain strings.
        conn.transaction::<uuid::Uuid, TransactionError, _>(|conn| {
            let _lock = admin_bootstrap_setup::table
                .filter(admin_bootstrap_setup::id.eq(1))
                .for_update()
                .select(AdminBootstrapSetup::as_select())
                .first::<AdminBootstrapSetup>(conn)
                .optional()
                .map_err(|_| "Failed to load bootstrap setup state".to_string())?;

            let admin_exists = users::table
                .filter(users::is_admin.eq(true))
                .select(users::id)
                .first::<uuid::Uuid>(conn)
                .optional()
                .map_err(|_| "Failed to query existing admins".to_string())?;
            if admin_exists.is_some() {
                return Err(TransactionError::Refused(
                    "Setup has already been completed".to_string(),
                ));
            }

            let username_exists = users::table
                .filter(users::username.eq(&username))
                .select(users::id)
                .first::<uuid::Uuid>(conn)
                .optional()
                .map_err(|_| "Failed to validate username".to_string())?;
            if username_exists.is_some() {
                return Err(TransactionError::Refused(
                    "Username is already in use".to_string(),
                ));
            }

            let email_exists = users::table
                .filter(users::email.eq(&email))
                .select(users::id)
                .first::<uuid::Uuid>(conn)
                .optional()
                .map_err(|_| "Failed to validate email".to_string())?;
            if email_exists.is_some() {
                return Err(TransactionError::Refused(
                    "Email is already in use".to_string(),
                ));
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
                .execute(conn)
                .map_err(|_| "Failed to create admin user".to_string())?;

            // ADR-093: deliberately NOT `mark_admin_setup_complete_sync`. The
            // account step is the middle of the wizard now, not the end of it —
            // `/authentication/setup/complete` writes `setup_completed_at` once the
            // required settings resolve and this administrator's second factor is
            // confirmed (FR-002a). Completing here would also consume the bootstrap
            // code the remaining steps authenticate with.

            Ok(user_id)
        })
        .map_err(|e| match e {
            TransactionError::Refused(message) => message,
            // Logged rather than returned: the caller is an anonymous browser
            // on an uninitialised instance, and a Diesel error names columns.
            TransactionError::Database(e) => {
                tracing::error!("First-administrator creation failed: {e}");
                "Failed to create admin user".to_string()
            }
        })
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

    if let Err(msg) = issue_session_cookie(&state, &cookies, user_id, client).await {
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
    client: ClientDescription,
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

    if let Err(msg) = issue_session_cookie(&state, &cookies, user_id, client).await {
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

/// One lock for the `admin_bootstrap_setup` singleton and the `is_admin` flag.
///
/// Both are process-global as far as this suite is concerned: there is one row
/// with `id = 1` and one set of administrators in the development database
/// every test shares. Two tests that arrange "setup is not finished" at the
/// same time are in each other's way, and the failure looks like flakiness
/// rather than like the missing lock it is.
///
/// A **second** mutex over the same resource would serialise nothing, so tests
/// that also touch the environment or the settings rows take
/// `settings::test_env::lock()` first and this one second — always in that
/// order, so two locks cannot deadlock against each other.
#[cfg(test)]
pub(crate) fn setup_state_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// T051's race, and the completion-once check, in their own file: this one is
/// against the 1000-line gate and the tests are the newest part of it.
#[cfg(test)]
#[path = "setup_completion_tests.rs"]
mod completion_race_tests;
