//! `worldStatistics(worldId)` — a world by its figures, counted here.
//!
//! # Why a query rather than arithmetic in the browser
//!
//! The world dashboard used to fetch every scene and draw the lot. The
//! owner's objection is what a five-year campaign looks like: "imagine 40
//! scenes we've been running for five years. Showing all the scenes would be
//! chaotic." The same page would need every actor and every token to say how
//! many there are, and a dashboard that downloads two thousand tokens to
//! render the number `2000` gets slower every session it is used.
//!
//! So the counting happens where the rows are. One round trip, seven
//! `COUNT(*)`s behind indexed `world_id`/`scene_id` columns, and a response
//! whose size does not depend on the size of the world.
//!
//! # What a player is allowed to be told
//!
//! Every figure here is counted **for the person asking**, which is not the
//! same as counting everything and hiding some of it:
//!
//! - **Scenes** are the scenes that viewer may open. A hidden scene is not in
//!   a player's total, because a total that included it would announce that
//!   there are places they have not been shown.
//! - **Tokens** are the tokens standing on those scenes, for the same reason.
//! - **NPCs** are `null` for anyone who does not run the world. Not zero, not
//!   "the ones you can see": absent. Hidden NPCs (`auth::npc_visibility`,
//!   2026-09-15) must never be countable into a figure a player can read, and
//!   the honest way to satisfy that is to show a player fewer figures rather
//!   than a figure they cannot trust. A count of *visible* NPCs would also
//!   leak: a Game Master revealing one goblin would move a number a player is
//!   watching.
//! - **Characters**, **members** and **the active encounter** are the same
//!   for everyone, because they already are: a player character is seen by
//!   every member of its world, the roster is `worldMembers`, and the combat
//!   tracker lists the encounter to the table it is happening to.
//!
//! # The owner who has no membership row
//!
//! `create_world` writes no `world_members` row for its creator (see
//! `require_world_member`), so `COUNT(*)` over that table can be one short of
//! the people actually in the world. The member figure is therefore a set
//! union of the rows and the creator, which is also what the roster shows.

use std::collections::HashSet;

use async_graphql::{Context, Error, Object, Result as GraphQLResult, SimpleObject};
use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::world_membership::actor_in_world;
use crate::graphql::{app_state, authenticated_user};
use crate::schema::{
    scenes, tokens, world_actor_claims, world_actors, world_combatants, world_combats,
    world_members, worlds,
};

/// The encounter running right now, if one is.
///
/// At most one per world: `world_combats` carries a partial unique index on
/// `ended_at IS NULL` (see `mutations_combat::find_active_combat`), so this
/// is a `first`, not a `first` over an ordering that might tie.
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "ActiveEncounterSummary")]
pub struct GraphQLActiveEncounterSummary {
    pub id: Uuid,
    /// Which round the encounter is on.
    pub round: i32,
    /// How many combatants are in the initiative order.
    pub combatants: i32,
}

/// A world's figures, for the person who asked.
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "WorldStatistics")]
pub struct GraphQLWorldStatistics {
    /// Scenes this viewer may open. Hidden scenes are excluded for a player.
    pub scenes: i32,
    /// People in the world, the creator included whether or not a
    /// `world_members` row was ever written for them.
    pub members: i32,
    /// How many of those hold a character.
    pub members_with_character: i32,
    /// Player characters — actors that are not NPCs.
    pub characters: i32,
    /// NPCs. `null` for anyone who does not run the world; see the module
    /// doc for why this is absent rather than partial.
    pub npcs: Option<i32>,
    /// Tokens standing on the scenes this viewer may open.
    pub tokens: i32,
    /// The encounter in progress, or `null`.
    pub active_encounter: Option<GraphQLActiveEncounterSummary>,
}

#[derive(Default)]
pub struct WorldStatisticsQuery;

/// Narrowing for the GraphQL `Int`, which is 32-bit. A world with more than
/// two billion tokens has a different problem than a wrong dashboard.
fn as_i32(value: i64) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

/// The whole computation, synchronous and testable without a schema.
pub fn world_statistics_sync(
    conn: &mut PgConnection,
    user_id: Uuid,
    is_admin: bool,
    world_id: Uuid,
) -> GraphQLResult<GraphQLWorldStatistics> {
    let actor = actor_in_world(conn, user_id, is_admin, world_id);
    if actor.role.is_none() && !actor.is_site_admin {
        return Err(Error::new("You must be a member of this world"));
    }
    let runs_the_world = actor.runs_the_world();

    let read = |_: diesel::result::Error| Error::new("Failed to count this world");

    // Scenes first, because the token count is "tokens on these scenes" and
    // a player's scenes are only the ones they may open.
    let mut scene_query = scenes::table
        .filter(scenes::world_id.eq(world_id))
        .into_boxed();
    if !runs_the_world {
        scene_query = scene_query.filter(scenes::hidden.eq(false));
    }
    let scene_ids: Vec<Uuid> = scene_query
        .select(scenes::scene_id)
        .load(conn)
        .map_err(read)?;

    let token_count: i64 = tokens::table
        .filter(tokens::scene_id.eq_any(&scene_ids))
        .count()
        .get_result(conn)
        .map_err(read)?;

    let member_ids: Vec<Uuid> = world_members::table
        .filter(world_members::world_id.eq(world_id))
        .select(world_members::user_id)
        .load(conn)
        .map_err(read)?;
    let creator: Uuid = worlds::table
        .find(world_id)
        .select(worlds::created_by)
        .first(conn)
        .map_err(read)?;
    let mut people: HashSet<Uuid> = member_ids.into_iter().collect();
    people.insert(creator);

    let members_with_character: i64 = world_actor_claims::table
        .inner_join(
            world_members::table.on(world_actor_claims::world_member_id.eq(world_members::id)),
        )
        .filter(world_members::world_id.eq(world_id))
        .select(diesel::dsl::count(world_members::user_id).aggregate_distinct())
        .first(conn)
        .map_err(read)?;

    let characters: i64 = world_actors::table
        .filter(world_actors::world_id.eq(world_id))
        .filter(world_actors::is_npc.eq(false))
        .count()
        .get_result(conn)
        .map_err(read)?;

    let npcs: Option<i32> = if runs_the_world {
        let count: i64 = world_actors::table
            .filter(world_actors::world_id.eq(world_id))
            .filter(world_actors::is_npc.eq(true))
            .count()
            .get_result(conn)
            .map_err(read)?;
        Some(as_i32(count))
    } else {
        None
    };

    let running: Option<(Uuid, i32)> = world_combats::table
        .filter(world_combats::world_id.eq(world_id))
        .filter(world_combats::ended_at.is_null())
        .select((world_combats::id, world_combats::round))
        .first(conn)
        .optional()
        .map_err(read)?;

    let active_encounter = match running {
        Some((id, round)) => {
            let combatants: i64 = world_combatants::table
                .filter(world_combatants::combat_id.eq(id))
                .count()
                .get_result(conn)
                .map_err(read)?;
            Some(GraphQLActiveEncounterSummary {
                id,
                round,
                combatants: as_i32(combatants),
            })
        }
        None => None,
    };

    Ok(GraphQLWorldStatistics {
        scenes: as_i32(scene_ids.len() as i64),
        members: as_i32(people.len() as i64),
        members_with_character: as_i32(members_with_character),
        characters: as_i32(characters),
        npcs,
        tokens: as_i32(token_count),
        active_encounter,
    })
}

#[Object]
impl WorldStatisticsQuery {
    /// A world's figures for the person asking: how many scenes they may
    /// open, how many people are at the table and how many of them hold a
    /// character, how many characters and (for a Game Master) how many NPCs,
    /// how many tokens stand on those scenes, and whether a fight is running.
    ///
    /// Guarded by membership, like its neighbours. A site admin who is not a
    /// member is answered, because the dashboard already tells them they are
    /// looking through administrator access.
    async fn world_statistics(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
    ) -> GraphQLResult<GraphQLWorldStatistics> {
        let caller = authenticated_user(ctx)?;
        let (user_id, is_admin) = (caller.user_id, caller.is_admin);
        let mut conn = app_state(ctx)?
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        tokio::task::spawn_blocking(move || {
            world_statistics_sync(&mut conn, user_id, is_admin, world_id)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
    }
}

#[cfg(test)]
#[path = "world_statistics_tests.rs"]
mod world_statistics_tests;
