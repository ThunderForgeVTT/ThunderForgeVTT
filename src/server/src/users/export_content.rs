//! What a person made, in the export's own words.
//!
//! Spec 039 T076, ADR-011 as amended 2026-09-10.
//!
//! # Why this exists now
//!
//! A disabled account has thirty days to download its data (FR-031, FR-032),
//! and a download missing that person's characters, lore and items is not the
//! remedy the spec promises. The export's `scenes` and `actors` had been empty
//! placeholders since May 2026.
//!
//! # Shapes, not rows
//!
//! ADR-011 rejected raw table dumps because they leak internal schema and
//! bypass the contract's versioning, and that still holds. Each kind of content
//! is exported as a shape named in the export's vocabulary — what somebody
//! would recognise as their work — and the manifest's version moves to `v2`
//! with it. Internal bookkeeping (permission flags, claim state, storage keys
//! that mean nothing outside this instance) is left out.
//!
//! # What "theirs" means
//!
//! A character is theirs if they **own** it, in any world — which is how a
//! player's character comes along even though a Game Master created the world.
//! Items, abilities, lore and collections are theirs if they created them.
//! Scenes are theirs if they own them.

use std::collections::HashMap;

use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::Serialize;
use uuid::Uuid;

use crate::models::{Collection, LoreEntry, Scene, WorldAbility, WorldActor, WorldItem};
use crate::schema::{
    scenes, world_abilities, world_actor_abilities, world_actor_images, world_actor_inventory,
    world_actor_system_data, world_actors, world_collection_members, world_collections,
    world_items, world_lore_entries,
};

/// A character, whole: its sheet and everything it carries.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ExportedActor {
    pub id: Uuid,
    pub world_id: Uuid,
    pub label: String,
    pub description: Option<String>,
    pub actor_type: String,
    pub game_system_id: Option<String>,
    pub is_npc: bool,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    /// The sheet, as the game system stores it — the part of a character that
    /// is most obviously somebody's work.
    pub system_data: Option<ExportedSystemData>,
    /// Ability names, as the character knows them.
    pub abilities: Vec<String>,
    pub inventory: Vec<ExportedInventoryLine>,
    /// Image roles and the stored objects behind them. The bytes are not in
    /// the export (ADR-011: the ZIP packages JSON only).
    pub images: Vec<ExportedImage>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ExportedSystemData {
    pub game_system_id: String,
    pub abilities: Option<serde_json::Value>,
    pub resources: Option<serde_json::Value>,
    pub proficiencies: Option<serde_json::Value>,
    pub traits: Option<serde_json::Value>,
    pub spells: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ExportedInventoryLine {
    pub name: String,
    pub quantity: i32,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ExportedImage {
    pub role: String,
    pub asset_id: Uuid,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ExportedItem {
    pub id: Uuid,
    pub world_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ExportedAbility {
    pub id: Uuid,
    pub world_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub classification: String,
    pub grade: Option<i32>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ExportedLoreEntry {
    pub id: Uuid,
    pub world_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub title: String,
    pub slug: String,
    /// The Markdown source, which is what the person wrote.
    pub content: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ExportedScene {
    pub id: Uuid,
    pub world_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub kind: String,
    pub width: i32,
    pub height: i32,
    pub grid_size: i32,
    pub grid_type: String,
    pub summary_markdown: Option<String>,
    pub hidden: bool,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ExportedCollection {
    pub id: Uuid,
    pub world_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub members: Vec<ExportedCollectionMember>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ExportedCollectionMember {
    pub member_type: String,
    pub member_id: Uuid,
}

/// Everything above, for one person.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct ExportedContent {
    pub actors: Vec<ExportedActor>,
    pub items: Vec<ExportedItem>,
    pub abilities: Vec<ExportedAbility>,
    pub lore_entries: Vec<ExportedLoreEntry>,
    pub scenes: Vec<ExportedScene>,
    pub collections: Vec<ExportedCollection>,
}

/// Group `(key, value)` pairs by key, keeping order.
fn grouped<V>(pairs: Vec<(Uuid, V)>) -> HashMap<Uuid, Vec<V>> {
    let mut out: HashMap<Uuid, Vec<V>> = HashMap::new();
    for (key, value) in pairs {
        out.entry(key).or_default().push(value);
    }
    out
}

/// Load everything `user_id` made. One query per kind of thing, and one per
/// part of a character, rather than one per character.
pub fn load_content_sync(conn: &mut PgConnection, user_id: Uuid) -> QueryResult<ExportedContent> {
    let actors: Vec<WorldActor> = world_actors::table
        .filter(world_actors::owned_by.eq(user_id))
        .order(world_actors::created_at.asc())
        .select(WorldActor::as_select())
        .load(conn)?;
    let actor_ids: Vec<Uuid> = actors.iter().map(|a| a.id).collect();

    type SystemRow = (
        Uuid,
        String,
        Option<serde_json::Value>,
        Option<serde_json::Value>,
        Option<serde_json::Value>,
        Option<serde_json::Value>,
        Option<serde_json::Value>,
    );
    let mut system_data: HashMap<Uuid, ExportedSystemData> = world_actor_system_data::table
        .filter(world_actor_system_data::actor_id.eq_any(&actor_ids))
        .select((
            world_actor_system_data::actor_id,
            world_actor_system_data::game_system_id,
            world_actor_system_data::ability_data,
            world_actor_system_data::resource_data,
            world_actor_system_data::proficiency_data,
            world_actor_system_data::trait_data,
            world_actor_system_data::spell_data,
        ))
        .load::<SystemRow>(conn)?
        .into_iter()
        .map(
            |(actor_id, game_system_id, abilities, resources, proficiencies, traits, spells)| {
                (
                    actor_id,
                    ExportedSystemData {
                        game_system_id,
                        abilities,
                        resources,
                        proficiencies,
                        traits,
                        spells,
                    },
                )
            },
        )
        .collect();

    let mut known = grouped(
        world_actor_abilities::table
            .filter(world_actor_abilities::actor_id.eq_any(&actor_ids))
            .order(world_actor_abilities::created_at.asc())
            .select((
                world_actor_abilities::actor_id,
                world_actor_abilities::ability_name_snapshot,
            ))
            .load::<(Uuid, String)>(conn)?,
    );
    let mut carried = grouped(
        world_actor_inventory::table
            .filter(world_actor_inventory::actor_id.eq_any(&actor_ids))
            .order(world_actor_inventory::created_at.asc())
            .select((
                world_actor_inventory::actor_id,
                world_actor_inventory::item_name_snapshot,
                world_actor_inventory::quantity,
            ))
            .load::<(Uuid, String, i32)>(conn)?
            .into_iter()
            .map(|(actor_id, name, quantity)| (actor_id, ExportedInventoryLine { name, quantity }))
            .collect(),
    );
    let mut pictured = grouped(
        world_actor_images::table
            .filter(world_actor_images::actor_id.eq_any(&actor_ids))
            .select((
                world_actor_images::actor_id,
                world_actor_images::role,
                world_actor_images::asset_id,
            ))
            .load::<(Uuid, String, Uuid)>(conn)?
            .into_iter()
            .map(|(actor_id, role, asset_id)| (actor_id, ExportedImage { role, asset_id }))
            .collect(),
    );

    let actors = actors
        .into_iter()
        .map(|actor| ExportedActor {
            system_data: system_data.remove(&actor.id),
            abilities: known.remove(&actor.id).unwrap_or_default(),
            inventory: carried.remove(&actor.id).unwrap_or_default(),
            images: pictured.remove(&actor.id).unwrap_or_default(),
            id: actor.id,
            world_id: actor.world_id,
            label: actor.label,
            description: actor.description,
            actor_type: actor.actor_type,
            game_system_id: actor.game_system_id,
            is_npc: actor.is_npc,
            created_at: actor.created_at,
            updated_at: actor.updated_at,
        })
        .collect();

    let items = world_items::table
        .filter(world_items::created_by.eq(user_id))
        .order(world_items::created_at.asc())
        .select(WorldItem::as_select())
        .load::<WorldItem>(conn)?
        .into_iter()
        .map(|item| ExportedItem {
            id: item.id,
            world_id: item.world_id,
            name: item.name,
            description: item.description,
            created_at: item.created_at,
            updated_at: item.updated_at,
        })
        .collect();

    let abilities = world_abilities::table
        .filter(world_abilities::created_by.eq(user_id))
        .order(world_abilities::created_at.asc())
        .select(WorldAbility::as_select())
        .load::<WorldAbility>(conn)?
        .into_iter()
        .map(|ability| ExportedAbility {
            id: ability.id,
            world_id: ability.world_id,
            name: ability.name,
            description: ability.description,
            classification: ability.classification,
            grade: ability.grade,
            created_at: ability.created_at,
            updated_at: ability.updated_at,
        })
        .collect();

    let lore_entries = world_lore_entries::table
        .filter(world_lore_entries::created_by.eq(user_id))
        .order(world_lore_entries::created_at.asc())
        .select(LoreEntry::as_select())
        .load::<LoreEntry>(conn)?
        .into_iter()
        .map(|entry| ExportedLoreEntry {
            id: entry.id,
            world_id: entry.world_id,
            parent_id: entry.parent_id,
            title: entry.title,
            slug: entry.slug,
            content: entry.content,
            created_at: entry.created_at,
            updated_at: entry.updated_at,
        })
        .collect();

    let scenes = scenes::table
        .filter(scenes::owner_id.eq(user_id))
        .order(scenes::created_at.asc())
        .select(Scene::as_select())
        .load::<Scene>(conn)?
        .into_iter()
        .map(|scene| ExportedScene {
            id: scene.scene_id,
            world_id: scene.world_id,
            name: scene.name,
            description: scene.description,
            kind: scene.type_,
            width: scene.width,
            height: scene.height,
            grid_size: scene.grid_size,
            grid_type: scene.grid_type,
            summary_markdown: scene.summary_markdown,
            hidden: scene.hidden,
            created_at: scene.created_at,
            updated_at: scene.updated_at,
        })
        .collect();

    let collections: Vec<Collection> = world_collections::table
        .filter(world_collections::created_by.eq(user_id))
        .order(world_collections::created_at.asc())
        .select(Collection::as_select())
        .load(conn)?;
    let collection_ids: Vec<Uuid> = collections.iter().map(|c| c.id).collect();
    let mut members = grouped(
        world_collection_members::table
            .filter(world_collection_members::collection_id.eq_any(&collection_ids))
            .order(world_collection_members::sort_order.asc())
            .select((
                world_collection_members::collection_id,
                world_collection_members::member_type,
                world_collection_members::member_id,
            ))
            .load::<(Uuid, String, Uuid)>(conn)?
            .into_iter()
            .map(|(collection_id, member_type, member_id)| {
                (
                    collection_id,
                    ExportedCollectionMember {
                        member_type,
                        member_id,
                    },
                )
            })
            .collect(),
    );
    let collections = collections
        .into_iter()
        .map(|collection| ExportedCollection {
            members: members.remove(&collection.id).unwrap_or_default(),
            id: collection.id,
            world_id: collection.world_id,
            name: collection.name,
            description: collection.description,
            created_at: collection.created_at,
            updated_at: collection.updated_at,
        })
        .collect();

    Ok(ExportedContent {
        actors,
        items,
        abilities,
        lore_entries,
        scenes,
        collections,
    })
}
