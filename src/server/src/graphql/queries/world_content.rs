//! `worldContentInventory(worldId, targetSystemId)` — what a system change
//! puts at stake, in numbers.
//!
//! Spec 033 FR-025 and FR-037, and ADR-065. The warning a Game Master reads
//! before changing a world's system has to name real counts, because a generic
//! warning gets clicked through and a *false* one teaches people to distrust
//! every warning the product shows them.
//!
//! Nothing here is stored. The counts are computed when a confirmation opens
//! and acknowledged by digest; see `contracts/system-change-guard.md`.

use async_graphql::{
    Context, Error, ErrorExtensions, Object, Result as GraphQLResult, SimpleObject,
};
use diesel::dsl::count_star;
use diesel::prelude::*;
use uuid::Uuid;

use crate::ability_vocabulary::for_system;
use crate::auth::world_membership::is_dm_of_world;
use crate::graphql::{app_state, authenticated_user};
use crate::schema::{world_abilities, world_actors, world_items, worlds};

/// One kind of content, and the system it was authored under.
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLContentCount {
    /// `actors`, `abilities`, `items`.
    pub kind: String,
    /// The system this content was authored for, where the content records
    /// one. `null` for content that carries no system tag of its own — items
    /// and abilities belong to the world rather than to a ruleset, and saying
    /// otherwise in a warning would be inventing provenance.
    pub system_id: Option<String>,
    pub count: i32,
}

/// What a system change puts at stake in one world.
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLContentInventory {
    pub counts: Vec<GraphQLContentCount>,
    /// Abilities whose type the **target** system would not recognise
    /// (FR-037). `0` when no target was named — that means "not asked", not
    /// "none".
    pub becoming_unrecognised: i32,
    /// Whether this world is switchable without ceremony (FR-029).
    ///
    /// Actors, abilities and items only. **Scenes and lore do not count**, and
    /// that is load-bearing rather than an oversight: every world is created
    /// with a default scene already made (spec 010), so counting scenes would
    /// mean no world is ever empty, the one-step path would be unreachable,
    /// and a Game Master would meet the red warning on a world they made a
    /// minute earlier.
    pub is_empty: bool,
    /// What an acknowledgement acknowledges. See `digest_of`.
    pub digest: String,
}

/// A stable digest over the counts.
///
/// The mutation takes this back, and the server recomputes and compares — so
/// "I acknowledge" means "I acknowledge **these** numbers" rather than "I
/// clicked something". A world that gained an actor while the dialog was open
/// is re-confirmed rather than switched behind the Game Master's back.
///
/// Sorted before hashing, because the digest must not depend on the order the
/// database happened to return rows in.
pub fn digest_of(counts: &[GraphQLContentCount], becoming_unrecognised: i32) -> String {
    let mut parts: Vec<String> = counts
        .iter()
        .map(|entry| {
            format!(
                "{}:{}:{}",
                entry.kind,
                entry.system_id.as_deref().unwrap_or("-"),
                entry.count
            )
        })
        .collect();
    parts.sort();
    parts.push(format!("unrecognised:{becoming_unrecognised}"));

    // Not a cryptographic claim: this detects a *changed* count, not a forged
    // one. Forgery is already covered — the caller must be the world's DM, and
    // the server recomputes the counts it compares against.
    let joined = parts.join("|");
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in joined.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// Count one world's authored content, and what a move to `target_system_id`
/// would leave unrecognised.
///
/// Split from the resolver so the mutation can call it too: the guard
/// recomputes rather than trusting what it was handed, which is the whole
/// point of the digest.
pub fn inventory_of(
    conn: &mut PgConnection,
    systems_dir: &str,
    world_id: Uuid,
    target_system_id: Option<&str>,
) -> Result<GraphQLContentInventory, diesel::result::Error> {
    // Actors carry their own `game_system_id`, so they are counted per system
    // — that is what lets the warning say "12 actors authored for genie"
    // rather than "12 actors".
    let actors: Vec<(Option<String>, i64)> = world_actors::table
        .filter(world_actors::world_id.eq(world_id))
        .group_by(world_actors::game_system_id)
        .select((world_actors::game_system_id, count_star()))
        .load(conn)?;

    let abilities: i64 = world_abilities::table
        .filter(world_abilities::world_id.eq(world_id))
        .count()
        .get_result(conn)?;

    let items: i64 = world_items::table
        .filter(world_items::world_id.eq(world_id))
        .count()
        .get_result(conn)?;

    let mut counts: Vec<GraphQLContentCount> = actors
        .into_iter()
        .map(|(system_id, count)| GraphQLContentCount {
            kind: "actors".to_string(),
            system_id,
            count: count as i32,
        })
        .collect();

    if abilities > 0 {
        counts.push(GraphQLContentCount {
            kind: "abilities".to_string(),
            system_id: None,
            count: abilities as i32,
        });
    }
    if items > 0 {
        counts.push(GraphQLContentCount {
            kind: "items".to_string(),
            system_id: None,
            count: items as i32,
        });
    }

    // FR-037: how many abilities lose their tab under the target system. Needs
    // the target's vocabulary, which is why the query takes a target at all.
    let becoming_unrecognised = match target_system_id {
        None => 0,
        Some(target) => {
            let held: Vec<String> = world_abilities::table
                .filter(world_abilities::world_id.eq(world_id))
                .select(world_abilities::classification)
                .load::<String>(conn)?;

            let vocabulary = for_system(systems_dir, Some(target), &held);
            held.iter()
                .filter(|classification| !vocabulary.recognises(classification))
                .count() as i32
        }
    };

    let actor_total: i64 = counts
        .iter()
        .filter(|entry| entry.kind == "actors")
        .map(|entry| i64::from(entry.count))
        .sum();

    let is_empty = actor_total == 0 && abilities == 0 && items == 0;
    let digest = digest_of(&counts, becoming_unrecognised);

    Ok(GraphQLContentInventory {
        counts,
        becoming_unrecognised,
        is_empty,
        digest,
    })
}

#[derive(Default)]
pub struct WorldContentQuery;

#[Object]
impl WorldContentQuery {
    /// What changing this world's system would affect.
    ///
    /// **DM-only.** The counts describe content a player may not be able to
    /// see, and a total is a disclosure even when the rows are not.
    async fn world_content_inventory(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
        target_system_id: Option<String>,
    ) -> GraphQLResult<GraphQLContentInventory> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;

        if !is_dm_of_world(state, auth_user.user_id, auth_user.is_admin, world_id).await? {
            return Err(Error::new(
                "Only Owners and GMs can see what a system change would affect",
            )
            .extend_with(|_, ext| ext.set("code", "FORBIDDEN")));
        }

        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;
        let systems_dir = state.directories.systems_dir.clone();

        tokio::task::spawn_blocking(move || {
            inventory_of(
                &mut conn,
                &systems_dir,
                world_id,
                target_system_id.as_deref(),
            )
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to count this world's content"))
    }
}
