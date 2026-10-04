use super::*;
use crate::graphql::input_types::GraphQLCreateWorldInput;

fn input(game_system_id: Option<&str>) -> GraphQLCreateWorldInput {
    GraphQLCreateWorldInput {
        name: "A World".to_string(),
        description: None,
        game_system_id: game_system_id.map(str::to_string),
        interface_pack_id: None,
    }
}

/// The operator's configured default applies when the caller says nothing.
#[test]
fn an_unspecified_system_takes_the_configured_default() {
    let prepared = prepare_world_input(input(None), Some("some-system")).expect("valid");
    assert_eq!(prepared.game_system_id.as_deref(), Some("some-system"));
}

/// And never overrides one the caller did specify.
#[test]
fn a_specified_system_is_never_replaced_by_the_default() {
    let prepared = prepare_world_input(input(Some("chosen")), Some("configured")).expect("valid");
    assert_eq!(prepared.game_system_id.as_deref(), Some("chosen"));
}

/// No default configured is a real answer. A world with no system is a
/// state this product handles everywhere; inventing one would bind a world
/// to a ruleset nobody picked.
#[test]
fn no_configured_default_leaves_the_world_systemless() {
    let prepared = prepare_world_input(input(None), None).expect("valid");
    assert_eq!(prepared.game_system_id, None);
}
