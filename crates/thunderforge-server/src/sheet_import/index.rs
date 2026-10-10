//! What a world already holds, for the planner to resolve content against
//! (FR-031): its own abilities and items, and what is staged in it.

use std::collections::HashMap;

use diesel::prelude::*;
use thunderforge_sheet_import::{
    ContentChange, ContentIndex, ContentTarget, Indexed, SheetMapping, normalise_name,
};
use uuid::Uuid;

use crate::staged_content::StagedState;

/// The kind a piece is staged under: its vocabulary type, or `item`.
pub fn staged_kind(change: &ContentChange) -> String {
    match &change.target {
        ContentTarget::Ability { vocabulary } => vocabulary.clone(),
        ContentTarget::Item => "item".to_string(),
        ContentTarget::Refine => change.kind.clone(),
    }
}

pub struct WorldIndex {
    targets: HashMap<String, ContentTarget>,
    /// (vocabulary type, normalised name) to the world ability.
    abilities: HashMap<(String, String), Uuid>,
    /// Normalised name to the world item.
    items: HashMap<String, Uuid>,
    /// (staged kind, normalised name) to the latest piece not yet adopted.
    staged: HashMap<(String, String), (Uuid, String)>,
}

impl WorldIndex {
    pub fn load(
        conn: &mut PgConnection,
        world_id: Uuid,
        mapping: &SheetMapping,
    ) -> QueryResult<Self> {
        use crate::schema::{world_abilities, world_items, world_staged_content as staged};

        let mut abilities = HashMap::new();
        for (id, name, classification) in world_abilities::table
            .filter(world_abilities::world_id.eq(world_id))
            .order(world_abilities::created_at.asc())
            .select((
                world_abilities::id,
                world_abilities::name,
                world_abilities::classification,
            ))
            .load::<(Uuid, String, String)>(conn)?
        {
            abilities
                .entry((classification, normalise_name(&name)))
                .or_insert(id);
        }
        let mut items = HashMap::new();
        for (id, name) in world_items::table
            .filter(world_items::world_id.eq(world_id))
            .order(world_items::created_at.asc())
            .select((world_items::id, world_items::name))
            .load::<(Uuid, String)>(conn)?
        {
            items.entry(normalise_name(&name)).or_insert(id);
        }
        let mut pieces = HashMap::new();
        for (id, kind, normalised, hash) in staged::table
            .filter(staged::world_id.eq(world_id))
            .filter(staged::state.ne(StagedState::Adopted))
            .order(staged::created_at.asc())
            .select((
                staged::id,
                staged::kind,
                staged::normalized_name,
                staged::content_hash,
            ))
            .load::<(Uuid, String, String, String)>(conn)?
        {
            pieces.insert((kind, normalised), (id, hash));
        }
        Ok(Self {
            targets: mapping.content.clone().into_iter().collect(),
            abilities,
            items,
            staged: pieces,
        })
    }
}

impl ContentIndex for WorldIndex {
    fn lookup(&self, kind: &str, normalised: &str) -> Option<Indexed> {
        let name = normalised.to_string();
        let world = match self.targets.get(kind) {
            Some(ContentTarget::Item) => self.items.get(&name).copied(),
            Some(ContentTarget::Ability { vocabulary }) => self
                .abilities
                .get(&(vocabulary.clone(), name.clone()))
                .copied(),
            // The pack decides where it lands: an item of that name, or an
            // ability of any type.
            Some(ContentTarget::Refine) => self.items.get(&name).copied().or_else(|| {
                self.abilities
                    .iter()
                    .filter(|((_, n), _)| *n == name)
                    .min_by(|a, b| a.0.0.cmp(&b.0.0))
                    .map(|(_, id)| *id)
            }),
            None => None,
        };
        if let Some(id) = world {
            return Some(Indexed::World { id: id.to_string() });
        }
        let stored = match self.targets.get(kind) {
            Some(ContentTarget::Item) => vec!["item".to_string()],
            Some(ContentTarget::Ability { vocabulary }) => vec![vocabulary.clone()],
            _ => {
                let mut kinds: Vec<String> = self
                    .staged
                    .keys()
                    .filter(|(_, n)| *n == name)
                    .map(|(k, _)| k.clone())
                    .collect();
                kinds.sort();
                kinds
            }
        };
        stored.into_iter().find_map(|kind| {
            self.staged
                .get(&(kind, name.clone()))
                .map(|(id, hash)| Indexed::Staged {
                    id: id.to_string(),
                    content_hash: hash.clone(),
                })
        })
    }
}
