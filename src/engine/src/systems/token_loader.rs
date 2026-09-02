use crate::components::Token;
use crate::resources::SceneData;
use bevy::prelude::*;

#[derive(Resource)]
pub struct TokenCache(pub Vec<Token>);

pub fn load_test_tokens(scene: &SceneData) -> Vec<Token> {
    vec![
        Token {
            id: "token_1".to_string(),
            world_id: "world_1".to_string(),
            scene_id: scene.scene_id.clone(),
            token_type: "character".to_string(),
            label: Some("Alice".to_string()),
            base_x: 2,
            base_y: 3,
            size_x: 1,
            size_y: 1,
            color: Color::srgb(0.2, 0.8, 0.2),
            is_visible: true,
            health: Some(20),
            max_health: Some(20),
            attributes: Default::default(),
            schema_version: 1,
            is_selected: false,
            is_hovered: false,
        },
        Token {
            id: "token_2".to_string(),
            world_id: "world_1".to_string(),
            scene_id: scene.scene_id.clone(),
            token_type: "npc".to_string(),
            label: Some("Goblin 1".to_string()),
            base_x: 5,
            base_y: 4,
            size_x: 1,
            size_y: 1,
            color: Color::srgb(0.8, 0.2, 0.2),
            is_visible: true,
            health: Some(8),
            max_health: Some(8),
            attributes: Default::default(),
            schema_version: 1,
            is_selected: false,
            is_hovered: false,
        },
        Token {
            id: "token_3".to_string(),
            world_id: "world_1".to_string(),
            scene_id: scene.scene_id.clone(),
            token_type: "object".to_string(),
            label: Some("Chest".to_string()),
            base_x: 8,
            base_y: 6,
            size_x: 2,
            size_y: 2,
            color: Color::srgb(0.8, 0.8, 0.2),
            is_visible: true,
            health: None,
            max_health: None,
            attributes: Default::default(),
            schema_version: 1,
            is_selected: false,
            is_hovered: false,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::GridType;

    #[test]
    fn test_load_test_tokens() {
        let scene = SceneData::new(
            "test".to_string(),
            "test".to_string(),
            GridType::Square,
            32.0,
            10,
            10,
            None,
        );

        let tokens = load_test_tokens(&scene);
        assert_eq!(tokens.len(), 3, "Should have 3 test tokens");

        assert_eq!(tokens[0].label, Some("Alice".to_string()));
        assert_eq!(tokens[0].base_x, 2);
        assert_eq!(tokens[0].base_y, 3);
        assert_eq!(tokens[0].size_x, 1);
        assert_eq!(tokens[0].size_y, 1);
    }

    #[test]
    fn test_token_positions_within_bounds() {
        let scene = SceneData::new(
            "test".to_string(),
            "test".to_string(),
            GridType::Square,
            32.0,
            10,
            10,
            None,
        );

        let tokens = load_test_tokens(&scene);
        for token in &tokens {
            assert!(token.base_x < scene.width);
            assert!(token.base_y < scene.height);
        }
    }

    #[test]
    fn test_token_colors_valid() {
        let scene = SceneData::new(
            "test".to_string(),
            "test".to_string(),
            GridType::Square,
            32.0,
            10,
            10,
            None,
        );

        let tokens = load_test_tokens(&scene);
        for token in &tokens {
            // Just verify color is created without panic
            let _ = token.color;
        }
    }

    #[test]
    fn test_token_types_recognized() {
        let scene = SceneData::new(
            "test".to_string(),
            "test".to_string(),
            GridType::Square,
            32.0,
            10,
            10,
            None,
        );

        let tokens = load_test_tokens(&scene);
        let valid_types = ["character", "npc", "object"];
        for token in &tokens {
            assert!(
                valid_types.contains(&token.token_type.as_str()),
                "Token type must be character, npc, or object"
            );
        }
    }
}
