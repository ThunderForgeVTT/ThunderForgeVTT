//! The snapping switch's key, and the report of its state (spec 077).
//!
//! `GridSnapEnabled` has had a handle from the page since spec 001
//! (`set_grid_snap`); this gives it one on the keyboard. `S` toggles it while
//! an authoring tool is open and is left alone under the select tool, where
//! WASD walks the selected token (`120284e1`): one key, two meanings, told
//! apart by the mode the engine already owns (FR-002).
//!
//! Whatever flipped the switch — the key, the page, a test — the page hears
//! about it the same way, by change detection on the resource, so the rail's
//! button and the engine never disagree (SC-003).

use bevy::prelude::*;
use serde_json::json;

use crate::emit_event;
use crate::plugins::authoring_mode::AuthoringMode;
use crate::resources::{GridSnapEnabled, IsGameMaster};

/// `S` toggles snapping for the whole board, under any authoring tool.
///
/// Gated to Game Masters: a player has no authoring tool open, and the switch
/// changes only what authoring places.
///
/// The mode is optional so a build that adds `WallPlugin` without the
/// authoring-mode plugin still runs; with no mode there is no authoring tool
/// open, and the key does nothing.
pub(crate) fn toggle_grid_snap_on_s(
    keyboard: Res<ButtonInput<KeyCode>>,
    mode: Option<Res<State<AuthoringMode>>>,
    is_gm: Res<IsGameMaster>,
    mut snap: ResMut<GridSnapEnabled>,
) {
    if !is_gm.0 || mode.as_deref().is_none_or(s_walks_token) {
        return;
    }
    if keyboard.just_pressed(KeyCode::KeyS) {
        snap.0 = !snap.0;
    }
}

/// Tell the page when the switch moves, however it moved. Fires once on the
/// first frame too, so the page learns the starting state without asking.
pub(crate) fn report_grid_snap(snap: Res<GridSnapEnabled>) {
    if snap.is_changed() {
        emit_event(json!({
            "type": "grid_snap_changed",
            "enabled": snap.0,
        }));
    }
}

/// Whether `S` should walk a token right now: only under the select tool.
/// `handle_token_movement_input` asks this so the two meanings never land
/// on the same press.
pub(crate) fn s_walks_token(mode: &State<AuthoringMode>) -> bool {
    *mode.get() == AuthoringMode::Select
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(mode: AuthoringMode) -> App {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_resource::<ButtonInput<KeyCode>>()
            .insert_resource(IsGameMaster(true))
            .init_resource::<GridSnapEnabled>()
            .insert_state(mode)
            .add_systems(Update, toggle_grid_snap_on_s);
        app
    }

    fn press_s(app: &mut App) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyS);
        app.update();
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.clear_just_pressed(KeyCode::KeyS);
        keys.release(KeyCode::KeyS);
    }

    #[test]
    fn s_toggles_snapping_under_an_authoring_tool() {
        let mut app = app(AuthoringMode::Walls);
        assert!(app.world().resource::<GridSnapEnabled>().0);
        press_s(&mut app);
        assert!(!app.world().resource::<GridSnapEnabled>().0);
        press_s(&mut app);
        assert!(app.world().resource::<GridSnapEnabled>().0);
    }

    #[test]
    fn s_leaves_snapping_alone_under_the_select_tool() {
        let mut app = app(AuthoringMode::Select);
        press_s(&mut app);
        assert!(app.world().resource::<GridSnapEnabled>().0);
        assert!(s_walks_token(
            app.world().resource::<State<AuthoringMode>>()
        ));
    }

    #[test]
    fn a_player_cannot_toggle_it() {
        let mut app = app(AuthoringMode::Lights);
        app.insert_resource(IsGameMaster(false));
        press_s(&mut app);
        assert!(app.world().resource::<GridSnapEnabled>().0);
    }
}
