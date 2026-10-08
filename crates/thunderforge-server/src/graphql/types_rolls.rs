//! Spec 081: a roll as one member of the table may see it.
//!
//! `WorldRoll` is the whole roll. `MaskedRoll` is another player's roll for
//! the GM's eyes, and it has no field that could hold what was rolled
//! (FR-004): it is built from four values that say nothing about the dice,
//! so a mistake in a resolver has nothing to leak.

use async_graphql::{Enum, SimpleObject, Union};

use crate::graphql::types::{GraphQLRollResolution, stored_outcome};
use crate::models::RollRecord;
use crate::rolls::facets::facet_labels;
use crate::rolls::visibility::Visibility;
use thunderforge_canvas_core::roll_facets::Advantage;
use thunderforge_dice::{ResolutionKind, RollResolution};

/// Who a roll is for.
#[derive(Enum, Copy, Clone, Debug, PartialEq, Eq)]
pub enum RollVisibility {
    Everyone,
    GmEyes,
    GmOnly,
}

impl From<RollVisibility> for Visibility {
    fn from(value: RollVisibility) -> Self {
        match value {
            RollVisibility::Everyone => Visibility::Everyone,
            RollVisibility::GmEyes => Visibility::GmEyes,
            RollVisibility::GmOnly => Visibility::GmOnly,
        }
    }
}

impl From<Visibility> for RollVisibility {
    fn from(value: Visibility) -> Self {
        match value {
            Visibility::Everyone => RollVisibility::Everyone,
            Visibility::GmEyes => RollVisibility::GmEyes,
            Visibility::GmOnly => RollVisibility::GmOnly,
        }
    }
}

/// One placeholder of a roll's formula and the number the server put in
/// for it (spec 083 FR-003).
#[derive(SimpleObject, Debug, Clone, PartialEq)]
pub struct RollBinding {
    pub placeholder: String,
    pub value: f64,
}

/// A row's recorded bindings, sorted. A null or unreadable column reads as
/// none: the roll is still shown, with its formula as written.
fn row_bindings(stored: Option<&serde_json::Value>) -> Vec<RollBinding> {
    let Some(serde_json::Value::Object(map)) = stored else {
        return Vec::new();
    };
    let mut bindings: Vec<RollBinding> = map
        .iter()
        .filter_map(|(name, value)| {
            value.as_f64().map(|value| RollBinding {
                placeholder: name.clone(),
                value,
            })
        })
        .collect();
    bindings.sort_by(|a, b| a.placeholder.cmp(&b.placeholder));
    bindings
}

/// Spec 084: a facet or a spend, by its id and the name the table sees.
#[derive(SimpleObject, Debug, Clone, PartialEq, Eq)]
pub struct RollFacet {
    pub id: String,
    pub label: String,
}

/// Spec 084: roll a d20 test twice and keep the higher or the lower.
#[derive(Enum, Copy, Clone, Debug, Default, PartialEq, Eq)]
#[graphql(name = "Advantage")]
pub enum GraphQLAdvantage {
    #[default]
    Normal,
    Advantage,
    Disadvantage,
}

impl From<GraphQLAdvantage> for Advantage {
    fn from(value: GraphQLAdvantage) -> Self {
        match value {
            GraphQLAdvantage::Normal => Advantage::Normal,
            GraphQLAdvantage::Advantage => Advantage::Advantage,
            GraphQLAdvantage::Disadvantage => Advantage::Disadvantage,
        }
    }
}

/// What a row alone cannot say: the system that names its facets, and the
/// roll that replaced it, if one has.
#[derive(Debug, Clone, Copy, Default)]
pub struct RollLinks<'a> {
    pub system_id: &'a str,
    pub rerolled_by: Option<uuid::Uuid>,
}

fn named(system_id: &str, ids: &[String]) -> Vec<RollFacet> {
    facet_labels(system_id, ids)
        .into_iter()
        .map(|(id, label)| RollFacet { id, label })
        .collect()
}

/// A roll, whole.
#[derive(SimpleObject, Debug, Clone)]
pub struct WorldRoll {
    pub id: uuid::Uuid,
    pub roller_id: uuid::Uuid,
    pub roller_name: String,
    /// What it was for: "Stealth", "Longsword".
    pub label: Option<String>,
    pub formula: String,
    /// The values the server substituted for the formula's placeholders,
    /// sorted by placeholder.
    pub bindings: Vec<RollBinding>,
    pub resolution: GraphQLRollResolution,
    pub visibility: RollVisibility,
    pub created_at: String,
    pub revealed_at: Option<String>,
    pub revealed_by_name: Option<String>,
    /// Spec 084: the facets applied, plus the spend on a reroll.
    pub facets: Vec<RollFacet>,
    /// The roll this one replaces.
    pub reroll_of: Option<uuid::Uuid>,
    /// The roll that replaced this one.
    pub rerolled_by: Option<uuid::Uuid>,
    /// What this reroll spent.
    pub spent: Option<RollFacet>,
    /// Only for the roll's maker: the spends the server would accept now.
    pub reroll_offers: Vec<RollFacet>,
    /// RFC 3339; null when the roll cannot be rerolled at all.
    pub reroll_until: Option<String>,
}

impl WorldRoll {
    pub fn from_row(
        row: RollRecord,
        roller_name: String,
        revealed_by_name: Option<String>,
        links: RollLinks<'_>,
    ) -> Self {
        // Written only after a successful resolve; a row that no longer reads
        // is shown with its total and no dice rather than refused.
        let resolution: RollResolution =
            serde_json::from_value(row.detail.clone()).unwrap_or(RollResolution {
                formula: row.formula.clone(),
                dice: Vec::new(),
                kind: ResolutionKind::Total(row.result_value),
            });
        WorldRoll {
            id: row.id,
            roller_id: row.triggered_by,
            roller_name,
            label: row.label.clone(),
            formula: row.formula.clone(),
            bindings: row_bindings(row.bindings.as_ref()),
            resolution: GraphQLRollResolution {
                outcome: stored_outcome(&row),
                ..GraphQLRollResolution::from(&resolution)
            },
            visibility: Visibility::parse(&row.visibility).into(),
            created_at: row.created_at.to_rfc3339(),
            revealed_at: row.revealed_at.map(|at| at.to_rfc3339()),
            revealed_by_name,
            facets: named(links.system_id, &row.facet_ids()),
            reroll_of: row.reroll_of,
            rerolled_by: links.rerolled_by,
            spent: row
                .reroll_spent
                .as_ref()
                .and_then(|id| named(links.system_id, std::slice::from_ref(id)).pop()),
            reroll_offers: Vec::new(),
            reroll_until: None,
        }
    }
}

/// Another player's roll for the GM's eyes: that it happened, by whom and
/// when, and nothing of what it was.
#[derive(SimpleObject, Debug, Clone)]
pub struct MaskedRoll {
    pub id: uuid::Uuid,
    pub roller_name: String,
    pub created_at: String,
    pub visibility: RollVisibility,
}

impl MaskedRoll {
    /// The only way to make one, and it is never handed the roll's row.
    pub fn new(
        id: uuid::Uuid,
        roller_name: String,
        created_at: chrono::DateTime<chrono::Utc>,
        visibility: Visibility,
    ) -> Self {
        MaskedRoll {
            id,
            roller_name,
            created_at: created_at.to_rfc3339(),
            visibility: visibility.into(),
        }
    }
}

/// A roll as the caller may see it. The whole roll is boxed: it is several
/// times the size of a masked one.
#[derive(Union, Debug, Clone)]
pub enum WorldRollEntry {
    WorldRoll(Box<WorldRoll>),
    MaskedRoll(MaskedRoll),
}

#[cfg(test)]
#[path = "roll_bindings_tests.rs"]
mod roll_bindings_tests;
