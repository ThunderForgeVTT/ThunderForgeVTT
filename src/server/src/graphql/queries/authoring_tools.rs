//! `authoringTools(worldId)` — which tools the caller may use in one world.
//!
//! Spec 031 FR-044/FR-047. The rail asks this rather than deciding from the
//! caller's role, so "which tools do I have" has one answer and the server
//! gives it. Chrome hiding a button is presentation; the refusal that matters
//! happens here and in the engine.

use async_graphql::{Context, Object, Result as GraphQLResult};
use uuid::Uuid;

use crate::auth::authoring_tools::effective_authoring_tools;
use crate::graphql::{app_state, authenticated_user};

#[derive(Default)]
pub struct AuthoringToolsQuery;

#[Object]
impl AuthoringToolsQuery {
    /// The tool ids the caller may author with in this world.
    ///
    /// About the caller and nobody else. A Game Master configuring another
    /// member's tools needs a different question — one that names a subject
    /// and is DM-gated — and answering both from one field would make it easy
    /// to ship the second without the gate.
    ///
    /// An empty list is the honest answer for a player in a world whose Game
    /// Master has granted nothing, which today is every world (FR-045).
    async fn authoring_tools(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
    ) -> GraphQLResult<Vec<String>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        effective_authoring_tools(state, auth_user.user_id, auth_user.is_admin, world_id).await
    }
}
