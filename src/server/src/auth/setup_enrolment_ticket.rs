//! The enrolment ticket for a first administrator who has no password to
//! authorise enrolment with.
//!
//! # The dead end this closes
//!
//! `/2fa/setup/start` and `/2fa/setup/confirm` are authorised by exactly one
//! of a username and password or a login challenge (`authorise_enrolment`).
//! A first administrator created through a sign-in provider has neither: the
//! bootstrap gives them a random password nobody ever sees, and the bootstrap
//! callback ends with a session rather than a challenge. The wizard then told
//! them to "sign in once" to enrol — and `/login` redirects to `/setup` for as
//! long as setup is open. Setup could not finish without a second factor and
//! the second factor could not be enrolled until setup finished.
//!
//! The same hole swallowed a locally created administrator who reloaded the
//! page and whose password manager did not save what they typed, though that
//! one at least had a password to remember.
//!
//! # What this is, and what it deliberately is not
//!
//! It is a second place the **existing** ticket is minted. Nothing about how a
//! ticket is spent changes: `authorise_enrolment_by_ticket` still refuses one
//! for an account that holds a factor, it is still single-use, and it still
//! dies in ten minutes. `authorise_enrolment` is untouched and there is no
//! third authorisation shape.
//!
//! The login path mints a ticket after a correct password. This mints one
//! after **both** of:
//!
//!   - a live session for the account the ticket will name — issued by the
//!     bootstrap itself, moments ago, after the provider vouched for the
//!     person (or after `/setup/basic` hashed the password they chose); and
//!   - the one-time bootstrap code, which only somebody who can read this
//!     instance's log holds, and which stops being valid the moment setup
//!     completes.
//!
//! The second is what keeps this from being "a session may enrol a factor" in
//! general. That would be a real widening — a stolen session cookie could put
//! the thief's authenticator on an unenrolled account — and it is exactly the
//! thing the doc comment on `authorise_enrolment` declines to do. Here the
//! route is dead on every instance that has finished setup, because
//! `ensure_admin_setup_code_valid` answers `409 setup_complete` before
//! anything else is looked at.
//!
//! The ticket names the session's own account and no other; there is no
//! account parameter to get wrong.

use super::*;

#[derive(serde::Deserialize)]
pub(crate) struct SetupEnrolmentTicketRequest {
    pub(crate) admin_code: String,
}

/// Why a signed-in account is not handed a ticket.
///
/// Pure, and separate from the handler, for the reason `bootstrap_action` is:
/// the decision is the part worth testing and the rest is plumbing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TicketRefusal {
    /// Setup is the first administrator's to finish. Anybody else who is
    /// signed in and holds the code has no business enrolling through it.
    NotAnAdministrator,
    /// The account already holds a factor. A ticket for it would be refused
    /// when spent anyway; refusing here says why instead of minting a row
    /// that can never be used.
    AlreadyEnrolled,
}

pub(crate) fn ticket_refusal(is_admin: bool, two_factor_enabled: bool) -> Option<TicketRefusal> {
    if !is_admin {
        Some(TicketRefusal::NotAnAdministrator)
    } else if two_factor_enabled {
        Some(TicketRefusal::AlreadyEnrolled)
    } else {
        None
    }
}

/// Mint a ticket for `user_id`, if that account is one setup is waiting on.
///
/// The caller has already established that setup is open and that `user_id`
/// is the account the request's session belongs to.
pub(crate) async fn mint_setup_enrolment_ticket(
    state: &AppState,
    user_id: uuid::Uuid,
) -> Result<uuid::Uuid, (StatusCode, Json<OAuthResponse>)> {
    let mut conn = state.db_pool.get().map_err(|_| {
        error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "setup_error",
            "Failed to get DB connection",
        )
    })?;

    let row = tokio::task::spawn_blocking(move || {
        users::table
            .filter(users::id.eq(user_id))
            .select((users::is_admin, users::two_factor_enabled))
            .first::<(bool, bool)>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())
    .and_then(|r| r.map_err(|_| "Failed to read the account".to_string()))
    .map_err(|msg| {
        error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "setup_error",
            msg.as_str(),
        )
    })?;

    // A session for an account that has since gone is not an administrator.
    let (is_admin, two_factor_enabled) = row.unwrap_or((false, false));

    match ticket_refusal(is_admin, two_factor_enabled) {
        Some(TicketRefusal::NotAnAdministrator) => {
            return Err(error_response(
                StatusCode::FORBIDDEN,
                "forbidden",
                "Only the administrator setup created can enrol through setup",
            ));
        }
        Some(TicketRefusal::AlreadyEnrolled) => {
            return Err(error_response(
                StatusCode::CONFLICT,
                "two_factor_already_enabled",
                "This administrator already holds a second factor",
            ));
        }
        None => {}
    }

    create_login_two_factor_challenge(state, user_id)
        .await
        .map_err(|msg| {
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "setup_error",
                msg.as_str(),
            )
        })
}

/// `POST /authentication/setup/enrolment-ticket`
///
/// The code is checked first, so an instance that has finished setup answers
/// `409 setup_complete` whoever is asking and whatever session they hold.
pub(crate) async fn setup_enrolment_ticket(
    cookies: Cookies,
    State(state): State<AppState>,
    Json(request): Json<SetupEnrolmentTicketRequest>,
) -> (StatusCode, Json<OAuthResponse>) {
    if let Err(resp) = ensure_admin_setup_code_valid(&state, &request.admin_code).await {
        return resp;
    }

    let user = match resolve_authenticated_user(&state, &cookies).await {
        Ok(user) => user,
        Err(StatusCode::UNAUTHORIZED) => {
            return error_response(
                StatusCode::UNAUTHORIZED,
                "unauthenticated",
                "This browser is not signed in as the administrator setup created",
            );
        }
        Err(_) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "session_error",
                "Failed to validate current session",
            );
        }
    };

    match mint_setup_enrolment_ticket(&state, user.user_id).await {
        Ok(challenge_id) => (
            StatusCode::OK,
            Json(OAuthResponse {
                status: "success",
                message: "Enrolment may begin".to_string(),
                challenge_id: None,
                login_two_factor_challenge_id: Some(challenge_id),
            }),
        ),
        Err(resp) => resp,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{insert_test_user, test_app_state};

    #[test]
    fn only_an_unenrolled_administrator_is_handed_a_ticket() {
        assert_eq!(ticket_refusal(true, false), None);
        assert_eq!(
            ticket_refusal(false, false),
            Some(TicketRefusal::NotAnAdministrator),
            "an ordinary account holding the setup code enrolled through setup"
        );
        assert_eq!(
            ticket_refusal(true, true),
            Some(TicketRefusal::AlreadyEnrolled),
            "a ticket was minted for an account that already holds a factor"
        );
    }

    fn set_flags(state: &AppState, user_id: uuid::Uuid, is_admin: bool, enrolled: bool) {
        let mut conn = state.db_pool.get().unwrap();
        diesel::update(users::table.filter(users::id.eq(user_id)))
            .set((
                users::is_admin.eq(is_admin),
                users::two_factor_enabled.eq(enrolled),
            ))
            .execute(&mut conn)
            .expect("failed to prepare the test account");
    }

    /// The case this module exists for: an administrator with no usable
    /// password gets a ticket, and it is one `authorise_enrolment` accepts —
    /// for that account and nobody else's.
    #[tokio::test]
    async fn the_ticket_authorises_the_administrator_it_was_minted_for() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let user_id = insert_test_user(&mut conn);
        drop(conn);
        set_flags(&state, user_id, true, false);

        let ticket = mint_setup_enrolment_ticket(&state, user_id)
            .await
            .unwrap_or_else(|(_, body)| panic!("no ticket was minted: {}", body.message));

        let authorised = authorise_enrolment(&state, None, None, Some(ticket))
            .await
            .unwrap_or_else(|e| panic!("the minted ticket must authorise: {}", e.message));
        assert_eq!(authorised.user_id, user_id);
        assert_eq!(authorised.ticket, Some(ticket));
    }

    #[tokio::test]
    async fn no_ticket_for_an_ordinary_account_or_an_enrolled_one() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let ordinary = insert_test_user(&mut conn);
        let enrolled = insert_test_user(&mut conn);
        drop(conn);
        set_flags(&state, ordinary, false, false);
        set_flags(&state, enrolled, true, true);

        let (code, body) = mint_setup_enrolment_ticket(&state, ordinary)
            .await
            .expect_err("an ordinary account was handed an enrolment ticket");
        assert_eq!(code, StatusCode::FORBIDDEN, "{}", body.message);

        let (code, body) = mint_setup_enrolment_ticket(&state, enrolled)
            .await
            .expect_err("an enrolled administrator was handed an enrolment ticket");
        assert_eq!(code, StatusCode::CONFLICT, "{}", body.message);
    }
}
