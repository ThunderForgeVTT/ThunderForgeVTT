//! Who may see which scenes, and the canvas assets attached to them.
//!
//! # The rule, stated once
//!
//! - Somebody who **runs the world** (Owner, Game Master, or a site admin)
//!   sees every scene, hidden or not.
//! - Anyone else sees scenes with `hidden = false`, **and the one scene the
//!   world is currently playing**, hidden or not.
//! - `hidden` defaults to **true**: a freshly created scene is invisible to
//!   players until the GM reveals it, or launches it.
//! - A **canvas asset** inherits the visibility of the scene it is attached
//!   to. An asset attached to no scene belongs to the world rather than to
//!   any scene, and is visible to every member.
//!
//! # Why the scene being played is not hidden from the people playing it
//!
//! `hidden` keeps a GM's unfinished prep out of the players' Scenes table.
//! Launching a scene is the opposite act — the GM deliberately putting it in
//! front of everyone — and a world's auto-created scene is hidden by default.
//! Without the carve-out the ordinary case was a player sitting at a table
//! whose map they were not allowed to know anything about: `world_sync_plan`
//! returned an **empty plan** to every player in every world whose scene had
//! never been un-hidden. No scene state, no assets, nothing cached, in a
//! feature whose whole promise is that the world is already on the device.
//!
//! It is one scene per world, chosen by the GM. Guessing a *different* hidden
//! scene's id still answers no.
//!
//! # Why this is a predicate and the server still has SQL
//!
//! The rule has two shapes at the call sites. A sync plan classifies every
//! asset in a world and wants the visible set in one round trip; the byte
//! route holds one asset and wants one answer. Making the plan call a
//! per-row predicate would cost it an N+1, so it stays a SQL filter — but the
//! filter is the bulk form of *this* function, and the truth table below is
//! what says what that filter has to mean.
//!
//! This distinction had already cost something. The two call sites answered
//! differently: the plan narrowed a world's assets to visible scenes, so a
//! hidden scene's art never appeared in a player's plan — while
//! `GET /canvas-assets/{id}` authorized on world membership alone, so the
//! same player could fetch those exact bytes by asking for the id directly.
//! The plan was the only thing enforcing a rule the bytes did not have.

/// What the visibility decision needs to know about one scene.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Scene {
    /// The GM-only flag. Defaults to true in the database.
    pub hidden: bool,
    /// Whether this is the scene the world is currently playing.
    pub is_world_active_scene: bool,
}

impl Scene {
    /// A scene the GM has revealed.
    pub fn revealed() -> Self {
        Self {
            hidden: false,
            is_world_active_scene: false,
        }
    }

    /// A scene the GM is still preparing.
    pub fn hidden() -> Self {
        Self {
            hidden: true,
            is_world_active_scene: false,
        }
    }

    /// The scene the world is currently playing.
    pub fn active(hidden: bool) -> Self {
        Self {
            hidden,
            is_world_active_scene: true,
        }
    }
}

/// Whether this caller may see this scene.
///
/// `runs_the_world` is the caller's already-resolved authority — Owner or
/// Game Master here, or a site admin. It is a parameter rather than something
/// derived here because callers have just established it and re-deriving it
/// would take a second query to answer a question already answered.
pub fn scene_visible(runs_the_world: bool, scene: Scene) -> bool {
    runs_the_world || !scene.hidden || scene.is_world_active_scene
}

/// Whether this caller may see a canvas asset.
///
/// `attached_scene` is `None` when the asset belongs to the world rather than
/// to any scene, which every member may see.
pub fn asset_visible(runs_the_world: bool, attached_scene: Option<Scene>) -> bool {
    match attached_scene {
        None => true,
        Some(scene) => scene_visible(runs_the_world, scene),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// All eight combinations, written out.
    ///
    /// The SQL in `scene_visibility.rs` is the bulk form of this table; if the
    /// two ever disagree, this is the statement of which one is right.
    #[test]
    fn the_visibility_truth_table_is_stated_in_full() {
        // (runs_the_world, hidden, is_active, visible)
        let table = [
            (true, false, false, true),
            (true, true, false, true),
            (true, false, true, true),
            (true, true, true, true),
            (false, false, false, true),
            // The prep a GM has not revealed.
            (false, true, false, false),
            (false, false, true, true),
            // The carve-out: hidden, but it is the scene being played.
            (false, true, true, true),
        ];

        for (runs, hidden, is_active, expected) in table {
            let scene = Scene {
                hidden,
                is_world_active_scene: is_active,
            };
            assert_eq!(
                scene_visible(runs, scene),
                expected,
                "runs_the_world={runs} hidden={hidden} active={is_active}"
            );
        }

        assert_eq!(table.len(), 8, "every combination must be stated");
    }

    /// The bug this carve-out exists for, as its own named test.
    #[test]
    fn a_player_sees_the_hidden_scene_their_table_is_actually_playing() {
        assert!(
            scene_visible(false, Scene::active(true)),
            "a world's auto-created scene is hidden by default; without this \
             every player got an empty sync plan"
        );
    }

    #[test]
    fn a_player_does_not_see_a_different_hidden_scene() {
        assert!(
            !scene_visible(false, Scene::hidden()),
            "guessing another hidden scene's id must still answer no"
        );
    }

    #[test]
    fn a_game_master_sees_their_own_unrevealed_prep() {
        assert!(scene_visible(true, Scene::hidden()));
    }

    #[test]
    fn a_world_level_asset_belongs_to_everyone_in_the_world() {
        assert!(asset_visible(false, None));
        assert!(asset_visible(true, None));
    }

    /// The asymmetry that let a player fetch a hidden scene's art directly.
    #[test]
    fn an_asset_is_exactly_as_visible_as_the_scene_it_hangs_on() {
        for hidden in [true, false] {
            for is_active in [true, false] {
                for runs in [true, false] {
                    let scene = Scene {
                        hidden,
                        is_world_active_scene: is_active,
                    };
                    assert_eq!(
                        asset_visible(runs, Some(scene)),
                        scene_visible(runs, scene),
                        "an asset must never be reachable when its scene is not"
                    );
                }
            }
        }
    }
}
