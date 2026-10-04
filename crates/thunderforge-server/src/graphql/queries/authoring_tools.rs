//! `authoringTools(worldId)` — which tools the caller may use in one world.
//!
//! Spec 031 FR-044/FR-047. The rail asks this rather than deciding from the
//! caller's role, so "which tools do I have" has one answer and the server
//! gives it. Chrome hiding a button is presentation; the refusal that matters
//! happens here and in the engine.

use async_graphql::{
    Context, Error, ErrorExtensions, Object, Result as GraphQLResult, SimpleObject,
};
use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::authoring_tools::{AUTHORING_TOOLS, effective_authoring_tools};
use crate::auth::world_membership::is_dm_of_world;
use crate::graphql::{app_state, authenticated_user};
use crate::schema::{world_authoring_tool_grants, world_members};

/// What one member of a world has been *granted*, as the settings page needs
/// it: keyed by membership, one entry per member holding anything.
///
/// Deliberately not "what this member may use". A Game Master holds every
/// tool implicitly, and rendering that as six lit toggles would invite
/// somebody to turn one off and find that nothing happened.
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLMemberAuthoringTools {
    pub world_member_id: Uuid,
    pub user_id: Uuid,
    pub tools: Vec<String>,
}

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

    /// Every grant handed out in this world, for the Game Master configuring
    /// them (FR-046).
    ///
    /// DM-gated, which is the reason it is a second field rather than an
    /// argument on the first: a query that answers both "what may I use" and
    /// "what may they use" would have one of its two answers guarded and the
    /// other not, and the guard is easy to leave off the day the argument is
    /// added.
    ///
    /// Members with no grants are simply absent. The settings page has the
    /// roster already and reads an absent member as "nothing", which is the
    /// same default the resolver applies.
    async fn authoring_tool_grants(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
    ) -> GraphQLResult<Vec<GraphQLMemberAuthoringTools>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;

        if !is_dm_of_world(state, auth_user.user_id, auth_user.is_admin, world_id).await? {
            return Err(Error::new(
                "Only Owners and GMs can see this world's authoring tool grants",
            )
            .extend_with(|_, ext| ext.set("code", "FORBIDDEN")));
        }

        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        let rows = tokio::task::spawn_blocking(move || {
            world_authoring_tool_grants::table
                .inner_join(world_members::table)
                .filter(world_members::world_id.eq(world_id))
                .select((
                    world_members::id,
                    world_members::user_id,
                    world_authoring_tool_grants::tool,
                ))
                .load::<(Uuid, Uuid, String)>(&mut conn)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to load authoring tool grants"))?;

        // Grouped here rather than by SQL aggregation so the tools come back
        // in declaration order — the order the rail draws them in, so the
        // toggles read the same way in both places — and so a row naming a
        // tool this build does not have is dropped rather than shown as a
        // switch that controls nothing.
        let mut grouped: Vec<GraphQLMemberAuthoringTools> = Vec::new();
        for (world_member_id, user_id, _) in rows.iter().cloned() {
            if grouped.iter().any(|g| g.world_member_id == world_member_id) {
                continue;
            }
            grouped.push(GraphQLMemberAuthoringTools {
                world_member_id,
                user_id,
                tools: AUTHORING_TOOLS
                    .iter()
                    .filter(|tool| {
                        rows.iter()
                            .any(|(m, _, held)| *m == world_member_id && held == *tool)
                    })
                    .map(|tool| (*tool).to_string())
                    .collect(),
            });
        }

        Ok(grouped)
    }
}
