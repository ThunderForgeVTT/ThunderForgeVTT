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

use crate::auth::authoring_tools::effective_authoring_tools;
use crate::auth::world_membership::is_dm_of_world;
use crate::graphql::mutations_authoring_tools::member_tools;
use crate::graphql::{app_state, authenticated_user};
use crate::schema::world_members;
use thunderforge_authz::Role;

/// What one player of a world may use, as the settings page needs it: keyed
/// by membership, one entry per member who does not run the world.
///
/// Game Masters are left out. They hold every tool implicitly, and rendering
/// that as six lit toggles would invite somebody to turn one off and find
/// that nothing happened.
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
    /// A player holds Select and Shapes until a Game Master takes them away
    /// (spec 082).
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
    /// Spec 082: one entry per member who does not run the world, carrying
    /// the tools that member may use, so a member with no rows shows the
    /// defaults rather than reading as "nothing".
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

        tokio::task::spawn_blocking(move || players_tools_on(&mut conn, world_id))
            .await
            .map_err(|_| Error::new("Failed to spawn blocking task"))?
            .map_err(|_| Error::new("Failed to load authoring tool grants"))
    }
}

/// Every member of the world who does not run it, in the order they joined,
/// with the tools each may use. A member with no rows reads as Select and
/// Shapes (spec 082).
pub fn players_tools_on(
    conn: &mut PgConnection,
    world_id: Uuid,
) -> QueryResult<Vec<GraphQLMemberAuthoringTools>> {
    let members = world_members::table
        .filter(world_members::world_id.eq(world_id))
        .order(world_members::joined_at.asc())
        .select((
            world_members::id,
            world_members::user_id,
            world_members::role,
        ))
        .load::<(Uuid, Uuid, String)>(conn)?;
    members
        .into_iter()
        .filter(|(_, _, role)| !Role::from_stored(role).is_some_and(Role::runs_the_world))
        .map(|(world_member_id, user_id, _)| {
            Ok(GraphQLMemberAuthoringTools {
                world_member_id,
                user_id,
                tools: member_tools(conn, world_member_id)?,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{
        insert_test_user, insert_test_world, insert_test_world_member, test_app_state,
    };

    /// Spec 082: the settings page lists every player, and one nobody has
    /// touched shows Select and Shapes rather than nothing. Game Masters are
    /// left out.
    #[test]
    fn every_player_is_listed_with_the_tools_they_hold() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        insert_test_world_member(&mut conn, world_id, owner_id, "Owner");
        let gm_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, gm_id, "GM");
        let player_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");

        let listed = players_tools_on(&mut conn, world_id).expect("listing");
        assert_eq!(listed.len(), 1, "only the player is configurable");
        assert_eq!(listed[0].user_id, player_id);
        assert_eq!(listed[0].tools, vec!["select", "shapes"]);
    }
}
