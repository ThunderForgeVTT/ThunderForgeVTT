//! A player's character outlives the world it was played in.
//!
//! Decided 2026-09-08, and applied by spec 039 to both ways an account ends —
//! a person deleting their own, and a termination window closing: **the
//! account's worlds are deleted, including ones other people play in, but
//! every character owned by somebody else is moved to that somebody first.**
//! A character is its player's work, not the world owner's.
//!
//! # Where it goes
//!
//! Into a world the player owns **on the same game system** — a character sheet
//! written for one system is not a character in another — or, if they have
//! none, a new "<name>'s characters" world on that system. Inside it, the
//! rescued characters (and their own abilities and items, so they arrive
//! whole) are filed as a **collection named for the world they came from**.
//!
//! The collection is not decoration. Content is expected to move out onto a
//! person's profile one day, organised by collection; a rescued character is
//! then already filed where that move will look for it.

use std::collections::BTreeMap;

use diesel::prelude::*;
use serde_json::json;
use uuid::Uuid;

use crate::models::World;
use crate::notices;
use crate::schema::{
    scenes, users, world_actors, world_collection_members, world_collections, worlds,
};

/// What the world a character came from was.
struct Source {
    name: String,
    game_system_id: Option<String>,
    interface_pack_id: Option<String>,
}

/// Move every character in `doomed_worlds` that belongs to somebody other than
/// `departing` to its player, before those worlds are deleted. Returns how many
/// characters were moved.
///
/// On the caller's connection and inside its transaction: a deletion whose
/// rescue failed rolls back whole, rather than deleting a world whose
/// characters were never moved.
pub fn rescue_characters_sync(
    conn: &mut PgConnection,
    departing: Uuid,
    doomed_worlds: &[Uuid],
) -> QueryResult<usize> {
    if doomed_worlds.is_empty() {
        return Ok(0);
    }

    let others: Vec<(Uuid, Uuid, Uuid)> = world_actors::table
        .filter(world_actors::world_id.eq_any(doomed_worlds))
        .filter(world_actors::owned_by.ne(departing))
        .order(world_actors::created_at.asc())
        .select((
            world_actors::world_id,
            world_actors::owned_by,
            world_actors::id,
        ))
        .load(conn)?;

    let mut by_world_and_player: BTreeMap<(Uuid, Uuid), Vec<Uuid>> = BTreeMap::new();
    for (world_id, player, actor_id) in others {
        by_world_and_player
            .entry((world_id, player))
            .or_default()
            .push(actor_id);
    }

    let mut moved = 0;
    for ((world_id, player), actors) in by_world_and_player {
        let source = source_of(conn, world_id)?;
        let destination = personal_world_sync(conn, player, &source)?;
        let ctx = crate::collections::copy::rescue_actors_sync(conn, destination, player, &actors)
            .map_err(|e| diesel::result::Error::QueryBuilderError(e.0.into()))?;

        let collection_id = file_as_collection(conn, destination, player, &source.name, &ctx)?;
        let characters: Vec<&str> = ctx
            .created
            .iter()
            .filter(|record| record.member_type == "actor")
            .map(|record| record.name.as_str())
            .collect();
        notices::record_sync(
            conn,
            player,
            notices::kind::ACTOR_RESCUED,
            Some(json!({ "worldId": destination, "collectionId": collection_id })),
            Some(json!({
                "sourceWorldName": source.name,
                "destinationWorldId": destination,
                "collectionId": collection_id,
                "characters": characters,
            })),
        )?;
        moved += actors.len();
    }
    Ok(moved)
}

fn source_of(conn: &mut PgConnection, world_id: Uuid) -> QueryResult<Source> {
    let (name, game_system_id, interface_pack_id) = worlds::table
        .filter(worlds::id.eq(world_id))
        .select((
            worlds::name,
            worlds::game_system_id,
            worlds::interface_pack_id,
        ))
        .first::<(String, Option<String>, Option<String>)>(conn)?;
    Ok(Source {
        name,
        game_system_id,
        interface_pack_id,
    })
}

/// A world `player` owns on the source's game system and that has a scene to
/// put a character in — the oldest, so the answer is repeatable — or a new one.
fn personal_world_sync(
    conn: &mut PgConnection,
    player: Uuid,
    source: &Source,
) -> QueryResult<Uuid> {
    let existing = worlds::table
        .filter(worlds::created_by.eq(player))
        .filter(worlds::game_system_id.is_not_distinct_from(source.game_system_id.clone()))
        .filter(diesel::dsl::exists(
            scenes::table.filter(scenes::world_id.eq(worlds::id)),
        ))
        .order(worlds::created_at.asc())
        .select(worlds::id)
        .first::<Uuid>(conn)
        .optional()?;
    if let Some(world_id) = existing {
        return Ok(world_id);
    }

    let username: String = users::table
        .filter(users::id.eq(player))
        .select(users::username)
        .first(conn)?;
    let now = chrono::Utc::now().naive_utc();
    let world = World {
        id: Uuid::now_v7(),
        name: format!("{username}'s characters"),
        description: Some(
            "Characters moved here when the worlds they were played in were deleted.".to_string(),
        ),
        game_system_id: source.game_system_id.clone(),
        interface_pack_id: source.interface_pack_id.clone(),
        created_by: player,
        updated_by: player,
        created_at: now,
        updated_at: now,
        session_notes: None,
        allow_player_created_actors: false,
        genie_resource_carryover_enabled: false,
        default_scene_grid_type: "square".to_string(),
        active_scene_id: None,
    };
    crate::graphql::mutations_worlds::insert_world_sync(conn, &world, Uuid::now_v7(), now)?;
    Ok(world.id)
}

/// File everything rescued as one collection, named for the world it came from.
fn file_as_collection(
    conn: &mut PgConnection,
    destination: Uuid,
    player: Uuid,
    source_name: &str,
    ctx: &crate::collections::copy::CopyContext,
) -> QueryResult<Uuid> {
    let collection_id = Uuid::now_v7();
    let now = chrono::Utc::now().naive_utc();
    diesel::insert_into(world_collections::table)
        .values((
            world_collections::id.eq(collection_id),
            world_collections::world_id.eq(destination),
            world_collections::name.eq(source_name.chars().take(200).collect::<String>()),
            world_collections::description.eq(Some(
                "Rescued when the world of this name was deleted with its creator's account.",
            )),
            world_collections::created_by.eq(player),
            world_collections::updated_by.eq(player),
            world_collections::created_at.eq(now),
            world_collections::updated_at.eq(now),
        ))
        .execute(conn)?;

    for (sort_order, record) in ctx.created.iter().enumerate() {
        diesel::insert_into(world_collection_members::table)
            .values((
                world_collection_members::id.eq(Uuid::now_v7()),
                world_collection_members::collection_id.eq(collection_id),
                world_collection_members::member_type.eq(&record.member_type),
                world_collection_members::member_id.eq(record.id),
                world_collection_members::sort_order.eq(sort_order as i32),
                world_collection_members::added_by.eq(player),
                world_collection_members::created_at.eq(now),
            ))
            .execute(conn)?;
    }
    Ok(collection_id)
}
