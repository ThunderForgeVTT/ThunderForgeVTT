use bevy::prelude::*;

/// Tokens currently being dragged, each with its grab-point offset (token
/// centre minus cursor world position at drag start) so nothing jumps to
/// re-centre under the cursor on the first move frame.
///
/// A list, not one token: clicking a stack picks up the whole stack, and
/// dragging it has to move every member by the same delta while preserving
/// their relative positions — which is exactly what per-token offsets do.
#[derive(Resource, Default)]
pub struct DraggingToken(pub Vec<(String, Vec2)>);

/// The current selection, topmost first.
///
/// Was a single `Option<String>`, which made stacked tokens unreachable:
/// the hit test took the first token it found and everything underneath
/// was unselectable without dragging the pile apart. A click now selects
/// the whole stack (`select_stack`), and a double-click offers a picker
/// that narrows it back to one (`select`).
///
/// The first element is the *primary*: the one whose properties panels
/// show, and the one reported to the world store's single-selection slot.
/// `get_selected` returns it, so every existing single-selection caller
/// keeps working unchanged.
#[derive(Resource, Default)]
pub struct SelectedToken(pub Vec<String>);

impl SelectedToken {
    /// Selects exactly one token, replacing any stack.
    pub fn select(&mut self, token_id: String) {
        self.0 = vec![token_id];
    }

    /// Selects a whole stack, topmost first.
    pub fn select_stack(&mut self, token_ids: Vec<String>) {
        self.0 = token_ids;
    }

    pub fn deselect(&mut self) {
        self.0.clear();
    }

    /// True for any member of the selection, not only the primary — this
    /// is what draws every token in a picked-up stack as selected.
    pub fn is_selected(&self, token_id: &str) -> bool {
        self.0.iter().any(|id| id == token_id)
    }

    /// The primary (topmost) selected token, if any.
    pub fn get_selected(&self) -> Option<&String> {
        self.0.first()
    }

    /// The whole selection, topmost first.
    pub fn selected_ids(&self) -> &[String] {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select_token() {
        let mut selected = SelectedToken::default();
        assert!(selected.get_selected().is_none());

        selected.select("token_1".to_string());
        assert_eq!(selected.get_selected(), Some(&"token_1".to_string()));
    }

    #[test]
    fn test_is_selected() {
        let selected = SelectedToken(vec!["token_1".to_string()]);
        assert!(selected.is_selected("token_1"));
        assert!(!selected.is_selected("token_2"));
    }

    #[test]
    fn test_deselect_token() {
        let mut selected = SelectedToken(vec!["token_1".to_string()]);
        assert!(selected.is_selected("token_1"));

        selected.deselect();
        assert!(!selected.is_selected("token_1"));
        assert!(selected.get_selected().is_none());
    }
}
