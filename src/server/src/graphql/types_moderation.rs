//! Notice-and-takedown: what a moderation action looks like over the wire
//! (spec 015).

use async_graphql::SimpleObject;

use crate::models::ContentModerationAction;

// The shared "World" prefix mirrors this codebase's actual table/db-string
// names (world_actor, world_item, world_lore_entry) rather than being
// redundant — dropping it would desync the variant names from the
// `as_db_str`/`from_db_str` strings they map to.
#[allow(clippy::enum_variant_names)]
#[derive(async_graphql::Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum ModerationEntityType {
    WorldActor,
    WorldItem,
    WorldLoreEntry,
    /// Spec 025: abilities are moderatable per spec 015 FR-010's
    /// individual-compendium-entry granularity requirement. Without this a
    /// share link would be a moderation bypass for exactly the content type
    /// ADR-049's DMCA determination concerns.
    WorldAbility,
}

impl ModerationEntityType {
    pub fn as_db_str(self) -> &'static str {
        match self {
            ModerationEntityType::WorldActor => "world_actor",
            ModerationEntityType::WorldItem => "world_item",
            ModerationEntityType::WorldLoreEntry => "world_lore_entry",
            ModerationEntityType::WorldAbility => "world_ability",
        }
    }

    pub fn from_db_str(value: &str) -> Option<Self> {
        match value {
            "world_actor" => Some(ModerationEntityType::WorldActor),
            "world_item" => Some(ModerationEntityType::WorldItem),
            "world_lore_entry" => Some(ModerationEntityType::WorldLoreEntry),
            "world_ability" => Some(ModerationEntityType::WorldAbility),
            _ => None,
        }
    }
}

#[derive(async_graphql::Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum ModerationActionType {
    NoticeReceived,
    NoticeRejectedIncomplete,
    ContentDisabled,
    CounterNoticeReceived,
    CounterNoticeForwarded,
    ContentRestored,
    ContentRemainsDisabled,
}

impl ModerationActionType {
    pub fn as_db_str(self) -> &'static str {
        use crate::moderation::action_type as at;
        match self {
            ModerationActionType::NoticeReceived => at::NOTICE_RECEIVED,
            ModerationActionType::NoticeRejectedIncomplete => at::NOTICE_REJECTED_INCOMPLETE,
            ModerationActionType::ContentDisabled => at::CONTENT_DISABLED,
            ModerationActionType::CounterNoticeReceived => at::COUNTER_NOTICE_RECEIVED,
            ModerationActionType::CounterNoticeForwarded => at::COUNTER_NOTICE_FORWARDED,
            ModerationActionType::ContentRestored => at::CONTENT_RESTORED,
            ModerationActionType::ContentRemainsDisabled => at::CONTENT_REMAINS_DISABLED,
        }
    }

    pub fn from_db_str(value: &str) -> Option<Self> {
        use crate::moderation::action_type as at;
        match value {
            v if v == at::NOTICE_RECEIVED => Some(ModerationActionType::NoticeReceived),
            v if v == at::NOTICE_REJECTED_INCOMPLETE => {
                Some(ModerationActionType::NoticeRejectedIncomplete)
            }
            v if v == at::CONTENT_DISABLED => Some(ModerationActionType::ContentDisabled),
            v if v == at::COUNTER_NOTICE_RECEIVED => {
                Some(ModerationActionType::CounterNoticeReceived)
            }
            v if v == at::COUNTER_NOTICE_FORWARDED => {
                Some(ModerationActionType::CounterNoticeForwarded)
            }
            v if v == at::CONTENT_RESTORED => Some(ModerationActionType::ContentRestored),
            v if v == at::CONTENT_REMAINS_DISABLED => {
                Some(ModerationActionType::ContentRemainsDisabled)
            }
            _ => None,
        }
    }
}

#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLModerationAction {
    pub id: uuid::Uuid,
    pub case_id: uuid::Uuid,
    pub action_type: ModerationActionType,
    pub entity_type: ModerationEntityType,
    pub entity_id: uuid::Uuid,
    pub world_id: uuid::Uuid,
    pub validity_result: Option<String>,
    pub missing_elements: Option<Vec<String>>,
    pub restoration_due_at: Option<String>,
    pub created_at: String,
}

impl From<ContentModerationAction> for GraphQLModerationAction {
    fn from(row: ContentModerationAction) -> Self {
        Self {
            id: row.id,
            case_id: row.case_id,
            action_type: ModerationActionType::from_db_str(&row.action_type)
                .unwrap_or(ModerationActionType::NoticeReceived),
            entity_type: ModerationEntityType::from_db_str(&row.entity_type)
                .unwrap_or(ModerationEntityType::WorldActor),
            entity_id: row.entity_id,
            world_id: row.world_id,
            validity_result: row.validity_result,
            missing_elements: row
                .missing_elements
                .map(|v| v.into_iter().flatten().collect()),
            restoration_due_at: row.restoration_due_at.map(|dt| dt.to_rfc3339()),
            created_at: row.created_at.to_rfc3339(),
        }
    }
}

/// One takedown case's full event thread (data-model.md's `case_id`
/// grouping), plus its currently-effective status.
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLModerationCase {
    pub case_id: uuid::Uuid,
    pub entity_type: ModerationEntityType,
    pub entity_id: uuid::Uuid,
    pub world_id: uuid::Uuid,
    pub current_status: ModerationActionType,
    pub events: Vec<GraphQLModerationAction>,
}

// ============================================================================
