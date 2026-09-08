//! The one-time bootstrap code that lets the first administrator exist, and
//! the OAuth-provisioned account it can create.
//!
//! # The code survives a restart (spec 040 FR-006, `contracts/setup.md` rule 7)
//!
//! This module used to mint a fresh code on **every** start while setup was
//! incomplete. An operator who was handed a link, closed the browser and
//! restarted the container found their link rejected with no explanation, and
//! the new one was only in a log line they had already scrolled past. So
//! "setup is resumable" was a claim rather than a behaviour.
//!
//! An unconsumed code is now left alone. The cost is that it cannot be printed
//! again — only the Argon2 hash is stored, deliberately, and the alternative to
//! not printing it is keeping the plaintext of a credential that creates an
//! administrator. The log therefore says the earlier link is still valid and
//! names the deliberate way to get a new one (`THUNDERFORGE_REGENERATE_SETUP_CODE`).
//!
//! # The logged link is not `127.0.0.1:5173` (rule 8)
//!
//! It used to be. That address is the Vite dev server on the machine running
//! `make dev`, and it is wrong for every containerised deployment — which is
//! every deployment that is not the author's laptop, and it is the *first*
//! thing an operator sees. There is no public-URL setting in
//! `settings::registry` to derive it from (checked; there is not one, and this
//! module may not add one), so `THUNDERFORGE_PUBLIC_URL` is read directly and,
//! when it is unset, the path alone is logged rather than a host that would be
//! a guess. See ADR-093.

use super::*;

/// The variable that says where this instance is reachable from outside.
///
/// Read here rather than declared in `settings::registry` because a setting
/// stored in the database cannot help the one message that has to be right
/// before anybody has configured anything. It is a candidate for a declaration
/// once something else needs it — ADR-093 records that.
const PUBLIC_URL_VAR: &str = "THUNDERFORGE_PUBLIC_URL";

/// Set this to ask for a new bootstrap code on the next start, replacing an
/// unconsumed one. The deliberate regeneration rule 7 asks for, in the one form
/// available to somebody whose only handle on the instance is its environment.
const REGENERATE_VAR: &str = "THUNDERFORGE_REGENERATE_SETUP_CODE";

/// What a start should do about the bootstrap code.
///
/// Pure, and separate from the statements that carry it out, because the whole
/// defect this replaces was a decision made implicitly by the order of two
/// `if`s. It is also the only way to test the restart case: the development
/// database this suite runs against has administrators in it, so
/// "an instance with no administrator restarts" cannot be arranged with real
/// rows without destroying somebody's data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BootstrapAction {
    /// Mint a code, store its hash, and log the link.
    Mint,
    /// A code from an earlier start is unconsumed. Leave it exactly as it is —
    /// FR-006. The operator's existing link keeps working.
    Keep,
    /// An administrator exists and no setup is in progress: this instance is
    /// set up, and the record should say so.
    MarkComplete,
}

/// The decision, from the four facts it depends on.
///
/// `admin_exists` alone is deliberately **not** completion any more. Since
/// FR-002a, an instance whose first administrator exists but has not confirmed
/// a second factor is mid-setup, not finished — and a restart in that window
/// has to leave the code alone rather than declare victory. See ADR-093.
pub(crate) fn bootstrap_action(
    admin_exists: bool,
    setup_completed: bool,
    has_unconsumed_code: bool,
    regeneration_requested: bool,
) -> BootstrapAction {
    // Refused on an instance that is already set up: a variable left in a
    // container's environment must not be able to reopen setup on a live
    // deployment months later.
    if regeneration_requested && !(admin_exists && setup_completed) {
        return BootstrapAction::Mint;
    }

    if has_unconsumed_code && !setup_completed {
        return BootstrapAction::Keep;
    }

    if admin_exists {
        return BootstrapAction::MarkComplete;
    }

    BootstrapAction::Mint
}

/// Where an operator should go to finish setup.
///
/// `None` for the host half when `THUNDERFORGE_PUBLIC_URL` is unset: a path is
/// incomplete, a wrong host is misleading, and this line is read by somebody
/// who has not got the instance working yet.
fn setup_link(code: &str) -> String {
    match crate::settings::registry::read_env(PUBLIC_URL_VAR) {
        Some(base) => format!("{}/setup/{code}", base.trim_end_matches('/')),
        None => format!(
            "/setup/{code} on this instance (set {PUBLIC_URL_VAR} to have this printed as a full link)"
        ),
    }
}

pub async fn ensure_admin_bootstrap_code(state: &AppState) -> Result<(), String> {
    let now = Utc::now().naive_utc();
    let bootstrap_code = random_setup_code();
    let bootstrap_code_hash = hash_password(&bootstrap_code)?;
    let regeneration_requested = crate::settings::registry::read_env(REGENERATE_VAR)
        .is_some_and(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"));
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;

    let outcome = tokio::task::spawn_blocking(move || -> Result<BootstrapAction, String> {
        let admin_exists = users::table
            .filter(users::is_admin.eq(true))
            .select(users::id)
            .first::<uuid::Uuid>(&mut conn)
            .optional()
            .map_err(|_| "Failed to query admin users".to_string())?
            .is_some();

        let existing = admin_bootstrap_setup::table
            .filter(admin_bootstrap_setup::id.eq(1))
            .select(AdminBootstrapSetup::as_select())
            .first::<AdminBootstrapSetup>(&mut conn)
            .optional()
            .map_err(|_| "Failed to load bootstrap setup state".to_string())?;

        let setup_completed = existing
            .as_ref()
            .is_some_and(|row| row.setup_completed_at.is_some());
        let has_unconsumed_code = existing
            .as_ref()
            .is_some_and(|row| row.admin_code_hash.is_some());

        let action = bootstrap_action(
            admin_exists,
            setup_completed,
            has_unconsumed_code,
            regeneration_requested,
        );

        match action {
            BootstrapAction::Keep => {}
            BootstrapAction::MarkComplete => {
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
            }
            BootstrapAction::Mint => {
                if existing.is_some() {
                    diesel::update(
                        admin_bootstrap_setup::table.filter(admin_bootstrap_setup::id.eq(1)),
                    )
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
            }
        }

        Ok(action)
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())??;

    match outcome {
        BootstrapAction::Mint => tracing::warn!(
            "Initial admin setup is incomplete. To create an admin account, visit: {}",
            setup_link(&bootstrap_code)
        ),
        BootstrapAction::Keep => tracing::warn!(
            "Initial admin setup is incomplete. The setup link issued earlier is still \
             valid — this instance does not reissue it, because only its hash is stored. \
             Set {}=1 and restart to replace it with a new one.",
            REGENERATE_VAR
        ),
        BootstrapAction::MarkComplete => {}
    }

    Ok(())
}

/// Whether setup is finished, from the two facts that decide it.
///
/// Since ADR-093 this is the row's `setup_completed_at` and not "an
/// administrator exists": the first administrator now exists for the whole
/// second half of the wizard. The `admin_exists && !has_unconsumed_code` arm
/// is the back-compatibility one — a deployment upgraded from before this
/// feature, or an administrator created outside setup, is set up, and reading
/// it any other way would reopen the wizard on a running instance.
pub(crate) fn setup_is_completed(admin_exists: bool, setup: Option<&AdminBootstrapSetup>) -> bool {
    match setup {
        Some(row) => {
            row.setup_completed_at.is_some() || (admin_exists && row.admin_code_hash.is_none())
        }
        None => admin_exists,
    }
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
            .map_err(|_| "Failed to query admin users".to_string())?
            .is_some();

        let setup = admin_bootstrap_setup::table
            .filter(admin_bootstrap_setup::id.eq(1))
            .select(AdminBootstrapSetup::as_select())
            .first::<AdminBootstrapSetup>(&mut conn)
            .optional()
            .map_err(|_| "Failed to load bootstrap setup state".to_string())?;

        // ADR-093: an existing administrator is no longer the end of setup.
        // The account step creates them and the second factor and the required
        // settings come after, so this used to refuse the code for the whole
        // second half of the wizard it is the gate for.
        if setup_is_completed(admin_exists, setup.as_ref()) {
            return Ok(Err("Setup has already been completed".to_string()));
        }

        let Some(setup) = setup else {
            return Ok(Err("Setup state is not initialized yet".to_string()));
        };

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

        // ADR-093: deliberately NOT `mark_admin_setup_complete_sync`. Setup
        // finishes at `/authentication/setup/complete`, once the required
        // settings resolve and the second factor is confirmed (FR-002a).
        // Completing here would consume the bootstrap code the remaining steps
        // authenticate with, and would declare an instance set up whose
        // administrator holds only a password.

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

#[cfg(test)]
mod bootstrap_code_tests {
    use super::*;

    /// T052 / FR-006, `contracts/setup.md` rule 7. **The defect this file
    /// existed with**: an unconsumed code was replaced on every start, so
    /// closing the browser and restarting the container silently killed the
    /// operator's link.
    ///
    /// Asserted against `bootstrap_action` rather than against a live
    /// database, because "an instance with no administrator restarts" cannot
    /// be arranged against the development database this suite shares — it has
    /// three hundred administrators in it, and clearing them to make the test
    /// pass would be destroying somebody's data to check a decision that is
    /// pure anyway.
    #[test]
    fn an_unconsumed_code_survives_a_restart() {
        assert_eq!(
            bootstrap_action(false, false, true, false),
            BootstrapAction::Keep,
            "a restart reissued a code that had not been used, killing the operator's link"
        );
    }

    /// The same is true once the first administrator exists: since FR-002a the
    /// account step is the middle of the wizard, so a restart in that window
    /// must not declare victory and consume the code the remaining steps
    /// authenticate with.
    #[test]
    fn a_restart_mid_wizard_does_not_end_setup() {
        assert_eq!(
            bootstrap_action(true, false, true, false),
            BootstrapAction::Keep,
            "restarting after the account step ended a setup that had not finished"
        );
    }

    /// A fresh database still gets a code, and a finished instance is recorded
    /// as finished — the two behaviours that were already right.
    #[test]
    fn an_empty_instance_is_given_a_code_and_a_finished_one_is_recorded() {
        assert_eq!(
            bootstrap_action(false, false, false, false),
            BootstrapAction::Mint
        );
        assert_eq!(
            bootstrap_action(true, false, false, false),
            BootstrapAction::MarkComplete,
            "an instance with an administrator and no setup in progress is set up"
        );
        assert_eq!(
            bootstrap_action(true, true, false, false),
            BootstrapAction::MarkComplete
        );
    }

    /// Rule 7's "and offers a deliberate regeneration". An operator who has
    /// lost the link asks for a new one and gets one; the same variable left
    /// behind in a container's environment cannot reopen setup on a live
    /// instance months later.
    #[test]
    fn regeneration_is_deliberate_and_refused_on_a_finished_instance() {
        assert_eq!(
            bootstrap_action(false, false, true, true),
            BootstrapAction::Mint,
            "an operator who asked for a new code did not get one"
        );
        assert_eq!(
            bootstrap_action(true, true, false, true),
            BootstrapAction::MarkComplete,
            "a stale environment variable reopened setup on a configured instance"
        );
    }

    /// Rule 8. The line an operator reads first must not name the author's
    /// Vite dev server, and must not invent a host it cannot know.
    #[test]
    fn the_logged_link_is_derived_from_configuration() {
        crate::settings::test_env::temp_env(
            &[(PUBLIC_URL_VAR, Some("https://play.example.org/"))],
            || {
                let link = setup_link("abc123");
                assert_eq!(link, "https://play.example.org/setup/abc123");
            },
        );

        crate::settings::test_env::temp_env(&[(PUBLIC_URL_VAR, None)], || {
            let link = setup_link("abc123");
            assert!(
                !link.contains("127.0.0.1") && !link.contains("5173"),
                "the setup link still hard-codes the development server: {link}"
            );
            assert!(
                link.contains("/setup/abc123") && link.contains(PUBLIC_URL_VAR),
                "an unconfigured instance must log the path and say how to get a full link: {link}"
            );
        });
    }

    /// ADR-093's back-compatibility arm, which is the one that decides whether
    /// an upgraded deployment reopens its own setup wizard.
    #[test]
    fn an_upgraded_instance_is_already_set_up() {
        let row = |completed: bool, code: bool| AdminBootstrapSetup {
            id: 1,
            setup_completed_at: completed.then(|| Utc::now().naive_utc()),
            admin_code_hash: code.then(|| "hash".to_string()),
            admin_code_generated_at: code.then(|| Utc::now().naive_utc()),
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        };

        assert!(setup_is_completed(true, Some(&row(true, false))));
        // An administrator and no code in flight: nothing is in progress.
        assert!(setup_is_completed(true, Some(&row(false, false))));
        // Mid-wizard: the administrator exists and the code is still live.
        assert!(!setup_is_completed(true, Some(&row(false, true))));
        assert!(!setup_is_completed(false, Some(&row(false, true))));
        // No row at all — an administrator created outside setup entirely.
        assert!(setup_is_completed(true, None));
        assert!(!setup_is_completed(false, None));
    }
}
