//! Spec 041 US2 (FR-006 … FR-011): the codes that get somebody back in when
//! the phone is gone.
//!
//! Ten codes are issued when an enrolment is confirmed, handed over in exactly
//! one response body, and stored only as Argon2 hashes — the way the admin
//! bootstrap code is stored (`auth/admin_bootstrap.rs`), because a recovery
//! code is what that code is: a single-use, human-transcribed,
//! account-granting secret. Not the way a share code is stored, because a
//! share link exists to be shown again and a recovery code must never be.
//!
//! Lives at `auth/two_factor/recovery.rs` rather than as a top-level sibling
//! of `two_factor.rs`: research R11 makes `two_factor` a module directory, and
//! two top-level modules sharing every helper is the shape `auth/` was split
//! out of.

use super::*;

use crate::models::NewUserRecoveryCode;
use crate::schema::user_recovery_codes;

/// Ten codes to a set. A person files one sheet away and does not come back
/// for more until they ask.
pub(crate) const RECOVERY_CODE_SET_SIZE: usize = 10;

/// FR-011: three or fewer unspent codes is "running low", and is said out loud
/// rather than left for somebody to discover at the moment they need the
/// fourth.
pub(crate) const RECOVERY_CODES_LOW_WATERMARK: i64 = 3;

pub(crate) const RECOVERY_CODES_NOTICE: &str = "Keep these somewhere other than the device you just set up. Each works once. They will not be shown again.";

pub(crate) const RECOVERY_CODES_REPLACED_NOTICE: &str =
    "Every code from your previous set has stopped working.";

/// What is hashed, and what a presented code is compared against.
///
/// Somebody retyping a code off a piece of paper will not reproduce the
/// grouping, and may not reproduce the case. The hyphens and the capitals are
/// presentation; the twelve characters are the secret.
pub(crate) fn normalise_recovery_code(presented: &str) -> String {
    presented
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .map(|ch| ch.to_ascii_uppercase())
        .collect()
}

/// FR-011's number: how many of this account's codes are still spendable.
pub(crate) async fn count_unspent_recovery_codes(
    state: &AppState,
    user_id: uuid::Uuid,
) -> Result<i64, String> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;

    tokio::task::spawn_blocking(move || {
        user_recovery_codes::table
            .filter(user_recovery_codes::user_id.eq(user_id))
            .filter(user_recovery_codes::used_at.is_null())
            .count()
            .get_result::<i64>(&mut conn)
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())
    .and_then(|r| r.map_err(|_| "Failed to count recovery codes".to_string()))
}

pub(crate) fn recovery_codes_low(remaining: i64) -> bool {
    remaining <= RECOVERY_CODES_LOW_WATERMARK
}

/// Issue a fresh set, and destroy the old one.
///
/// FR-010 says every earlier code stops working, so the outstanding rows are
/// **deleted** rather than marked superseded: there is then nothing left for a
/// later query that forgets a filter to match. Both halves happen in one
/// transaction, so an account is never briefly without any way in.
///
/// The plaintext returned here is the only plaintext there will ever be. The
/// caller puts it in one response body; nothing writes it anywhere else.
pub(crate) async fn issue_recovery_codes(
    state: &AppState,
    user_id: uuid::Uuid,
) -> Result<Vec<String>, String> {
    let (codes, rows) = generate_recovery_code_rows(user_id)?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;

    tokio::task::spawn_blocking(move || {
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            replace_recovery_codes_sync(conn, user_id, &rows)
        })
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())
    .and_then(|r| r.map_err(|_| "Failed to issue recovery codes".to_string()))?;

    Ok(codes)
}

/// Ten fresh codes and the rows that will stand for them.
///
/// Split out from `issue_recovery_codes` so enrolment confirmation can put the
/// insert in the **same transaction** that promotes the pending secret
/// (FR-003, FR-006): an account is never enabled without its codes, and never
/// issued codes for an enrolment that did not take.
///
/// Ten Argon2 hashes are computed here, sequentially — peak memory is one
/// hash's worth rather than ten.
pub(crate) fn generate_recovery_code_rows(
    user_id: uuid::Uuid,
) -> Result<(Vec<String>, Vec<NewUserRecoveryCode>), String> {
    let now = Utc::now().naive_utc();
    let codes: Vec<String> = (0..RECOVERY_CODE_SET_SIZE)
        .map(|_| random_setup_code())
        .collect();

    let mut rows = Vec::with_capacity(codes.len());
    for code in &codes {
        rows.push(NewUserRecoveryCode {
            id: uuid::Uuid::now_v7(),
            user_id,
            code_hash: hash_password(&normalise_recovery_code(code))?,
            used_at: None,
            created_at: now,
        });
    }

    Ok((codes, rows))
}

/// Delete every outstanding code for the account, then insert the new set.
///
/// The delete is FR-010 in one statement: "every earlier code stops working"
/// is true because there is nothing left to match, not because a later query
/// remembered to filter on a flag. Callers run this inside a transaction.
/// Remove every recovery code an account holds.
///
/// Separate from [`replace_recovery_codes_sync`] because removal is not a
/// replacement with an empty set: the caller that turns a factor off wants the
/// codes gone, and expressing that as "replace with nothing" would put an
/// insert of zero rows in the path and invite somebody to optimise it away
/// along with the delete.
pub(crate) fn delete_recovery_codes_sync(
    conn: &mut PgConnection,
    user_id: uuid::Uuid,
) -> QueryResult<()> {
    diesel::delete(user_recovery_codes::table.filter(user_recovery_codes::user_id.eq(user_id)))
        .execute(conn)?;
    Ok(())
}

pub(crate) fn replace_recovery_codes_sync(
    conn: &mut PgConnection,
    user_id: uuid::Uuid,
    rows: &[NewUserRecoveryCode],
) -> QueryResult<()> {
    diesel::delete(user_recovery_codes::table.filter(user_recovery_codes::user_id.eq(user_id)))
        .execute(conn)?;
    diesel::insert_into(user_recovery_codes::table)
        .values(rows)
        .execute(conn)?;
    Ok(())
}

/// Spend a recovery code, or refuse.
///
/// Two properties this function exists to hold:
///
/// 1. **Constant work.** Every unspent code is verified, sequentially, with no
///    early exit even after one matches (FR-018). Returning the moment a match
///    is found would make "matched the second of ten" measurably faster than
///    "matched none", and that difference is an oracle for how many codes an
///    account has and how close a guess came. Sequential rather than parallel
///    keeps peak memory at one Argon2 hash rather than ten.
/// 2. **Exactly once.** The spend is a conditional write, not a read followed
///    by a write: `SET used_at = now() WHERE id = $1 AND used_at IS NULL`, and
///    zero rows updated is the refusal (FR-008). Two simultaneous
///    presentations of one code cannot both win.
///
/// An account with no codes at all takes the same refusal as a wrong code, and
/// says nothing about which it was.
pub(crate) async fn consume_recovery_code(
    state: &AppState,
    user_id: uuid::Uuid,
    presented: &str,
) -> Result<bool, String> {
    let presented = normalise_recovery_code(presented);
    if presented.is_empty() {
        return Ok(false);
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;

    tokio::task::spawn_blocking(move || {
        let candidates = user_recovery_codes::table
            .filter(user_recovery_codes::user_id.eq(user_id))
            .filter(user_recovery_codes::used_at.is_null())
            .order(user_recovery_codes::created_at.asc())
            .select((user_recovery_codes::id, user_recovery_codes::code_hash))
            .load::<(uuid::Uuid, String)>(&mut conn)
            .map_err(|_| "Failed to load recovery codes".to_string())?;

        let mut matched: Option<uuid::Uuid> = None;
        for (id, stored) in &candidates {
            // No `break`, and no `?` that could leave early either: a row whose
            // stored hash will not parse is a corrupt row, not a reason to stop
            // hashing.
            let verified = PasswordHash::new(stored)
                .ok()
                .map(|parsed| {
                    Argon2::default()
                        .verify_password(presented.as_bytes(), &parsed)
                        .is_ok()
                })
                .unwrap_or(false);
            if verified && matched.is_none() {
                matched = Some(*id);
            }
        }

        let Some(matched) = matched else {
            return Ok(false);
        };

        let spent = diesel::update(
            user_recovery_codes::table
                .filter(user_recovery_codes::id.eq(matched))
                .filter(user_recovery_codes::used_at.is_null()),
        )
        .set(user_recovery_codes::used_at.eq(Some(Utc::now().naive_utc())))
        .execute(&mut conn)
        .map_err(|_| "Failed to spend recovery code".to_string())?;

        if spent == 1 {
            // FR-015. The row says a code was used; it never says which, and
            // there is nowhere in this table's shape to put that even by
            // accident.
            let _ = super::events::record_sync(
                &mut conn,
                user_id,
                Some(user_id),
                super::events::event_type::RECOVERY_CODE_USED,
            );
        }

        Ok(spent == 1)
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())?
}

/// `POST /authentication/2fa/recovery-codes` — a fresh set.
///
/// A fresh set is a fresh way in, so it costs possession exactly as removal
/// does: the session proves the password was held at sign-in, and the request
/// carries either a current authenticator code or one of the outstanding
/// recovery codes. Handing a set out on the session alone would rebuild the
/// hole FR-012 closes, one indirection further away.
pub(crate) async fn regenerate_recovery_codes(
    cookies: Cookies,
    State(state): State<AppState>,
    Json(request): Json<RecoveryCodesRegenerateRequest>,
) -> (StatusCode, Json<RecoveryCodesResponse>) {
    let Ok(authenticated) = resolve_authenticated_user(&state, &cookies).await else {
        return recovery_codes_error(
            StatusCode::UNAUTHORIZED,
            "failure",
            "Authentication required",
        );
    };
    let user_id = authenticated.user_id;

    // FR-018, and the reason the two fields are never both read: a request
    // carrying both is asking for two chances counted as one attempt.
    let proof = match (
        request.code.as_deref().filter(|v| !v.trim().is_empty()),
        request
            .recovery_code
            .as_deref()
            .filter(|v| !v.trim().is_empty()),
    ) {
        (Some(_), Some(_)) | (None, None) => {
            return recovery_codes_error(
                StatusCode::BAD_REQUEST,
                "two_factor_invalid",
                "Provide exactly one of a 2FA code or a recovery code",
            );
        }
        (Some(code), None) => TwoFactorProof::Totp(code.to_string()),
        (None, Some(code)) => TwoFactorProof::Recovery(code.to_string()),
    };

    // FR-017. A fresh set of codes is a fresh way in, so guessing at the
    // possession this route asks for is worth an attacker's time — and before
    // the bound moved under the verification, this route did not consult it.
    let held = super::throttle::guarded(&state, user_id, || async {
        match proof {
            TwoFactorProof::Totp(ref code) => {
                verify_two_factor_for_user(&state, user_id, code).await
            }
            TwoFactorProof::Recovery(ref code) => {
                consume_recovery_code(&state, user_id, code).await
            }
        }
    })
    .await;

    if matches!(held, Ok(super::throttle::SecondFactor::Throttled)) {
        return recovery_codes_error(
            StatusCode::TOO_MANY_REQUESTS,
            "two_factor_throttled",
            "Too many incorrect codes. Wait a moment and try again.",
        );
    }

    match held.map(|outcome| outcome == super::throttle::SecondFactor::Held) {
        Ok(true) => {}
        Ok(false) => {
            return recovery_codes_error(
                StatusCode::UNAUTHORIZED,
                "two_factor_invalid",
                crate::auth::two_factor::verification::CREDENTIAL_REFUSED,
            );
        }
        Err(msg) => {
            return recovery_codes_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "two_factor_error",
                msg.as_str(),
            );
        }
    }

    match issue_recovery_codes(&state, user_id).await {
        Ok(codes) => {
            let remaining = codes.len() as i64;
            // FR-015. Every earlier code has just stopped working, which is a
            // thing the account holder needs to know about even — especially —
            // if they did not do it.
            super::notify::tell(
                &state,
                user_id,
                super::notify::Change::RecoveryCodesReissued,
            )
            .await;
            (
                StatusCode::OK,
                Json(RecoveryCodesResponse {
                    status: "success",
                    message: "Recovery codes issued".to_string(),
                    recovery_codes: Some(codes),
                    recovery_codes_notice: Some(RECOVERY_CODES_REPLACED_NOTICE.to_string()),
                    recovery_codes_remaining: Some(remaining),
                    recovery_codes_low: Some(recovery_codes_low(remaining)),
                }),
            )
        }
        Err(msg) => recovery_codes_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "two_factor_error",
            msg.as_str(),
        ),
    }
}

/// Which of the two second-factor proofs a request carried. Never both — the
/// caller refuses that before either is evaluated.
enum TwoFactorProof {
    Totp(String),
    Recovery(String),
}

/// Prove possession of the account's second factor, by either accepted means.
///
/// Extracted so that turning a factor **off** and issuing fresh recovery codes
/// ask the same question in the same way. Two implementations of "prove you
/// still hold it" is one more than this product should have, and they would
/// drift in the direction of whichever was edited last.
///
/// `Err` carries a message the caller maps to a status: the "exactly one"
/// refusal is the caller's bad request, anything else is the server's fault.
///
/// FR-018's rule is enforced here rather than by each caller: a request
/// carrying **both** a code and a recovery code is asking for two chances
/// counted as one attempt, and is refused before either is evaluated.
pub(crate) async fn verify_second_factor_proof(
    state: &AppState,
    user_id: uuid::Uuid,
    code: Option<&str>,
    recovery_code: Option<&str>,
) -> Result<bool, String> {
    let proof = match (
        code.filter(|v| !v.trim().is_empty()),
        recovery_code.filter(|v| !v.trim().is_empty()),
    ) {
        (Some(_), Some(_)) | (None, None) => {
            return Err("Provide exactly one of a 2FA code or a recovery code".to_string());
        }
        (Some(code), None) => TwoFactorProof::Totp(code.to_string()),
        (None, Some(code)) => TwoFactorProof::Recovery(code.to_string()),
    };

    // FR-017, same reasoning: turning a factor *off* is the action that makes
    // every future sign-in cheaper, so it is the last place to leave a
    // guessable proof unbounded.
    super::throttle::guarded(state, user_id, || async {
        match proof {
            TwoFactorProof::Totp(ref code) => {
                verify_two_factor_for_user(state, user_id, code).await
            }
            TwoFactorProof::Recovery(ref code) => consume_recovery_code(state, user_id, code).await,
        }
    })
    .await
    .map(|outcome| outcome == super::throttle::SecondFactor::Held)
}

fn recovery_codes_error(
    code: StatusCode,
    status: &'static str,
    message: &str,
) -> (StatusCode, Json<RecoveryCodesResponse>) {
    (
        code,
        Json(RecoveryCodesResponse {
            status,
            message: message.to_string(),
            recovery_codes: None,
            recovery_codes_notice: None,
            recovery_codes_remaining: None,
            recovery_codes_low: None,
        }),
    )
}

/// What this account's own second factor looks like, to its owner
/// (spec 041 FR-005).
///
/// # Why this exists
///
/// The enrolment interface has to answer "is this already on?" before it can
/// offer to turn it on, and nothing could answer it: `PublicUser` carries no
/// two-factor field, and the only GraphQL one is the *instance-wide* policy.
/// So the panel either had to claim two-factor was off — which is a lie to
/// somebody who has it on — or say it could not tell.
///
/// Self-only, like every other field on this surface. There is no argument by
/// which one account could ask about another's, and adding one would be a
/// different feature with a different review.
pub(crate) async fn two_factor_status(
    cookies: Cookies,
    State(state): State<AppState>,
) -> (StatusCode, Json<TwoFactorStatusResponse>) {
    let Ok(authenticated) = resolve_authenticated_user(&state, &cookies).await else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(TwoFactorStatusResponse {
                status: "failure",
                message: "Authentication required".to_string(),
                enabled: false,
                confirmed_at: None,
                enrolment_pending: false,
                recovery_codes_remaining: None,
                recovery_codes_low: false,
            }),
        );
    };
    let user_id = authenticated.user_id;

    let mut conn = state.db_pool.get().expect("Failed to get DB connection");
    let row = tokio::task::spawn_blocking(move || {
        users::table
            .filter(users::id.eq(user_id))
            .select((
                users::two_factor_enabled,
                users::two_factor_confirmed_at,
                users::two_factor_pending_secret_encrypted,
            ))
            .first::<(bool, Option<chrono::NaiveDateTime>, Option<String>)>(&mut conn)
            .optional()
    })
    .await
    .expect("Failed to spawn blocking task")
    .expect("Failed to read the account");

    let Some((enabled, confirmed_at, pending)) = row else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(TwoFactorStatusResponse {
                status: "failure",
                message: "Authentication required".to_string(),
                enabled: false,
                confirmed_at: None,
                enrolment_pending: false,
                recovery_codes_remaining: None,
                recovery_codes_low: false,
            }),
        );
    };

    // Only meaningful once a factor is actually in force. Reporting a count
    // for an account with no second factor would invite a screen that offers
    // to regenerate codes that protect nothing.
    let remaining = if enabled {
        count_unspent_recovery_codes(&state, user_id).await.ok()
    } else {
        None
    };

    (
        StatusCode::OK,
        Json(TwoFactorStatusResponse {
            status: "success",
            message: String::new(),
            enabled,
            confirmed_at: confirmed_at.map(|at| at.to_string()),
            // An enrolment in progress is not a second factor. Saying so lets
            // the panel offer "finish what you started" rather than starting
            // again — and ADR-081 is what makes the distinction real.
            enrolment_pending: pending.is_some(),
            recovery_codes_remaining: remaining,
            recovery_codes_low: remaining.is_some_and(recovery_codes_low),
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{insert_test_user, test_app_state};

    /// The whole set as it is stored — hashes and spend marks, never codes.
    fn stored_rows(
        state: &AppState,
        user_id: uuid::Uuid,
    ) -> Vec<(String, Option<chrono::NaiveDateTime>)> {
        let mut conn = state.db_pool.get().unwrap();
        user_recovery_codes::table
            .filter(user_recovery_codes::user_id.eq(user_id))
            .order(user_recovery_codes::created_at.asc())
            .select((user_recovery_codes::code_hash, user_recovery_codes::used_at))
            .load(&mut conn)
            .expect("failed to read the stored recovery codes")
    }

    /// FR-006, FR-008. The two halves of what a recovery code is: it gets a
    /// person in, and then it does not get anybody in ever again.
    #[tokio::test]
    async fn a_recovery_code_admits_once_and_is_refused_the_second_time() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let user_id = insert_test_user(&mut conn);
        drop(conn);

        let codes = issue_recovery_codes(&state, user_id)
            .await
            .expect("confirmation issues a set");
        assert_eq!(codes.len(), RECOVERY_CODE_SET_SIZE);

        assert!(
            consume_recovery_code(&state, user_id, &codes[3])
                .await
                .expect("the spend must not error"),
            "a freshly issued code must admit"
        );
        assert!(
            !consume_recovery_code(&state, user_id, &codes[3])
                .await
                .expect("the second attempt must not error"),
            "the same code must be refused the second time — a spent code is spent"
        );

        // And spending one costs exactly one: the other nine are untouched.
        assert_eq!(
            count_unspent_recovery_codes(&state, user_id).await.unwrap(),
            RECOVERY_CODE_SET_SIZE as i64 - 1
        );
        assert!(
            consume_recovery_code(&state, user_id, &codes[4])
                .await
                .unwrap(),
            "spending one code must not disturb the rest of the set"
        );
    }

    /// The guard whose absence is invisible from outside: **verification does
    /// the same work whichever code was offered.**
    ///
    /// `consume_recovery_code` verifies against every unspent hash with no
    /// early exit. Add a `break` on first match and nothing observable
    /// changes — the right codes still admit, the wrong ones are still
    /// refused, and every other test in this file still passes. What changes
    /// is that the *time taken* tells the caller how far down the set their
    /// code was, which for a ten-code set is three bits of a credential handed
    /// over for free.
    ///
    /// So this is a timing test, which is a thing to be careful about. It is
    /// written to be safe rather than tight: Argon2 is deliberately expensive
    /// — tens of milliseconds a verification — so with ten codes the gap
    /// between "stopped at the first" and "checked all ten" is close to
    /// tenfold. Asserting merely that the first is not *dramatically* faster
    /// than the last leaves an enormous margin for a loaded machine, and still
    /// fails an early `break` by a mile.
    ///
    /// If this ever goes flaky, raise the tolerance rather than delete it. The
    /// alternative is a guarantee nothing checks.
    #[tokio::test]
    async fn matching_the_first_code_costs_what_matching_the_last_one_costs() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let first_user = insert_test_user(&mut conn);
        let last_user = insert_test_user(&mut conn);
        drop(conn);

        let first_set = issue_recovery_codes(&state, first_user).await.unwrap();
        let last_set = issue_recovery_codes(&state, last_user).await.unwrap();

        // Warm the process: the very first Argon2 in a test binary pays for
        // allocation the rest do not, and attributing that to the loop would
        // be measuring the wrong thing.
        let _ = consume_recovery_code(&state, first_user, "not-a-code").await;

        let started = std::time::Instant::now();
        assert!(
            consume_recovery_code(&state, first_user, &first_set[0])
                .await
                .unwrap()
        );
        let cost_of_the_first = started.elapsed();

        let started = std::time::Instant::now();
        assert!(
            consume_recovery_code(&state, last_user, &last_set[RECOVERY_CODE_SET_SIZE - 1])
                .await
                .unwrap()
        );
        let cost_of_the_last = started.elapsed();

        // Four times, against a real ratio of about ten. Wide enough that a
        // busy machine does not fail it; narrow enough that an early exit
        // cannot hide in it.
        assert!(
            cost_of_the_last < cost_of_the_first * 4,
            "matching the last code took {cost_of_the_last:?} against \
             {cost_of_the_first:?} for the first. That difference is the \
             position of a code in the set, told to whoever offered it — \
             `consume_recovery_code` must verify every unspent hash with no \
             early exit."
        );
    }

    /// FR-011. The count is what tells somebody they are running low, so it
    /// has to fall as codes are spent and reset when a set is replaced.
    #[tokio::test]
    async fn the_remaining_count_falls_with_use_and_reports_running_low() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let user_id = insert_test_user(&mut conn);
        drop(conn);

        assert_eq!(
            count_unspent_recovery_codes(&state, user_id).await.unwrap(),
            0,
            "an account that never enrolled has no codes"
        );

        let codes = issue_recovery_codes(&state, user_id).await.unwrap();
        assert_eq!(
            count_unspent_recovery_codes(&state, user_id).await.unwrap(),
            RECOVERY_CODE_SET_SIZE as i64
        );
        assert!(!recovery_codes_low(RECOVERY_CODE_SET_SIZE as i64));

        for code in codes.iter().take(RECOVERY_CODE_SET_SIZE - 3) {
            assert!(consume_recovery_code(&state, user_id, code).await.unwrap());
        }

        let remaining = count_unspent_recovery_codes(&state, user_id).await.unwrap();
        assert_eq!(remaining, RECOVERY_CODES_LOW_WATERMARK);
        assert!(
            recovery_codes_low(remaining),
            "three left is the point at which the person is told"
        );
    }

    /// FR-010. "Every earlier code stops working" is the requirement, and
    /// deleting rather than marking is why it cannot be undone by a query that
    /// forgets a filter — so the old rows are gone, not merely ignored.
    #[tokio::test]
    async fn a_fresh_set_stops_every_code_from_the_old_one() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let user_id = insert_test_user(&mut conn);
        drop(conn);

        let first = issue_recovery_codes(&state, user_id).await.unwrap();
        let second = issue_recovery_codes(&state, user_id).await.unwrap();

        for code in &first {
            assert!(
                !consume_recovery_code(&state, user_id, code).await.unwrap(),
                "a code from the replaced set must not admit"
            );
        }
        assert!(
            consume_recovery_code(&state, user_id, &second[0])
                .await
                .unwrap(),
            "the new set must work"
        );

        assert_eq!(
            stored_rows(&state, user_id).len(),
            RECOVERY_CODE_SET_SIZE,
            "the replaced rows must be deleted, not kept as dead hashes"
        );
    }

    /// The refusal that has to hold when there is nothing to match against.
    ///
    /// An empty set is the state of every account that never enrolled, and of
    /// one that has spent all ten. "No codes" must never collapse into "no
    /// check" — and the answer is the same shape as a wrong code, saying
    /// nothing about which case it was (FR-018).
    #[tokio::test]
    async fn an_account_with_no_codes_is_never_admitted() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let user_id = insert_test_user(&mut conn);
        drop(conn);

        for presented in ["", "   ", "----", "AAAA-BBBB-CCCC"] {
            assert!(
                !consume_recovery_code(&state, user_id, presented)
                    .await
                    .unwrap(),
                "an account with no codes must refuse {presented:?}"
            );
        }

        // And once a set exists, a code belonging to somebody else is refused
        // in exactly the same way — never "wrong account".
        let mut conn = state.db_pool.get().unwrap();
        let stranger = insert_test_user(&mut conn);
        drop(conn);
        let theirs = issue_recovery_codes(&state, stranger).await.unwrap();
        issue_recovery_codes(&state, user_id).await.unwrap();

        assert!(
            !consume_recovery_code(&state, user_id, &theirs[0])
                .await
                .unwrap(),
            "another account's code must not admit"
        );
        assert!(
            consume_recovery_code(&state, stranger, &theirs[0])
                .await
                .unwrap(),
            "and presenting it must not have spent it for its owner"
        );
    }

    /// FR-009. The instance cannot show a code again because it does not have
    /// one — not because a rule says not to. Assert the property directly:
    /// nothing stored resembles what was handed over.
    #[tokio::test]
    async fn what_is_stored_is_a_hash_and_never_the_code() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let user_id = insert_test_user(&mut conn);
        drop(conn);

        let codes = issue_recovery_codes(&state, user_id).await.unwrap();
        let rows = stored_rows(&state, user_id);
        assert_eq!(rows.len(), RECOVERY_CODE_SET_SIZE);

        for (stored, used_at) in &rows {
            assert!(
                stored.starts_with("$argon2id$"),
                "expected an Argon2id PHC string, got {stored}"
            );
            assert!(used_at.is_none(), "a freshly issued code is unspent");
            for code in &codes {
                let normalised = normalise_recovery_code(code);
                assert!(
                    !stored.contains(code) && !stored.contains(&normalised),
                    "no fragment of a code may survive in storage"
                );
            }
        }

        // Individually salted, so ten codes do not become one hash to break.
        let hashes: std::collections::HashSet<&String> = rows.iter().map(|(h, _)| h).collect();
        assert_eq!(hashes.len(), RECOVERY_CODE_SET_SIZE);
    }

    /// A person reads a code off a sheet of paper at the worst moment of their
    /// week. The hyphens and the capitals are how it was printed, not what it
    /// is, and a retyped `4kjh92mxqw3t` is the same credential.
    #[tokio::test]
    async fn a_code_is_matched_however_the_person_retypes_it() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let user_id = insert_test_user(&mut conn);
        drop(conn);

        let codes = issue_recovery_codes(&state, user_id).await.unwrap();
        let retyped = format!("  {}  ", codes[0].to_lowercase().replace('-', " "));

        assert!(
            consume_recovery_code(&state, user_id, &retyped)
                .await
                .unwrap(),
            "grouping and case are presentation; the twelve characters are the secret"
        );
    }

    /// The generated shape, asserted here rather than assumed: twelve
    /// characters from an alphabet with no `I`, `O`, `0` or `1` in it, because
    /// these get written down and read back.
    #[test]
    fn codes_are_transcribable_and_ten_to_a_set() {
        let (codes, rows) =
            generate_recovery_code_rows(uuid::Uuid::now_v7()).expect("a set is generated");

        assert_eq!(codes.len(), RECOVERY_CODE_SET_SIZE);
        assert_eq!(rows.len(), RECOVERY_CODE_SET_SIZE);
        assert_eq!(
            codes.iter().collect::<std::collections::HashSet<_>>().len(),
            RECOVERY_CODE_SET_SIZE,
            "two identical codes in one set halves it"
        );

        for code in &codes {
            assert_eq!(code.len(), 14, "XXXX-XXXX-XXXX");
            assert_eq!(normalise_recovery_code(code).len(), 12);
            assert!(
                code.chars()
                    .all(|ch| ch == '-' || (ch.is_ascii_uppercase() || ch.is_ascii_digit())),
                "{code} must be typeable without a keyboard hunt"
            );
            assert!(
                !code.contains(['I', 'O', '0', '1']),
                "{code} contains a character that is read back wrong"
            );
        }
    }
}
