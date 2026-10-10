//! A world event's code as a bounded label (contracts/server-instruments.md,
//! FR-014): the constant's own name, lower-cased, without `EVENT_CODE_`.
//!
//! A code with no name here counts as `unknown`, and the scan test below
//! fails, so a new event cannot ship without one.

use crate::world_events::*;

/// The label for `code`, or `unknown`.
pub fn event_name(code: i32) -> &'static str {
    match code {
        EVENT_CODE_WALL_CHANGED => "wall_changed",
        EVENT_CODE_LIGHT_SOURCE_CHANGED => "light_source_changed",
        EVENT_CODE_SHAPE_CHANGED => "shape_changed",
        EVENT_CODE_MAP_IMPORTED => "map_imported",
        EVENT_CODE_TOKEN_CHANGED => "token_changed",
        EVENT_CODE_GENIE_SESSION_STATE => "genie_session_state",
        EVENT_CODE_SCENE_LAUNCHED => "scene_launched",
        EVENT_CODE_CHAT_MESSAGE => "chat_message",
        EVENT_CODE_COMBAT_CHANGED => "combat_changed",
        EVENT_CODE_TOKEN_DISCLOSURE_CHANGED => "token_disclosure_changed",
        EVENT_CODE_INTERACTIVE_CHANGED => "interactive_changed",
        EVENT_CODE_DOOR_CHANGED => "door_changed",
        EVENT_CODE_INTERACTION_REQUEST => "interaction_request",
        EVENT_CODE_WORLD_APPEARANCE_CHANGED => "world_appearance_changed",
        EVENT_CODE_WORLD_SYSTEM_CHANGED => "world_system_changed",
        EVENT_CODE_SCENE_LIGHTING_CHANGED => "scene_lighting_changed",
        EVENT_CODE_ACTOR_SHEET_CHANGED => "actor_sheet_changed",
        EVENT_CODE_SCENE_EXPLORATION_RESET => "scene_exploration_reset",
        EVENT_CODE_WORLD_PLAY_PAUSED => "world_play_paused",
        EVENT_CODE_ATTACK_MADE => "attack_made",
        EVENT_CODE_OFFER_CHANGED => "offer_changed",
        EVENT_CODE_ACTOR_ACCESS_CHANGED => "actor_access_changed",
        EVENT_CODE_TABLE_DIFFICULTY_CHANGED => "table_difficulty_changed",
        EVENT_CODE_SCENE_LEVEL_CHANGED => "scene_level_changed",
        EVENT_CODE_TOKEN_TRAVELLED => "token_travelled",
        EVENT_CODE_WORLD_SYSTEM_SETTING_CHANGED => "world_system_setting_changed",
        EVENT_CODE_ROLL_MADE => "roll_made",
        EVENT_CODE_ROLL_REVEALED => "roll_revealed",
        EVENT_CODE_AUTHORING_TOOLS_CHANGED => "authoring_tools_changed",
        EVENT_CODE_ROLLS_CLEARED => "rolls_cleared",
        _ => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = include_str!("../world_events.rs");

    /// Every `pub const EVENT_CODE_*` in `world_events.rs`: name and code.
    fn declared() -> Vec<(String, i32)> {
        SOURCE
            .lines()
            .filter_map(|l| l.trim().strip_prefix("pub const EVENT_CODE_"))
            .map(|rest| {
                let (name, value) = rest.split_once(": i32 =").expect("an i32 constant");
                let code = value
                    .trim()
                    .trim_end_matches(';')
                    .parse()
                    .expect("a literal");
                (name.to_ascii_lowercase(), code)
            })
            .collect()
    }

    #[test]
    fn every_event_code_has_its_own_name() {
        let codes = declared();
        assert_eq!(codes.len(), 30, "the contract covers 30 codes today");
        for (name, code) in codes {
            assert_eq!(event_name(code), name, "EVENT_CODE_{}", name.to_uppercase());
        }
    }

    #[test]
    fn an_unknown_code_is_unknown() {
        assert_eq!(event_name(-1), "unknown");
        assert_eq!(event_name(9_999), "unknown");
    }
}
