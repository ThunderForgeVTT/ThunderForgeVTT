//! The world abilities compendium: abilities, their effects and their
//! permissions (spec 025).

use super::ActorPermissionLevel;
use super::types_lore::GraphQLLoreEntry;
use async_graphql::SimpleObject;
use chrono::NaiveDateTime;

use crate::models::{AbilityEffect, AbilityPermission, WorldAbility};

/// An ability's type, as a **stable identity** rather than a closed set.
///
/// # Why this stopped being an enum
///
/// It was `enum AbilityClassification { Spell, Feat, Power, Talent }`, and its
/// own doc said the set was fixed and systems "cannot add to it". Spec 033
/// FR-011 makes the available types the union of the built-ins and whatever
/// the world's system declares, so a 5e pack may name an Enchantment.
///
/// A GraphQL enum cannot carry a value a pack invented — introspection
/// publishes a closed set, and a client validating against it would reject the
/// Enchantment outright. So the wire type is the identity itself, described by
/// `abilityVocabulary(worldId)`, which the same request can fetch. This is the
/// move `DeclaredValue` made in spec 032, for the same reason (ADR-064).
///
/// Stored lowercase. `normalise` is the one place that decides so, because a
/// client sending `"SPELL"` and one sending `"spell"` must mean the same type.
pub fn normalise_classification(value: &str) -> String {
    value.trim().to_lowercase()
}

/// Spec 025 (FR-016): effect types, matching `ItemEffectType`'s set exactly so
/// a future resolution engine can consume item and ability effects through one
/// code path.
#[derive(async_graphql::Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum AbilityEffectType {
    Heal,
    Damage,
    Modifier,
    AttackRoll,
}

impl AbilityEffectType {
    pub fn as_db_str(self) -> &'static str {
        match self {
            AbilityEffectType::Heal => "heal",
            AbilityEffectType::Damage => "damage",
            AbilityEffectType::Modifier => "modifier",
            AbilityEffectType::AttackRoll => "attack_roll",
        }
    }

    pub fn from_db_str(value: &str) -> Option<Self> {
        match value {
            "heal" => Some(AbilityEffectType::Heal),
            "damage" => Some(AbilityEffectType::Damage),
            "modifier" => Some(AbilityEffectType::Modifier),
            "attack_roll" => Some(AbilityEffectType::AttackRoll),
            _ => None,
        }
    }
}

/// Spec 025 (FR-020): scaffolded, never evaluated in this pass — exists so a
/// future resolution spec can add real triggering without redesigning the table.
#[derive(async_graphql::Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum AbilityEffectTrigger {
    OnUse,
    Passive,
}

impl AbilityEffectTrigger {
    pub fn as_db_str(self) -> &'static str {
        match self {
            AbilityEffectTrigger::OnUse => "on_use",
            AbilityEffectTrigger::Passive => "passive",
        }
    }

    pub fn from_db_str(value: &str) -> Option<Self> {
        match value {
            "on_use" => Some(AbilityEffectTrigger::OnUse),
            "passive" => Some(AbilityEffectTrigger::Passive),
            _ => None,
        }
    }
}

/// One authored effect on an ability. Inert data (FR-019) — nothing in this
/// spec resolves, rolls, or applies it.
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLAbilityEffect {
    pub id: uuid::Uuid,
    pub ability_id: uuid::Uuid,
    pub effect_type: AbilityEffectType,
    pub formula: String,
    pub target: String,
    pub trigger_kind: Option<AbilityEffectTrigger>,
    pub sort_order: i32,
}

impl From<AbilityEffect> for GraphQLAbilityEffect {
    fn from(row: AbilityEffect) -> Self {
        Self {
            id: row.id,
            ability_id: row.ability_id,
            // An unrecognized DB string falls back rather than erroring,
            // mirroring GraphQLItemEffect — a row written by a newer version
            // must not break an older reader.
            effect_type: AbilityEffectType::from_db_str(&row.effect_type)
                .unwrap_or(AbilityEffectType::Modifier),
            formula: row.formula,
            target: row.target,
            trigger_kind: row
                .trigger_kind
                .as_deref()
                .and_then(AbilityEffectTrigger::from_db_str),
            sort_order: row.sort_order,
        }
    }
}

/// An Ability's GraphQL projection. Mirrors `GraphQLItem`, including its
/// `#[graphql(complex)]` backlink field.
#[derive(SimpleObject, Debug, Clone)]
#[graphql(complex)]
pub struct GraphQLAbility {
    pub id: uuid::Uuid,
    pub world_id: uuid::Uuid,
    pub name: String,
    pub description: Option<String>,
    pub classification: String,
    /// The value on this type's declared grade, where it declares one
    /// (spec 033 FR-021). `null` for an ungraded type, so a surface can show
    /// nothing rather than a zero that means something (FR-022).
    pub grade: Option<i32>,
    /// Spec 025 (FR-024a): visibility, deliberately separate from
    /// `my_permission_level`. Only ever `true` in a response to a DM — every
    /// non-DM read path filters GM-only abilities out entirely (FR-024b), so a
    /// player can never receive a row with this set.
    pub gm_only: bool,
    pub effects: Vec<GraphQLAbilityEffect>,
    /// Edit rights only — NOT visibility. See `auth::ability_permissions`.
    pub my_permission_level: ActorPermissionLevel,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    /// Spec 015: true when disabled by a DMCA takedown; the content fields are
    /// then a placeholder for every caller including the owner.
    pub moderated: bool,
    pub moderation_case_id: Option<uuid::Uuid>,
}

impl GraphQLAbility {
    pub fn from_row(
        row: WorldAbility,
        effects: Vec<GraphQLAbilityEffect>,
        my_permission_level: ActorPermissionLevel,
    ) -> Self {
        Self {
            id: row.id,
            world_id: row.world_id,
            name: row.name,
            description: row.description,
            // An unrecognized DB string falls back rather than erroring,
            // mirroring GraphQLItemEffect's handling — a row written by a
            // newer version must not break an older reader.
            // T037: this read `unwrap_or("spell".to_string())`, so an
            // ability of a type the build did not know was silently presented
            // as a Spell. Its comment argued a newer row must not break an
            // older reader, which was fair — but the behaviour is exactly what
            // FR-034 forbids, and dropping the CHECK constraint makes a fifth
            // value writable, so the case stops being hypothetical here.
            //
            // The identity is carried through as itself. What a person reads
            // comes from the world's vocabulary, which resolves an unrecognised
            // type to the identity rather than to another type's name.
            classification: normalise_classification(&row.classification),
            grade: row.grade,
            gm_only: row.gm_only,
            effects,
            my_permission_level,
            created_at: row.created_at,
            updated_at: row.updated_at,
            moderated: false,
            moderation_case_id: None,
        }
    }

    /// Spec 015: the placeholder returned in place of real content for a
    /// moderation-disabled ability.
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
            classification: "spell".to_string(),
            // A moderated placeholder carries no grade: the ability's own
            // values are removed with the rest of its content.
            grade: None,
            gm_only: false,
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
impl GraphQLAbility {
    /// Spec 025 (FR-029): every lore entry whose body currently contains a
    /// resolved in-text link to this ability. Named `linkedFromLore` to match
    /// `GraphQLItem` (the newer convention; actors use the older
    /// `loreLinkedFrom`).
    async fn linked_from_lore(
        &self,
        ctx: &async_graphql::Context<'_>,
    ) -> async_graphql::Result<Vec<GraphQLLoreEntry>> {
        let state = crate::graphql::app_state(ctx)?;
        crate::graphql::queries::lore::lore_entries_linking_to_ability(state, self.id).await
    }
}

/// Spec 025 (FR-024): an ability's ownership-block entry. Governs edit rights;
/// visibility is `GraphQLAbility::gm_only`.
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLAbilityPermission {
    pub ability_id: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub level: ActorPermissionLevel,
    pub updated_at: NaiveDateTime,
}

impl From<AbilityPermission> for GraphQLAbilityPermission {
    fn from(row: AbilityPermission) -> Self {
        Self {
            ability_id: row.ability_id,
            user_id: row.user_id,
            level: ActorPermissionLevel::from_db_str(&row.level)
                .unwrap_or(ActorPermissionLevel::Viewer),
            updated_at: row.updated_at,
        }
    }
}

/// Spec 025 (FR-021): an actor's known-ability entry.
///
/// `ability_id`/`classification` are null for a tombstoned row — the ability
/// was deleted — while `ability_name` always survives so the entry stays
/// identifiable (FR-023).
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLActorAbilityEntry {
    pub id: uuid::Uuid,
    pub actor_id: uuid::Uuid,
    pub ability_id: Option<uuid::Uuid>,
    pub ability_name: String,
    pub classification: Option<String>,
    pub gm_only: bool,
}

/// Spec 025 (FR-032): an ability share link.
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLAbilityShareLink {
    pub id: uuid::Uuid,
    pub ability_id: uuid::Uuid,
    pub share_code: String,
    pub revoked: bool,
    pub created_at: NaiveDateTime,
}

/// Spec 025 (FR-033): what a share-link viewer sees.
///
/// Deliberately carries **no** `id`, `world_id`, `created_by`, or ownership
/// block: a viewer must not be able to identify the source world or its
/// members. Mirrors `SharedItemPreview`.
#[derive(SimpleObject, Debug, Clone)]
pub struct SharedAbilityPreview {
    pub name: String,
    pub description: Option<String>,
    pub classification: String,
    /// The word the *owning world's* system uses for this ability's type.
    ///
    /// Spec 033 FR-006: every surface naming an ability type uses the system's
    /// vocabulary, and a share view is such a surface. It cannot resolve one
    /// itself — the viewer is deliberately not a member of that world (that is
    /// what a share link is for), so they cannot read its vocabulary. The
    /// server, which knows the world, resolves it here.
    ///
    /// Falls back to the stored type identity for a type no system recognises,
    /// never to another type's name (FR-034, FR-035).
    pub classification_label: String,
    pub effects: Vec<GraphQLAbilityEffect>,
}

// ============================================================================
