//! Spec 041 US4 (FR-019 … FR-022): who must hold a second factor, what a
//! sign-in does about it, and how an account that has not enrolled is
//! authorised to enrol at the moment it is asked to.
//!
//! # The lockout this closes
//!
//! Before this module, turning the instance-wide requirement on refused
//! everybody who had not already enrolled, permanently and with no way to fix
//! it from the screen where they were refused. Two things met to make that
//! happen and neither was wrong on its own:
//!
//!   - the login path challenged an account whenever
//!     `global || admin_required || enabled` — so a requirement produced a
//!     *verification* challenge for an account with nothing to verify;
//!   - `verify_two_factor_for_user` answers `false` for an account with no
//!     stored secret, which is right (it holds no factor) and is also the
//!     only answer that challenge could ever get.
//!
//! `contracts/requirement-policy.md` splits the expression in two, and
//! `contracts/verification.md`'s table is `login_second_factor_step` below:
//! holding a factor means *verify*, being required to hold one you do not have
//! means *enrol*, and neither of them means *refused*.
//!
//! # The `|| two_factor_enabled` term is gone
//!
//! Having a factor is not a reason you must have one. It is a reason you are
//! asked for it, which is a different question and is now a different answer
//! (`Verify` rather than `required = true`). Keeping the two collapsed is what
//! made the instance-wide switch look safe — and it is also FR-022: with the
//! requirement off, an enrolled account still gets `Verify`, because its
//! factor never depended on the policy.

use super::*;

/// What a sign-in must do about a second factor, once the password is correct.
///
/// `contracts/verification.md`, "The login path", as three values rather than
/// as a boolean plus an implication. There is deliberately no fourth variant
/// for "refuse": refusing is what FR-019 exists to remove.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LoginSecondFactorStep {
    /// Nothing further is asked. A session is issued.
    SignIn,
    /// The account holds a factor and must prove it (unchanged behaviour).
    Verify,
    /// The account is required to hold a factor and does not. It is taken
    /// through enrolment (FR-019), never refused for not having enrolled
    /// beforehand.
    Enrol,
}

/// `required(user)` from `contracts/requirement-policy.md`, minus the term
/// that is a property of a role.
///
/// `user.is_admin` (FR-027) belongs in this expression and is not here yet:
/// it is US5's, it changes what happens to every administrator on an upgraded
/// instance, and adding it as a side effect of US4 would be a different
/// feature landing without its own review. When it lands it is one more `||`
/// on this line and nothing else in this module moves.
pub(crate) fn second_factor_required(instance_required: bool, admin_required: bool) -> bool {
    instance_required || admin_required
}

/// The three rows of `contracts/verification.md`'s login table.
///
/// Pure on purpose. Every caller — password login, the OAuth resolve — reads
/// the same three inputs out of a different place, and the rule they share is
/// the thing worth testing without a database in the way.
pub(crate) fn login_second_factor_step(
    two_factor_enabled: bool,
    instance_required: bool,
    admin_required: bool,
) -> LoginSecondFactorStep {
    if two_factor_enabled {
        // FR-022. A factor in force is asked for whatever the policy says,
        // and turning a requirement off does not reach it.
        LoginSecondFactorStep::Verify
    } else if second_factor_required(instance_required, admin_required) {
        LoginSecondFactorStep::Enrol
    } else {
        LoginSecondFactorStep::SignIn
    }
}

/// The same decision for an account, read from the instance policy and the
/// account's own row.
pub(crate) async fn login_second_factor_step_for_user(
    state: &AppState,
    user_id: uuid::Uuid,
) -> Result<LoginSecondFactorStep, String> {
    let instance_required = load_global_two_factor_requirement(state).await?;
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection")?;
    let row = tokio::task::spawn_blocking(move || {
        users::table
            .filter(users::id.eq(user_id))
            .select((users::two_factor_enabled, users::two_factor_admin_required))
            .first::<(bool, bool)>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())
    .and_then(|r| r.map_err(|_| "Failed to query user 2FA state".to_string()))?;

    let Some((enabled, admin_required)) = row else {
        return Ok(LoginSecondFactorStep::SignIn);
    };

    Ok(login_second_factor_step(
        enabled,
        instance_required,
        admin_required,
    ))
}

/// An account that has proved it may enrol, and how it proved it.
pub(crate) struct AuthorisedEnrolment {
    pub(crate) user_id: uuid::Uuid,
    /// The label the `otpauth://` URI is built from.
    pub(crate) username: String,
    /// The login challenge that authorised this, when one did. Confirmation
    /// spends it and issues the session — which is what makes the sign-in the
    /// person was already doing finish where they were going (FR-020).
    pub(crate) ticket: Option<uuid::Uuid>,
}

/// Why an enrolment was not authorised, in the shape both setup handlers
/// answer in.
pub(crate) struct EnrolmentAuthorisationError {
    pub(crate) code: StatusCode,
    pub(crate) status: &'static str,
    pub(crate) message: &'static str,
}

impl EnrolmentAuthorisationError {
    fn new(code: StatusCode, status: &'static str, message: &'static str) -> Self {
        Self {
            code,
            status,
            message,
        }
    }
}

/// Authorise an enrolment by **exactly one** of a password or a login
/// challenge.
///
/// # The authorisation decision, and what it costs
///
/// `/2fa/setup/start` and `/2fa/setup/confirm` take a username and a password
/// and verify the Argon2 hash themselves. That works for the account-settings
/// entrance and it cannot work for this one: at the login challenge the person
/// has a correct password and no session, and the whole point of FR-019 is
/// that they are mid-sign-in.
///
/// Re-posting the password from the challenge screen would have worked and is
/// what this deliberately does not do. Instead the `login_two_factor_challenges`
/// row the login path just minted **is** the authorisation: it is created only
/// after a correct password, is bound to one account, is single-use, and dies
/// in ten minutes. That is the "single-use ticket" `contracts/enrolment.md`
/// asks for, and it needs no new table and no new column — which matters,
/// because a ticket that needed a migration would have been a password
/// re-post instead.
///
/// **What it costs.** A challenge id now buys something a challenge id did not
/// buy before: the ability to put a second factor on the account it names. So
/// it is fenced to the case that needs it — an account that holds **no**
/// confirmed factor. A challenge minted for an account that already has one is
/// a *verification* challenge and is refused here, unspent, so this can never
/// become a way to replace somebody's factor with a stolen challenge id. Held
/// against the alternative, the ticket is strictly narrower than the password
/// it replaces: the password can enrol on any account at any time, and the
/// ticket can enrol only on an unenrolled account, only for ten minutes, and
/// only once.
///
/// The residual cost is real and worth saying plainly: an attacker who can
/// read a challenge id out of a response (having already stolen the password,
/// which is the only way one gets minted) can enrol *their own* authenticator
/// on an account that has none. That is the same thing they could already do
/// with the password alone over `/2fa/setup/start`, so nothing is widened —
/// but it is not narrowed either, and closing it is FR-012's and US3's work,
/// not this module's.
pub(crate) async fn authorise_enrolment(
    state: &AppState,
    username: Option<&str>,
    password: Option<&str>,
    challenge_id: Option<uuid::Uuid>,
) -> Result<AuthorisedEnrolment, EnrolmentAuthorisationError> {
    match (username, password, challenge_id) {
        (Some(username), Some(password), None) => {
            authorise_enrolment_by_password(state, username, password).await
        }
        (None, None, Some(challenge_id)) => {
            authorise_enrolment_by_ticket(state, challenge_id).await
        }
        // Both, or neither. Refused before either is evaluated, for the reason
        // `two_factor_verify` refuses a request carrying two proofs: one
        // request is one attempt at one thing.
        _ => Err(EnrolmentAuthorisationError::new(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Provide exactly one of a username and password or a login challenge",
        )),
    }
}

/// The account-settings and first-run entrances: a username and its password.
///
/// Lifted out of the two setup handlers, which each verified the hash
/// themselves. `"Invalid credentials"` for an unknown username and for a wrong
/// password alike, which is what the rest of this surface answers.
async fn authorise_enrolment_by_password(
    state: &AppState,
    username: &str,
    password: &str,
) -> Result<AuthorisedEnrolment, EnrolmentAuthorisationError> {
    let username = username.to_string();
    let username_for_query = username.clone();
    let mut conn = state.db_pool.get().map_err(|_| {
        EnrolmentAuthorisationError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "two_factor_error",
            "Failed to get DB connection",
        )
    })?;

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

    let invalid = || {
        EnrolmentAuthorisationError::new(StatusCode::UNAUTHORIZED, "failure", "Invalid credentials")
    };

    let Some((user_id, password_hash)) = user else {
        return Err(invalid());
    };

    if !thunderforge_axum_auth_core::hashing::verify(password, &password_hash) {
        return Err(invalid());
    }

    Ok(AuthorisedEnrolment {
        user_id,
        username,
        ticket: None,
    })
}

/// The sign-in entrance: the challenge the login path just minted.
///
/// Not spent here. `setup/start` may be called more than once for one
/// enrolment (starting over re-provisions the pending secret and nothing
/// else), so the ticket has to survive until confirmation, which is the
/// moment it buys a session.
async fn authorise_enrolment_by_ticket(
    state: &AppState,
    challenge_id: uuid::Uuid,
) -> Result<AuthorisedEnrolment, EnrolmentAuthorisationError> {
    let now = Utc::now().naive_utc();
    let mut conn = state.db_pool.get().map_err(|_| {
        EnrolmentAuthorisationError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "two_factor_error",
            "Failed to get DB connection",
        )
    })?;

    let row = tokio::task::spawn_blocking(move || {
        login_two_factor_challenges::table
            .filter(login_two_factor_challenges::id.eq(challenge_id))
            .inner_join(users::table)
            .select((
                login_two_factor_challenges::expires_at,
                login_two_factor_challenges::consumed_at,
                users::id,
                users::username,
                users::two_factor_enabled,
            ))
            .first::<(
                chrono::NaiveDateTime,
                Option<chrono::NaiveDateTime>,
                uuid::Uuid,
                String,
                bool,
            )>(&mut conn)
            .optional()
    })
    .await
    .expect("Failed to spawn blocking task")
    .expect("Failed to query 2FA challenge");

    let invalid = || {
        EnrolmentAuthorisationError::new(
            StatusCode::BAD_REQUEST,
            "two_factor_challenge_invalid",
            "2FA challenge is expired or already used",
        )
    };

    let Some((expires_at, consumed_at, user_id, username, two_factor_enabled)) = row else {
        return Err(invalid());
    };

    if consumed_at.is_some() || expires_at <= now {
        return Err(invalid());
    }

    // The fence described above: a challenge for an account that already holds
    // a factor is a challenge to *verify* it, and answering it with an
    // enrolment would be a way past the factor rather than a way to one.
    if two_factor_enabled {
        return Err(invalid());
    }

    Ok(AuthorisedEnrolment {
        user_id,
        username,
        ticket: Some(challenge_id),
    })
}

/// Spend the ticket. Zero rows means somebody else spent it first, and the
/// caller rolls the enrolment back rather than issuing a session for a
/// challenge that is already gone.
pub(crate) fn consume_enrolment_ticket_sync(
    conn: &mut PgConnection,
    challenge_id: uuid::Uuid,
    now: chrono::NaiveDateTime,
) -> QueryResult<usize> {
    diesel::update(
        login_two_factor_challenges::table
            .filter(login_two_factor_challenges::id.eq(challenge_id))
            .filter(login_two_factor_challenges::consumed_at.is_null())
            .filter(login_two_factor_challenges::expires_at.gt(now)),
    )
    .set(login_two_factor_challenges::consumed_at.eq(Some(now)))
    .execute(conn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{insert_test_user, test_app_state};

    /// Put a confirmed factor on an account, the way a completed enrolment
    /// leaves it.
    fn enrol(state: &AppState, user_id: uuid::Uuid) {
        let mut conn = state.db_pool.get().unwrap();
        diesel::update(users::table.filter(users::id.eq(user_id)))
            .set((
                users::two_factor_enabled.eq(true),
                users::two_factor_secret_encrypted.eq(Some("ciphertext".to_string())),
                users::two_factor_confirmed_at.eq(Some(Utc::now().naive_utc())),
            ))
            .execute(&mut conn)
            .expect("failed to enrol the test account");
    }

    fn insert_challenge(
        state: &AppState,
        user_id: uuid::Uuid,
        expires_at: chrono::NaiveDateTime,
        consumed_at: Option<chrono::NaiveDateTime>,
    ) -> uuid::Uuid {
        let id = uuid::Uuid::now_v7();
        let mut conn = state.db_pool.get().unwrap();
        diesel::insert_into(login_two_factor_challenges::table)
            .values(&NewLoginTwoFactorChallenge {
                id,
                user_id,
                expires_at,
                consumed_at,
                created_at: Utc::now().naive_utc(),
            })
            .execute(&mut conn)
            .expect("failed to insert the test challenge");
        id
    }

    fn factor_columns(
        state: &AppState,
        user_id: uuid::Uuid,
    ) -> (bool, Option<String>, Option<chrono::NaiveDateTime>) {
        let mut conn = state.db_pool.get().unwrap();
        users::table
            .filter(users::id.eq(user_id))
            .select((
                users::two_factor_enabled,
                users::two_factor_secret_encrypted,
                users::two_factor_confirmed_at,
            ))
            .first(&mut conn)
            .expect("failed to read the account")
    }

    /// FR-019. The row that used to be a locked door: required, not enrolled.
    ///
    /// Nothing about this asks whether the account *can* answer a challenge,
    /// because that was the bug — the old expression produced a verification
    /// challenge for an account with no secret, and the only answer such a
    /// challenge can ever get is "no".
    #[test]
    fn a_required_account_without_a_factor_is_sent_to_enrol_not_refused() {
        assert_eq!(
            login_second_factor_step(false, true, false),
            LoginSecondFactorStep::Enrol,
            "the instance-wide requirement must take an unenrolled account through enrolment"
        );
        assert_eq!(
            login_second_factor_step(false, false, true),
            LoginSecondFactorStep::Enrol,
            "a per-account requirement must do the same"
        );
    }

    /// FR-022, as arithmetic: the factor survives the policy in both
    /// directions, because the policy is not one of its inputs.
    #[test]
    fn turning_the_requirement_off_still_asks_an_enrolled_account_for_its_code() {
        assert_eq!(
            login_second_factor_step(true, false, false),
            LoginSecondFactorStep::Verify
        );
        assert_eq!(
            login_second_factor_step(true, true, true),
            LoginSecondFactorStep::Verify
        );
    }

    /// FR-033. Everybody else is untouched, which is the requirement the other
    /// two rows are only tolerable because of.
    #[test]
    fn an_unenrolled_account_nobody_requires_anything_of_just_signs_in() {
        assert_eq!(
            login_second_factor_step(false, false, false),
            LoginSecondFactorStep::SignIn
        );
    }

    /// FR-022 against the database rather than the arithmetic: clearing a
    /// per-account requirement writes one column and reaches nothing else.
    ///
    /// The instance-wide half of FR-022 is the same statement in
    /// `set_admin_two_factor_requirement` — one `UPDATE` against
    /// `auth_security_settings`, which cannot name a user row — and is
    /// deliberately not driven here: flipping a single-row global setting
    /// inside a suite that runs its tests in parallel makes every other test
    /// that reads it non-deterministic. The rule it must satisfy is the
    /// `Verify` assertion above.
    #[tokio::test]
    async fn clearing_a_requirement_leaves_the_factor_in_force() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let user_id = insert_test_user(&mut conn);
        drop(conn);

        enrol(&state, user_id);
        let before = factor_columns(&state, user_id);

        let mut conn = state.db_pool.get().unwrap();
        diesel::update(users::table.filter(users::id.eq(user_id)))
            .set(users::two_factor_admin_required.eq(true))
            .execute(&mut conn)
            .unwrap();
        diesel::update(users::table.filter(users::id.eq(user_id)))
            .set(users::two_factor_admin_required.eq(false))
            .execute(&mut conn)
            .unwrap();
        drop(conn);

        assert_eq!(
            factor_columns(&state, user_id),
            before,
            "clearing a requirement must not touch the factor it required"
        );
        assert_eq!(
            login_second_factor_step_for_user(&state, user_id)
                .await
                .expect("the step must be answerable"),
            LoginSecondFactorStep::Verify,
            "the account still holds a factor, so it is still asked for it"
        );
    }

    /// The ticket, as FR-019 needs it: a fresh challenge for an account with
    /// no factor authorises that account to enrol, without a password.
    #[tokio::test]
    async fn a_fresh_challenge_authorises_the_account_it_names_to_enrol() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let user_id = insert_test_user(&mut conn);
        drop(conn);

        let challenge_id = insert_challenge(
            &state,
            user_id,
            Utc::now().naive_utc() + chrono::Duration::minutes(10),
            None,
        );

        let authorised = authorise_enrolment(&state, None, None, Some(challenge_id))
            .await
            .unwrap_or_else(|e| panic!("a live challenge must authorise: {}", e.message));
        assert_eq!(authorised.user_id, user_id);
        assert_eq!(authorised.ticket, Some(challenge_id));
    }

    /// The fence. A challenge minted for an account that already holds a
    /// factor is a challenge to *verify* it; spending it on an enrolment would
    /// be a way round the factor rather than a way to one.
    #[tokio::test]
    async fn a_challenge_for_an_enrolled_account_is_not_an_enrolment_ticket() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let user_id = insert_test_user(&mut conn);
        drop(conn);
        enrol(&state, user_id);

        let challenge_id = insert_challenge(
            &state,
            user_id,
            Utc::now().naive_utc() + chrono::Duration::minutes(10),
            None,
        );

        assert!(
            authorise_enrolment(&state, None, None, Some(challenge_id))
                .await
                .is_err(),
            "a verification challenge must not authorise an enrolment"
        );
    }

    /// Spent and expired tickets buy nothing, and a request carrying both
    /// kinds of proof is refused before either is evaluated.
    #[tokio::test]
    async fn a_spent_or_expired_or_doubled_authorisation_is_refused() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let user_id = insert_test_user(&mut conn);
        drop(conn);
        let now = Utc::now().naive_utc();

        let spent = insert_challenge(
            &state,
            user_id,
            now + chrono::Duration::minutes(10),
            Some(now),
        );
        assert!(
            authorise_enrolment(&state, None, None, Some(spent))
                .await
                .is_err(),
            "a consumed challenge must not authorise an enrolment"
        );

        let expired = insert_challenge(&state, user_id, now - chrono::Duration::minutes(1), None);
        assert!(
            authorise_enrolment(&state, None, None, Some(expired))
                .await
                .is_err(),
            "an expired challenge must not authorise an enrolment"
        );

        let live = insert_challenge(&state, user_id, now + chrono::Duration::minutes(10), None);
        assert!(
            authorise_enrolment(&state, Some("someone"), Some("hunter2"), Some(live))
                .await
                .is_err(),
            "a password and a ticket together is two attempts counted as one"
        );
        assert!(
            authorise_enrolment(&state, None, None, None).await.is_err(),
            "no proof at all authorises nothing"
        );
    }

    /// FR-008's rule in the ticket's own terms: confirmation spends it, and
    /// the second spend finds nothing to spend.
    #[tokio::test]
    async fn a_ticket_is_spent_exactly_once() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let user_id = insert_test_user(&mut conn);
        drop(conn);
        let now = Utc::now().naive_utc();
        let challenge_id =
            insert_challenge(&state, user_id, now + chrono::Duration::minutes(10), None);

        let mut conn = state.db_pool.get().unwrap();
        assert_eq!(
            consume_enrolment_ticket_sync(&mut conn, challenge_id, now).unwrap(),
            1
        );
        assert_eq!(
            consume_enrolment_ticket_sync(&mut conn, challenge_id, now).unwrap(),
            0,
            "the second spend must find the ticket already gone"
        );
        drop(conn);

        assert!(
            authorise_enrolment(&state, None, None, Some(challenge_id))
                .await
                .is_err(),
            "and the spent ticket must stop authorising anything"
        );
    }
}
