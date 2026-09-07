//! Spec 036 US4: the sessions a person holds, and ending them.
//!
//! # Why this exists at all
//!
//! Until ADR-073, signing in ended every session that came before it. That
//! was the whole of the account-security story for sessions: a person who
//! suspected a stolen cookie logged in again, and the suspicion was resolved
//! as a side effect.
//!
//! Removing the eviction removes that, so this replaces it with the control
//! it was approximating — a person can see what is signed in as them and end
//! the one they do not recognise. It is a weaker mechanism only in the sense
//! that it requires somebody to act; it is a far stronger one in that they
//! can act deliberately, on one session, without disturbing the others.
//!
//! # Every field here is about the caller's own sessions
//!
//! There is no field by which one account reaches another's, and adding one
//! would be a different feature with a different review. `end_session`
//! answers identically for a session belonging to somebody else and one that
//! never existed, on the rule spec 027 set for dead links: a refusal that
//! distinguishes them is an oracle for which session ids are real.

use async_graphql::{Context, Object, Result as GraphQLResult};

use crate::auth::session_registry::{self, SessionSummary};
use crate::graphql::helpers::{app_state, authenticated_user};

/// One live session, as its owner sees it.
#[derive(async_graphql::SimpleObject)]
pub struct GraphQLUserSession {
    pub id: uuid::Uuid,
    pub created_at: String,
    pub last_seen_at: String,
    pub expires_at: String,
    /// Coarse origin — a browser and a platform, never an address.
    pub client_description: Option<String>,
    /// True for the session making this request.
    pub is_current: bool,
}

impl From<SessionSummary> for GraphQLUserSession {
    fn from(summary: SessionSummary) -> Self {
        Self {
            id: summary.id,
            created_at: summary.created_at.to_string(),
            last_seen_at: summary.last_seen_at.to_string(),
            expires_at: summary.expires_at.to_string(),
            client_description: summary.client_description,
            is_current: summary.is_current,
        }
    }
}

#[derive(Default)]
pub struct SessionQuery;

#[Object]
impl SessionQuery {
    /// Every live session for the calling account, most recently used first.
    async fn my_sessions(&self, ctx: &Context<'_>) -> GraphQLResult<Vec<GraphQLUserSession>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        Ok(
            session_registry::live_sessions(state, auth_user.user_id, auth_user.session_id)
                .await
                .map_err(async_graphql::Error::new)?
                .into_iter()
                .map(GraphQLUserSession::from)
                .collect(),
        )
    }
}

#[derive(Default)]
pub struct SessionMutation;

#[Object]
impl SessionMutation {
    /// End one of the caller's own sessions. Ending the current one signs it
    /// out; ending any other leaves this one working.
    ///
    /// Returns `true` whether or not a row was ended. A session id belonging
    /// to somebody else and one that never existed are the same answer on
    /// purpose — see the module header.
    async fn end_session(&self, ctx: &Context<'_>, session_id: uuid::Uuid) -> GraphQLResult<bool> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        session_registry::end_session(state, auth_user.user_id, session_id)
            .await
            .map_err(async_graphql::Error::new)?;
        Ok(true)
    }

    /// End every session for the calling account, including this one. The
    /// control a person uses after losing a device.
    async fn end_all_sessions(&self, ctx: &Context<'_>) -> GraphQLResult<i32> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        session_registry::end_all_sessions(state, auth_user.user_id)
            .await
            .map_err(async_graphql::Error::new)
    }
}

#[cfg(test)]
mod tests {
    /// A surface that compiles but was never merged into the root fails for
    /// the first person trying to end a session they do not recognise, not
    /// for the suite. `mutations_instance_access.rs` keeps the same guard for
    /// the same reason.
    #[test]
    fn the_session_surface_is_registered_under_the_names_the_client_uses() {
        let schema = async_graphql::Schema::build(
            crate::graphql::QueryRoot::default(),
            crate::graphql::MutationRoot::default(),
            crate::graphql::SubscriptionRoot,
        )
        .finish();
        let sdl = schema.sdl();

        for field in [
            "mySessions",
            "endSession(sessionId: UUID!",
            "endAllSessions",
        ] {
            assert!(
                sdl.contains(field),
                "`{field}` must be reachable from the root"
            );
        }

        // Matched against a field *declaration* — SDL indents fields with a
        // tab — rather than anywhere in the text, because the doc comments
        // above these fields describe the thing being forbidden.
        //
        // No field takes a user id. Every one of these is about the caller's
        // own sessions, and a `userId` argument anywhere here would be one
        // account reaching another's.
        for forbidden in ["\n\tsessionsForUser(", "\n\tallSessions"] {
            assert!(
                !sdl.contains(forbidden),
                "`{forbidden}` would make this surface about somebody else's sessions"
            );
        }
    }
}
