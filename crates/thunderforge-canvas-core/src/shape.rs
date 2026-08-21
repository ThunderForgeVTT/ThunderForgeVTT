use serde_json::Value;

/// The five drawing-tool kinds tldraw previously provided (FR-007,
/// data-model.md's Shape section), mirroring `wall.rs`'s `DoorState`
/// string-conversion pattern for the server's `kind` text column /
/// GraphQL `ShapeKind` enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShapeKind {
    #[default]
    Stroke,
    Rect,
    Ellipse,
    Line,
    Text,
}

impl ShapeKind {
    pub fn from_str_loose(value: &str) -> Self {
        match value {
            "rect" => ShapeKind::Rect,
            "ellipse" => ShapeKind::Ellipse,
            "line" => ShapeKind::Line,
            "text" => ShapeKind::Text,
            _ => ShapeKind::Stroke,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            ShapeKind::Stroke => "stroke",
            ShapeKind::Rect => "rect",
            ShapeKind::Ellipse => "ellipse",
            ShapeKind::Line => "line",
            ShapeKind::Text => "text",
        }
    }
}

/// A single shape/annotation (data-model.md's Shape section). `id` is
/// `None`-equivalent (empty string, same server-confirms-real-id
/// convention as `Wall`) for a locally-drawn-but-not-yet-server-confirmed
/// shape — the server assigns the real UUID on create, so newly drawn
/// shapes stay untracked locally until the matching `upsert_shape`
/// command arrives, same as walls (see `systems/wall.rs`'s module doc).
///
/// `geometry`/`style` are opaque `serde_json::Value` blobs, matching the
/// server's contract (contracts/graphql.md): this crate never interprets
/// their contents, only carries them.
#[derive(Debug, Clone, PartialEq)]
pub struct Shape {
    pub id: String,
    pub kind: ShapeKind,
    pub geometry: Value,
    pub text: Option<String>,
    pub style: Option<Value>,
    pub visible_to_players: bool,
}

/// One reversible shape edit, pushed onto `ShapeSet`'s undo stack whenever
/// a confirmed edit is applied locally. Undo re-issues the inverse as a
/// normal outbound mutation (research.md §4), mirroring `WallEdit`.
#[derive(Debug, Clone)]
pub enum ShapeEdit {
    /// A shape was moved/resized (its `geometry` blob changed). Undo
    /// re-issues `update_shape` with the prior geometry.
    Move {
        shape_id: String,
        prior_geometry: Value,
    },
    /// A shape's style (e.g. color) was changed. Undo re-issues
    /// `update_shape` with the prior style.
    Restyle {
        shape_id: String,
        prior_style: Option<Value>,
    },
    /// A shape's GM-only/visible-to-players flag was toggled. Undo
    /// re-issues `update_shape` with the prior flag.
    VisibilityToggle {
        shape_id: String,
        prior_visible_to_players: bool,
    },
    /// A shape was deleted. Undo re-issues `create_shape` with the
    /// shape's full prior state (note: the re-created shape gets a *new*
    /// server-assigned id, it cannot resurrect the original id).
    Delete { deleted: Shape },
}

const MAX_UNDO_STACK: usize = 50;

/// Shape list plus a bounded per-session undo stack (research.md §4),
/// mirroring `WallSet`. Plain data, no Bevy `Resource` derive —
/// `thunderforge_engine` wraps this in a `Resource` newtype
/// (`src/engine/src/resources/shape.rs`) so Bevy's change-detection still
/// works transparently on `ResMut` access.
#[derive(Debug, Clone, Default)]
pub struct ShapeSet {
    shapes: Vec<Shape>,
    undo_stack: Vec<ShapeEdit>,
    /// Set whenever the shape list changes, so render-sync systems can
    /// react to `ShapeSet` changes — cleared by the system that consumes
    /// it (mirrors `WallSet::dirty`).
    pub dirty: bool,
}

impl ShapeSet {
    pub fn shapes(&self) -> &[Shape] {
        &self.shapes
    }

    pub fn get(&self, id: &str) -> Option<&Shape> {
        self.shapes.iter().find(|s| s.id == id)
    }

    fn index_of(&self, id: &str) -> Option<usize> {
        self.shapes.iter().position(|s| s.id == id)
    }

    /// Insert-or-update by id, mirroring `WallSet::upsert`.
    pub fn upsert(&mut self, shape: Shape) {
        if let Some(index) = self.index_of(&shape.id) {
            self.shapes[index] = shape;
        } else {
            self.shapes.push(shape);
        }
        self.dirty = true;
    }

    /// Removes and returns the shape with the given id, if present.
    pub fn remove(&mut self, id: &str) -> Option<Shape> {
        let index = self.index_of(id)?;
        self.dirty = true;
        Some(self.shapes.remove(index))
    }

    pub fn push_undo(&mut self, edit: ShapeEdit) {
        self.undo_stack.push(edit);
        if self.undo_stack.len() > MAX_UNDO_STACK {
            self.undo_stack.remove(0);
        }
    }

    pub fn pop_undo(&mut self) -> Option<ShapeEdit> {
        self.undo_stack.pop()
    }

    pub fn undo_stack_len(&self) -> usize {
        self.undo_stack.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn shape(id: &str, kind: ShapeKind) -> Shape {
        Shape {
            id: id.to_string(),
            kind,
            geometry: json!({ "x": 0.0, "y": 0.0, "w": 10.0, "h": 10.0 }),
            text: None,
            style: None,
            visible_to_players: false,
        }
    }

    #[test]
    fn shape_kind_round_trips() {
        assert_eq!(ShapeKind::from_str_loose("stroke"), ShapeKind::Stroke);
        assert_eq!(ShapeKind::from_str_loose("rect"), ShapeKind::Rect);
        assert_eq!(ShapeKind::from_str_loose("ellipse"), ShapeKind::Ellipse);
        assert_eq!(ShapeKind::from_str_loose("line"), ShapeKind::Line);
        assert_eq!(ShapeKind::from_str_loose("text"), ShapeKind::Text);
        assert_eq!(ShapeKind::from_str_loose("garbage"), ShapeKind::Stroke);

        assert_eq!(ShapeKind::Stroke.as_str(), "stroke");
        assert_eq!(ShapeKind::Rect.as_str(), "rect");
        assert_eq!(ShapeKind::Ellipse.as_str(), "ellipse");
        assert_eq!(ShapeKind::Line.as_str(), "line");
        assert_eq!(ShapeKind::Text.as_str(), "text");
    }

    #[test]
    fn upsert_inserts_then_updates_by_id() {
        let mut shapes = ShapeSet::default();
        shapes.upsert(shape("s1", ShapeKind::Rect));
        assert_eq!(shapes.shapes().len(), 1);

        let mut updated = shape("s1", ShapeKind::Rect);
        updated.geometry = json!({ "x": 5.0, "y": 5.0, "w": 20.0, "h": 20.0 });
        shapes.upsert(updated);

        assert_eq!(shapes.shapes().len(), 1);
        assert_eq!(shapes.get("s1").unwrap().geometry["x"], 5.0);
    }

    #[test]
    fn remove_returns_removed_shape() {
        let mut shapes = ShapeSet::default();
        shapes.upsert(shape("s1", ShapeKind::Stroke));

        let removed = shapes.remove("s1");
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().id, "s1");
        assert!(shapes.get("s1").is_none());
    }

    #[test]
    fn remove_missing_returns_none() {
        let mut shapes = ShapeSet::default();
        assert!(shapes.remove("nope").is_none());
    }

    #[test]
    fn dirty_flag_set_on_upsert_and_remove() {
        let mut shapes = ShapeSet::default();
        assert!(!shapes.dirty);
        shapes.upsert(shape("s1", ShapeKind::Text));
        assert!(shapes.dirty);

        shapes.dirty = false;
        shapes.remove("s1");
        assert!(shapes.dirty);
    }

    #[test]
    fn undo_stack_is_bounded() {
        let mut shapes = ShapeSet::default();
        for i in 0..(MAX_UNDO_STACK + 10) {
            shapes.push_undo(ShapeEdit::VisibilityToggle {
                shape_id: format!("s{i}"),
                prior_visible_to_players: false,
            });
        }
        assert_eq!(shapes.undo_stack_len(), MAX_UNDO_STACK);
    }

    #[test]
    fn undo_stack_pops_most_recent_first() {
        let mut shapes = ShapeSet::default();
        shapes.push_undo(ShapeEdit::VisibilityToggle {
            shape_id: "first".to_string(),
            prior_visible_to_players: false,
        });
        shapes.push_undo(ShapeEdit::VisibilityToggle {
            shape_id: "second".to_string(),
            prior_visible_to_players: false,
        });

        match shapes.pop_undo() {
            Some(ShapeEdit::VisibilityToggle { shape_id, .. }) => {
                assert_eq!(shape_id, "second")
            }
            _ => panic!("expected VisibilityToggle edit"),
        }
    }

    #[test]
    fn text_shape_carries_label() {
        let mut s = shape("s1", ShapeKind::Text);
        s.geometry = json!({ "x": 1.0, "y": 2.0 });
        s.text = Some("hello".to_string());
        assert_eq!(s.text.as_deref(), Some("hello"));
    }
}
