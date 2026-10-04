//! Reading and ending the sessions one account holds (spec 036 US4).
//!
//! Split from `sessions.rs`, which is about *becoming* signed in. This is
//! about what being signed in several times leaves behind, and what a person
//! can do about it.
//!
//! Every function here takes the caller's own `user_id` and scopes to it. A
//! session id is never trusted on its own — the `user_id` filter is what
//! makes "one account cannot reach another's sessions" a property of the
//! query rather than a check somebody has to remember.
//!
//! # What is deliberately absent
//!
//! FR-008 — a password change ends every session but the one that made it —
//! has nothing to attach to: **this product has no password-change path at
//! all.** There is no endpoint, no mutation and no screen; `password_hash` is
//! only ever written at registration, admin bootstrap, or OAuth
//! auto-provisioning. Writing the function now would leave an unused one
//! whose correctness nobody could check, so it is recorded here and belongs
//! with the change-password feature when that exists.

use chrono::Utc;
use diesel::prelude::*;

use crate::AppState;
use crate::auth::sessions::ended_reason;
use crate::schema::user_sessions;

/// One live session as its owner sees it.
pub struct SessionSummary {
    pub id: uuid::Uuid,
    pub created_at: chrono::NaiveDateTime,
    pub last_seen_at: chrono::NaiveDateTime,
    pub expires_at: chrono::NaiveDateTime,
    pub client_description: Option<String>,
    pub is_current: bool,
}

/// Every live session for one account, most recently used first.
///
/// "Live" means what `resolve_authenticated_user` means by it — not revoked
/// and not expired — so this list and the thing that decides whether a
/// request is authenticated cannot disagree about what exists.
pub async fn live_sessions(
    state: &AppState,
    user_id: uuid::Uuid,
    current_session_id: uuid::Uuid,
) -> Result<Vec<SessionSummary>, String> {
    let now = Utc::now().naive_utc();
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;

    let rows = tokio::task::spawn_blocking(move || {
        user_sessions::table
            .filter(user_sessions::user_id.eq(user_id))
            .filter(user_sessions::revoked_at.is_null())
            .filter(user_sessions::expires_at.gt(now))
            .order(user_sessions::last_seen_at.desc())
            .select((
                user_sessions::id,
                user_sessions::created_at,
                user_sessions::last_seen_at,
                user_sessions::expires_at,
                user_sessions::client_description,
            ))
            .load::<(
                uuid::Uuid,
                chrono::NaiveDateTime,
                chrono::NaiveDateTime,
                chrono::NaiveDateTime,
                Option<String>,
            )>(&mut conn)
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())?
    .map_err(|_| "Failed to load sessions".to_string())?;

    Ok(rows
        .into_iter()
        .map(
            |(id, created_at, last_seen_at, expires_at, client_description)| SessionSummary {
                id,
                created_at,
                last_seen_at,
                expires_at,
                client_description,
                is_current: id == current_session_id,
            },
        )
        .collect())
}

/// End one session belonging to this account.
///
/// Scoped by `user_id`, so a session id belonging to somebody else matches no
/// row and changes nothing — which is also why the caller reports the same
/// result either way. Returns how many rows were ended, for the tests; the
/// GraphQL surface deliberately does not pass that on.
pub async fn end_session(
    state: &AppState,
    user_id: uuid::Uuid,
    session_id: uuid::Uuid,
) -> Result<usize, String> {
    let now = Utc::now().naive_utc();
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;

    tokio::task::spawn_blocking(move || {
        diesel::update(
            user_sessions::table
                .filter(user_sessions::id.eq(session_id))
                .filter(user_sessions::user_id.eq(user_id))
                .filter(user_sessions::revoked_at.is_null()),
        )
        .set((
            user_sessions::revoked_at.eq(Some(now)),
            user_sessions::ended_reason.eq(Some(ended_reason::ENDED_BY_USER)),
        ))
        .execute(&mut conn)
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())?
    .map_err(|_| "Failed to end the session".to_string())
}

/// End every live session for this account, including the one asking.
pub async fn end_all_sessions(state: &AppState, user_id: uuid::Uuid) -> Result<i32, String> {
    let now = Utc::now().naive_utc();
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;

    let ended = tokio::task::spawn_blocking(move || {
        diesel::update(
            user_sessions::table
                .filter(user_sessions::user_id.eq(user_id))
                .filter(user_sessions::revoked_at.is_null()),
        )
        .set((
            user_sessions::revoked_at.eq(Some(now)),
            user_sessions::ended_reason.eq(Some(ended_reason::ENDED_BY_USER)),
        ))
        .execute(&mut conn)
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())?
    .map_err(|_| "Failed to end the sessions".to_string())?;

    Ok(i32::try_from(ended).unwrap_or(i32::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::ClientDescription;
    use crate::auth::sessions::issue_session_cookie;
    use crate::test_support::{insert_test_user, test_app_state};
    use tower_cookies::Cookies;

    async fn account_with_three_sessions() -> (AppState, uuid::Uuid, Vec<uuid::Uuid>) {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let user_id = insert_test_user(&mut conn);
        drop(conn);

        let mut ids = Vec::new();
        for _ in 0..3 {
            ids.push(
                issue_session_cookie(
                    &state,
                    &Cookies::default(),
                    user_id,
                    ClientDescription::unknown(),
                )
                .await
                .expect("a session")
                .id,
            );
        }
        (state, user_id, ids)
    }

    /// FR-005. Three sessions, three rows, and the caller knows which is theirs.
    #[tokio::test]
    async fn a_person_sees_their_own_live_sessions_and_which_one_they_are_on() {
        let (state, user_id, ids) = account_with_three_sessions().await;

        let listed = live_sessions(&state, user_id, ids[1]).await.unwrap();

        assert_eq!(listed.len(), 3);
        let current: Vec<uuid::Uuid> = listed
            .iter()
            .filter(|s| s.is_current)
            .map(|s| s.id)
            .collect();
        assert_eq!(
            current,
            vec![ids[1]],
            "exactly one session is the current one"
        );
    }

    /// FR-005's "where from", and spec 035's limit on it. The row a person
    /// reads names the client coarsely and carries nothing that identifies
    /// *them* — no address, no device name, no version. The description is
    /// built in `thunderforge_axum_auth_core::client_description` from a fixed
    /// vocabulary; this asserts it survives the round trip through the column
    /// and the summary unchanged, which is the join the unit tests there
    /// cannot see.
    #[tokio::test]
    async fn a_listed_session_names_the_client_and_never_the_person() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let user_id = insert_test_user(&mut conn);
        drop(conn);

        let described = issue_session_cookie(
            &state,
            &Cookies::default(),
            user_id,
            ClientDescription(Some("Firefox on Linux".to_string())),
        )
        .await
        .expect("a described session");
        let anonymous = issue_session_cookie(
            &state,
            &Cookies::default(),
            user_id,
            ClientDescription::unknown(),
        )
        .await
        .expect("an undescribed session");

        let listed = live_sessions(&state, user_id, described.id).await.unwrap();

        let row = |id: uuid::Uuid| {
            listed
                .iter()
                .find(|s| s.id == id)
                .unwrap_or_else(|| panic!("session {id} is listed"))
        };
        assert_eq!(
            row(described.id).client_description.as_deref(),
            Some("Firefox on Linux"),
        );
        assert_eq!(
            row(anonymous.id).client_description,
            None,
            "a client we did not recognise stays unnamed rather than guessed at",
        );

        for summary in &listed {
            let rendered = summary.client_description.clone().unwrap_or_default();
            for leak in ["127.0.0.1", "::1", "@", "Mozilla", "/"] {
                assert!(
                    !rendered.contains(leak),
                    "{rendered:?} carried {leak:?} into the session list",
                );
            }
        }
    }

    /// FR-003 and FR-006. Ending one leaves the others alone — the property
    /// the login-time revoke did not have.
    #[tokio::test]
    async fn ending_one_session_leaves_the_others_live() {
        let (state, user_id, ids) = account_with_three_sessions().await;

        let ended = end_session(&state, user_id, ids[0]).await.unwrap();
        assert_eq!(ended, 1);

        let remaining: Vec<uuid::Uuid> = live_sessions(&state, user_id, ids[1])
            .await
            .unwrap()
            .into_iter()
            .map(|s| s.id)
            .collect();
        assert_eq!(remaining.len(), 2);
        assert!(!remaining.contains(&ids[0]));
    }

    /// A session id the caller does not own must change nothing — and must be
    /// indistinguishable from one that never existed.
    #[tokio::test]
    async fn one_account_cannot_end_anothers_session() {
        let (state, _owner, ids) = account_with_three_sessions().await;
        let mut conn = state.db_pool.get().unwrap();
        let stranger = insert_test_user(&mut conn);
        drop(conn);

        let ended = end_session(&state, stranger, ids[0]).await.unwrap();
        let invented = end_session(&state, stranger, uuid::Uuid::now_v7())
            .await
            .unwrap();

        assert_eq!(ended, 0, "a stranger must not end somebody else's session");
        assert_eq!(
            ended, invented,
            "a real session id and an invented one must be indistinguishable"
        );
    }

    /// FR-007. Everything, including the caller's own.
    #[tokio::test]
    async fn ending_them_all_includes_the_one_that_asked() {
        let (state, user_id, ids) = account_with_three_sessions().await;

        let ended = end_all_sessions(&state, user_id).await.unwrap();
        assert_eq!(ended, 3);
        assert!(
            live_sessions(&state, user_id, ids[0])
                .await
                .unwrap()
                .is_empty()
        );
    }
}
