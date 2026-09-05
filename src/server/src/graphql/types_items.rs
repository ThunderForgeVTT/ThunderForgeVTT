//! Items, their effects, who may see them, and what a Game Master says they
//! cost.
//!
//! Spec 013's item and inventory types, plus spec 031's price note — see the
//! header of `types_actors.rs` for why that one lives here rather than with
//! the rest of spec 031.

use super::ActorPermissionLevel;
use super::types_lore::GraphQLLoreEntry;
use crate::models::WorldItemPrice;
use async_graphql::SimpleObject;
use chrono::NaiveDateTime;

use crate::models::{ActorInventoryEntry, ItemEffect, ItemPermission, ItemShare, WorldItem};

/// Kind of an Item Effect. `Modifier` covers both stat boosts and
/// detriments via a signed formula (e.g. `-1d4`) — no separate buff/debuff
/// variant (research.md §1). Extensible by adding new variants only.
#[derive(async_graphql::Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum ItemEffectType {
    Heal,
    Damage,
    Modifier,
    AttackRoll,
}

impl ItemEffectType {
    pub fn as_db_str(self) -> &'static str {
        match self {
            ItemEffectType::Heal => "heal",
            ItemEffectType::Damage => "damage",
            ItemEffectType::Modifier => "modifier",
            ItemEffectType::AttackRoll => "attack_roll",
        }
    }

    pub fn from_db_str(value: &str) -> Option<Self> {
        match value {
            "heal" => Some(ItemEffectType::Heal),
            "damage" => Some(ItemEffectType::Damage),
            "modifier" => Some(ItemEffectType::Modifier),
            "attack_roll" => Some(ItemEffectType::AttackRoll),
            _ => None,
        }
    }
}

/// Scaffolded per FR-004a — not evaluated/enforced by any code path in
/// this pass; exists so a future dice-roller spec can add real triggering
/// without redesigning `world_item_effects`.
#[derive(async_graphql::Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum ItemEffectTrigger {
    OnUse,
    Passive,
}

impl ItemEffectTrigger {
    pub fn as_db_str(self) -> &'static str {
        match self {
            ItemEffectTrigger::OnUse => "on_use",
            ItemEffectTrigger::Passive => "passive",
        }
    }

    pub fn from_db_str(value: &str) -> Option<Self> {
        match value {
            "on_use" => Some(ItemEffectTrigger::OnUse),
            "passive" => Some(ItemEffectTrigger::Passive),
            _ => None,
        }
    }
}

#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLItemEffect {
    pub id: uuid::Uuid,
    pub item_id: uuid::Uuid,
    pub effect_type: ItemEffectType,
    pub formula: String,
    pub target: String,
    pub trigger_kind: Option<ItemEffectTrigger>,
    pub sort_order: i32,
}

impl From<ItemEffect> for GraphQLItemEffect {
    fn from(row: ItemEffect) -> Self {
        Self {
            id: row.id,
            item_id: row.item_id,
            effect_type: ItemEffectType::from_db_str(&row.effect_type)
                .unwrap_or(ItemEffectType::Modifier),
            formula: row.formula,
            target: row.target,
            trigger_kind: row
                .trigger_kind
                .as_deref()
                .and_then(ItemEffectTrigger::from_db_str),
            sort_order: row.sort_order,
        }
    }
}

/// An Item's own GraphQL projection. `effects` is resolved separately by
/// the owning query/mutation (not a field resolver here) since every
/// current call site already has both rows in hand after a join/second
/// query. `linkedFromLore` (spec 013 US3) IS a per-request field
/// resolver — hence `#[graphql(complex)]`, mirroring `GraphQLWorldActor`'s
/// existing `lore_linked_from` pattern (`src/server/src/graphql.rs`).
#[derive(SimpleObject, Debug, Clone)]
#[graphql(complex)]
pub struct GraphQLItem {
    pub id: uuid::Uuid,
    pub world_id: uuid::Uuid,
    pub name: String,
    pub description: Option<String>,
    pub icon_asset_id: Option<uuid::Uuid>,
    pub effects: Vec<GraphQLItemEffect>,
    pub my_permission_level: ActorPermissionLevel,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    /// Spec 015: true when this item is currently disabled by a DMCA
    /// takedown — when true, `name`/`description`/`effects` are a
    /// placeholder, never the real content, for every caller including
    /// the owner (contracts/graphql-moderation.md's enforcement contract).
    pub moderated: bool,
    /// Spec 015: the disabling case's id, present only on a moderation
    /// placeholder — lets the owner's client jump straight into
    /// `submitCounterNotice` (FR-005) without a staff-only case lookup.
    pub moderation_case_id: Option<uuid::Uuid>,
}

impl GraphQLItem {
    pub fn from_row(
        row: WorldItem,
        effects: Vec<ItemEffect>,
        my_permission_level: ActorPermissionLevel,
    ) -> Self {
        Self {
            id: row.id,
            world_id: row.world_id,
            name: row.name,
            description: row.description,
            icon_asset_id: row.icon_asset_id,
            effects: effects.into_iter().map(GraphQLItemEffect::from).collect(),
            my_permission_level,
            created_at: row.created_at,
            updated_at: row.updated_at,
            moderated: false,
            moderation_case_id: None,
        }
    }

    /// Spec 015: the placeholder returned in place of real content for a
    /// moderation-disabled item.
    pub fn moderated_placeholder(
        id: uuid::Uuid,
        world_id: uuid::Uuid,
        my_permission_level: ActorPermissionLevel,
        moderation_case_id: Option<uuid::Uuid>,
    ) -> Self {
        let now = chrono::Utc::now().naive_utc();
        Self {
            id,
            world_id,
            name: "[Content removed in response to a takedown notice]".to_string(),
            description: None,
            icon_asset_id: None,
            effects: Vec::new(),
            my_permission_level,
            created_at: now,
            updated_at: now,
            moderated: true,
            moderation_case_id,
        }
    }
}

#[async_graphql::ComplexObject]
impl GraphQLItem {
    /// Spec 013 (US3, FR-016): every lore entry whose body currently
    /// contains a resolved in-text link to this item — mirrors
    /// `GraphQLWorldActor::lore_linked_from` (`src/server/src/graphql.rs`)
    /// and `GraphQLLoreEntry::linked_from` verbatim, extended to items.
    async fn linked_from_lore(
        &self,
        ctx: &async_graphql::Context<'_>,
    ) -> async_graphql::Result<Vec<GraphQLLoreEntry>> {
        let state = crate::graphql::app_state(ctx)?;
        crate::graphql::queries::lore::lore_entries_linking_to_item(state, self.id).await
    }

    /// Spec 031 (FR-037): the Game Master's price note, or `null` where none
    /// was written.
    ///
    /// A field resolver rather than a column carried on the row, because a
    /// price is a separate table by ADR-058's decision — and because a
    /// moderation placeholder must not carry one: `moderated_placeholder`
    /// builds an item with no real content, and a price left attached to it
    /// would be the one true thing on an otherwise blanked entry.
    async fn price(
        &self,
        ctx: &async_graphql::Context<'_>,
    ) -> async_graphql::Result<Option<GraphQLItemPrice>> {
        if self.moderated {
            return Ok(None);
        }
        let state = crate::graphql::app_state(ctx)?;
        Ok(
            crate::graphql::mutations_item_prices::item_price_impl(state, self.id)
                .await?
                .map(GraphQLItemPrice::from),
        )
    }
}

#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLItemPermission {
    pub item_id: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub level: ActorPermissionLevel,
    pub updated_at: NaiveDateTime,
}

impl From<ItemPermission> for GraphQLItemPermission {
    fn from(row: ItemPermission) -> Self {
        Self {
            item_id: row.item_id,
            user_id: row.user_id,
            level: ActorPermissionLevel::from_db_str(&row.level)
                .unwrap_or(ActorPermissionLevel::Viewer),
            updated_at: row.updated_at,
        }
    }
}

#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLItemShareLink {
    pub id: uuid::Uuid,
    pub item_id: uuid::Uuid,
    pub share_code: String,
    pub revoked: bool,
    pub created_at: NaiveDateTime,
}

impl From<ItemShare> for GraphQLItemShareLink {
    fn from(row: ItemShare) -> Self {
        Self {
            id: row.id,
            item_id: row.item_id,
            share_code: row.share_code,
            revoked: row.revoked,
            created_at: row.created_at,
        }
    }
}

/// Read-only, world-identity-scrubbed projection of a shared item
/// (mirrors `SharedActorPreview` — excludes id/worldId/createdBy/
/// ownership block so an arbitrary logged-in viewer can't learn the
/// source world, per contracts/item-share.md).
#[derive(SimpleObject, Debug, Clone)]
pub struct SharedItemPreview {
    pub name: String,
    pub description: Option<String>,
    pub icon_asset_id: Option<uuid::Uuid>,
    pub effects: Vec<GraphQLItemEffect>,
}

#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLInventoryEntry {
    pub id: uuid::Uuid,
    pub actor_id: uuid::Uuid,
    pub item_id: Option<uuid::Uuid>,
    pub item_name: String,
    pub quantity: i32,
}

impl From<ActorInventoryEntry> for GraphQLInventoryEntry {
    fn from(row: ActorInventoryEntry) -> Self {
        Self {
            id: row.id,
            actor_id: row.actor_id,
            item_id: row.item_id,
            item_name: row.item_name_snapshot,
            quantity: row.quantity,
        }
    }
}

// ============================================================================

/// What a Game Master says an item costs (FR-037).
///
/// Presentational (ADR-058) — the fields are a number, a free-text label and
/// the author's intent, and there is deliberately nothing here to compute
/// with. A system that models its own economy keeps it in its own type.
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLItemPrice {
    pub item_id: uuid::Uuid,
    pub amount: i32,
    pub currency_label: Option<String>,
    pub is_suggested: bool,
    pub updated_at: NaiveDateTime,
}

impl From<WorldItemPrice> for GraphQLItemPrice {
    fn from(row: WorldItemPrice) -> Self {
        Self {
            item_id: row.item_id,
            amount: row.amount,
            currency_label: row.currency_label,
            is_suggested: row.is_suggested,
            updated_at: row.updated_at,
        }
    }
}
