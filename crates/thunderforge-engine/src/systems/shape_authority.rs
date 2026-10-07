//! Who may change a drawing, as this board knows it (spec 082 R7).
//!
//! The server decides (`auth/shape_authority.rs`); this is the same rule
//! asked early, so a player's board never offers a handle on a drawing the
//! server would refuse. A Game Master edits any drawing; anybody else edits
//! only the drawings they made.

use thunderforge_canvas_core::shape::Shape;

use crate::resources::{IsGameMaster, ViewerUserId};

/// Whether this viewer may move, restyle or delete `shape`.
///
/// A drawing with no creator (one made before spec 082, or whose author has
/// left) is the Game Master's alone, and so is everything when the board does
/// not know who is looking.
pub fn may_edit_shape(is_gm: bool, viewer: Option<&str>, shape: &Shape) -> bool {
    is_gm
        || matches!(
            (viewer, shape.created_by.as_deref()),
            (Some(viewer), Some(creator)) if viewer == creator
        )
}

/// [`may_edit_shape`] for the viewer as the shape systems hold it. The
/// viewer resource is optional: a board never told who is looking edits a
/// drawing only if it is a Game Master's board.
pub(crate) fn viewer_may_edit(
    is_gm: &IsGameMaster,
    viewer: Option<&ViewerUserId>,
    shape: &Shape,
) -> bool {
    may_edit_shape(is_gm.0, viewer.and_then(|v| v.0.as_deref()), shape)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::authoring_mode::{AuthoringMode, ToolBoundary};
    use thunderforge_canvas_core::shape::ShapeKind;

    fn drawn_by(creator: Option<&str>) -> Shape {
        Shape {
            id: "s1".into(),
            kind: ShapeKind::Rect,
            geometry: serde_json::json!({ "x": 0, "y": 0, "w": 10, "h": 10 }),
            text: None,
            style: None,
            visible_to_players: true,
            created_by: creator.map(str::to_string),
        }
    }

    #[test]
    fn a_game_master_edits_any_drawing() {
        assert!(may_edit_shape(true, Some("gm"), &drawn_by(Some("a"))));
        assert!(may_edit_shape(true, None, &drawn_by(None)));
    }

    #[test]
    fn the_creator_edits_their_own() {
        assert!(may_edit_shape(false, Some("a"), &drawn_by(Some("a"))));
    }

    #[test]
    fn another_player_does_not() {
        assert!(!may_edit_shape(false, Some("b"), &drawn_by(Some("a"))));
    }

    #[test]
    fn an_unknown_viewer_edits_nothing() {
        assert!(!may_edit_shape(false, None, &drawn_by(Some("a"))));
    }

    #[test]
    fn a_drawing_with_no_creator_is_the_game_masters() {
        assert!(!may_edit_shape(false, Some("a"), &drawn_by(None)));
    }

    /// A player whose Game Master took the Shapes tool away cannot arm it.
    #[test]
    fn a_viewer_without_shapes_cannot_enter_the_shapes_tool() {
        let boundary = ToolBoundary::new();
        boundary.set_allowed("select");
        assert!(!boundary.is_allowed(AuthoringMode::Shapes));
        assert!(!boundary.request("shapes"));
        boundary.set_allowed("select,shapes");
        assert!(boundary.is_allowed(AuthoringMode::Shapes));
    }
}
