use bevy::prelude::*;

/// Token currently being dragged by the mouse, with the grab-point offset
/// (token center minus cursor world position at drag start) so the token
/// doesn't jump to re-center under the cursor on the first move frame.
#[derive(Resource, Default)]
pub struct DraggingToken(pub Option<(String, Vec2)>);

/// Currently selected token ID
/// Only one token can be selected at a time (Phase 4.7.E1)
/// Deferred: Multi-select (Shift+click) to Phase 4.8
#[derive(Resource, Default)]
pub struct SelectedToken(pub Option<String>);

impl SelectedToken {
    pub fn select(&mut self, token_id: String) {
        self.0 = Some(token_id);
    }

    pub fn deselect(&mut self) {
        self.0 = None;
    }

    pub fn is_selected(&self, token_id: &str) -> bool {
        self.0.as_ref().is_some_and(|id| id == token_id)
    }

    pub fn get_selected(&self) -> Option<&String> {
        self.0.as_ref()
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
        let selected = SelectedToken(Some("token_1".to_string()));
        assert!(selected.is_selected("token_1"));
        assert!(!selected.is_selected("token_2"));
    }

    #[test]
    fn test_deselect_token() {
        let mut selected = SelectedToken(Some("token_1".to_string()));
        assert!(selected.is_selected("token_1"));

        selected.deselect();
        assert!(!selected.is_selected("token_1"));
        assert!(selected.get_selected().is_none());
    }
}
