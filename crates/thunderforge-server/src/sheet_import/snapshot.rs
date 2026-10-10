//! An actor as an import finds it: what the plan diffs against, and the
//! before-snapshot a rollback restores (FR-040).

use std::collections::BTreeMap;

use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use thunderforge_sheet_import::{ActorSnapshot, ContentTarget, CurrentLink, SheetMapping};
use uuid::Uuid;

use crate::models::WorldActor;

/// The data types an actor's system data has, one column each.
pub const DATA_TYPES: [&str; 5] = [
    "ability_data",
    "resource_data",
    "proficiency_data",
    "trait_data",
    "spell_data",
];

/// One ability the actor links: a world ability, or staged content.
#[derive(Queryable, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AbilityLink {
    pub id: Uuid,
    pub ability_id: Option<Uuid>,
    pub staged_id: Option<Uuid>,
    pub name: String,
    pub prepared: Option<bool>,
    pub granted_by: Option<String>,
    pub uses_max: Option<i16>,
    pub uses_used: Option<i16>,
    pub recharge: Option<String>,
    /// The world ability's vocabulary type, or the staged piece's kind.
    pub vocabulary: Option<String>,
}

/// One item the actor carries.
#[derive(Queryable, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InventoryLink {
    pub id: Uuid,
    pub item_id: Option<Uuid>,
    pub staged_id: Option<Uuid>,
    pub name: String,
    pub quantity: i32,
    pub equipped: bool,
    pub attuned: bool,
}

/// Everything an import writes, as it is now.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActorState {
    pub label: String,
    /// Data type to its object; a type the actor has no data for is absent.
    pub system_data: BTreeMap<String, Value>,
    pub abilities: Vec<AbilityLink>,
    pub inventory: Vec<InventoryLink>,
}

/// The actor, its state, and whether a sheet was imported onto it before.
pub struct ActorContext {
    pub actor: WorldActor,
    pub state: ActorState,
    pub is_reimport: bool,
}

pub fn load(conn: &mut PgConnection, actor_id: Uuid) -> QueryResult<ActorContext> {
    use crate::schema::{
        actor_imports, world_abilities, world_actor_abilities as links,
        world_actor_inventory as inventory, world_actor_system_data as data, world_actors,
        world_staged_content as staged,
    };

    let actor = world_actors::table
        .find(actor_id)
        .select(WorldActor::as_select())
        .first(conn)?;

    let columns: Option<(
        Option<Value>,
        Option<Value>,
        Option<Value>,
        Option<Value>,
        Option<Value>,
    )> = data::table
        .filter(data::actor_id.eq(actor_id))
        .select((
            data::ability_data,
            data::resource_data,
            data::proficiency_data,
            data::trait_data,
            data::spell_data,
        ))
        .first(conn)
        .optional()?;
    let mut system_data = BTreeMap::new();
    if let Some((a, r, p, t, s)) = columns {
        for (name, value) in DATA_TYPES.iter().zip([a, r, p, t, s]) {
            if let Some(value) = value {
                system_data.insert(name.to_string(), value);
            }
        }
    }

    let abilities = links::table
        .left_join(world_abilities::table)
        .left_join(staged::table)
        .filter(links::actor_id.eq(actor_id))
        .order((links::created_at.asc(), links::id.asc()))
        .select((
            links::id,
            links::ability_id,
            links::staged_id,
            links::ability_name_snapshot,
            links::prepared,
            links::granted_by,
            links::uses_max,
            links::uses_used,
            links::recharge,
            diesel::dsl::sql::<diesel::sql_types::Nullable<diesel::sql_types::Text>>(
                "COALESCE(world_abilities.classification, world_staged_content.kind)",
            ),
        ))
        .load::<AbilityLink>(conn)?;

    let inventory = inventory::table
        .filter(inventory::actor_id.eq(actor_id))
        .order((inventory::created_at.asc(), inventory::id.asc()))
        .select((
            inventory::id,
            inventory::item_id,
            inventory::staged_id,
            inventory::item_name_snapshot,
            inventory::quantity,
            inventory::equipped,
            inventory::attuned,
        ))
        .load::<InventoryLink>(conn)?;

    let is_reimport = diesel::select(diesel::dsl::exists(
        actor_imports::table
            .filter(actor_imports::actor_id.eq(actor_id))
            .filter(actor_imports::kind.eq(super::ActorImportKind::Import)),
    ))
    .get_result::<bool>(conn)?;

    Ok(ActorContext {
        state: ActorState {
            label: actor.label.clone(),
            system_data,
            abilities,
            inventory,
        },
        actor,
        is_reimport,
    })
}

/// The neutral kind a stored link is, by the declaration: the kind whose
/// target is this vocabulary type, or the item kind.
fn neutral_kind(mapping: &SheetMapping, item: bool, vocabulary: Option<&str>) -> Option<String> {
    mapping
        .content
        .iter()
        .find(|(_, target)| match target {
            ContentTarget::Item => item,
            ContentTarget::Ability { vocabulary: v } => !item && Some(v.as_str()) == vocabulary,
            ContentTarget::Refine => false,
        })
        .map(|(kind, _)| kind.clone())
}

impl ActorContext {
    /// What the planner diffs against: each target's value, and the links.
    pub fn snapshot(&self, mapping: &SheetMapping) -> ActorSnapshot {
        let mut values = BTreeMap::new();
        values.insert(
            "actor.label".to_string(),
            Value::String(self.state.label.clone()),
        );
        for (data_type, object) in &self.state.system_data {
            for (key, value) in object.as_object().into_iter().flatten() {
                values.insert(format!("{data_type}.{key}"), value.clone());
            }
        }
        let mut links = Vec::new();
        for link in &self.state.abilities {
            let Some(kind) = neutral_kind(mapping, false, link.vocabulary.as_deref()) else {
                continue;
            };
            let Some(id) = link.ability_id.or(link.staged_id) else {
                continue;
            };
            links.push(CurrentLink {
                kind,
                name: link.name.clone(),
                id: id.to_string(),
                staged: link.staged_id.is_some(),
            });
        }
        if let Some(kind) = neutral_kind(mapping, true, None) {
            for link in &self.state.inventory {
                let Some(id) = link.item_id.or(link.staged_id) else {
                    continue;
                };
                links.push(CurrentLink {
                    kind: kind.clone(),
                    name: link.name.clone(),
                    id: id.to_string(),
                    staged: link.staged_id.is_some(),
                });
            }
        }
        ActorSnapshot {
            is_reimport: self.is_reimport,
            values,
            links,
        }
    }

    /// The system data as an object per type, empty where the actor has none.
    pub fn data_objects(&self) -> BTreeMap<String, Map<String, Value>> {
        DATA_TYPES
            .iter()
            .map(|data_type| {
                let object = self
                    .state
                    .system_data
                    .get(*data_type)
                    .and_then(Value::as_object)
                    .cloned()
                    .unwrap_or_default();
                (data_type.to_string(), object)
            })
            .collect()
    }
}
