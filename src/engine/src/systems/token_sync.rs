use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use crate::components::Token;
use crate::resources::SceneData;
use crate::transforms::coordinate::grid_to_pixel;
use crate::resources::CameraManager;
use std::sync::OnceLock;
use std::sync::Mutex;

/// Token data received from RxDB (JavaScript)
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TokenJson {
    pub id: String,
    pub world_id: String,
    pub scene_id: String,
    pub token_type: String,
    pub label: Option<String>,
    pub base_x: i32,
    pub base_y: i32,
    pub size_x: i32,
    pub size_y: i32,
    pub color: ColorJson,
    pub is_visible: bool,
}

/// RGB color from RxDB
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ColorJson {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

/// Thread-local slot for pending tokens JSON from WASM
static PENDING_TOKENS_JSON: OnceLock<Mutex<String>> = OnceLock::new();

fn pending_tokens_slot() -> &'static Mutex<String> {
    PENDING_TOKENS_JSON.get_or_init(|| Mutex::new(String::new()))
}

/// Public function for WASM export to call
pub fn set_pending_tokens(tokens_json: &str) {
    if let Ok(mut slot) = pending_tokens_slot().lock() {
        *slot = tokens_json.to_string();
    }
}

/// Sync tokens from RxDB into Bevy world
pub fn sync_tokens_from_rxdb(
    mut commands: Commands,
    query: Query<Entity, With<Token>>,
    scene: Res<SceneData>,
    camera_mgr: Res<CameraManager>,
) {
    // Check if new tokens are available
    let tokens_json = if let Ok(mut slot) = pending_tokens_slot().lock() {
        let json = slot.clone();
        slot.clear();
        json
    } else {
        return;
    };

    if tokens_json.is_empty() {
        return;
    }

    // Parse token JSON
    let tokens: Vec<TokenJson> = match serde_json::from_str(&tokens_json) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Failed to parse tokens JSON: {}", e);
            return;
        }
    };

    // Despawn all existing token entities
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }

    // Spawn new tokens from RxDB
    for token_json in tokens {
        let pixel_pos = grid_to_pixel(
            token_json.base_x as f32,
            token_json.base_y as f32,
            &scene,
            camera_mgr.translation,
            camera_mgr.scale,
        );

        let color = Color::srgb(
            token_json.color.r.clamp(0.0, 1.0),
            token_json.color.g.clamp(0.0, 1.0),
            token_json.color.b.clamp(0.0, 1.0),
        );

        let token = Token {
            id: token_json.id.clone(),
            world_id: token_json.world_id,
            scene_id: token_json.scene_id,
            token_type: token_json.token_type,
            label: token_json.label,
            base_x: token_json.base_x,
            base_y: token_json.base_y,
            size_x: token_json.size_x,
            size_y: token_json.size_y,
            color,
            is_visible: token_json.is_visible,
            health: None,
            max_health: None,
            abilities: Default::default(),
            schema_version: 1,
            is_selected: false,
            is_hovered: false,
        };

        let sprite_size = Vec2::new(
            (token_json.size_x as f32) * scene.grid_size,
            (token_json.size_y as f32) * scene.grid_size,
        );

        commands.spawn((
            Sprite {
                color: token.color,
                custom_size: Some(sprite_size),
                ..default()
            },
            Transform::default()
                .with_translation(pixel_pos.extend(1.0)),
            token,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_json_deserialization() {
        let json = r#"{
            "id": "token_1",
            "world_id": "world_1",
            "scene_id": "scene_1",
            "token_type": "character",
            "label": "Alice",
            "base_x": 5,
            "base_y": 5,
            "size_x": 1,
            "size_y": 1,
            "color": {"r": 0.2, "g": 0.8, "b": 0.2},
            "is_visible": true
        }"#;

        let token: TokenJson = serde_json::from_str(json).unwrap();
        assert_eq!(token.id, "token_1");
        assert_eq!(token.label, Some("Alice".to_string()));
        assert_eq!(token.base_x, 5);
    }

    #[test]
    fn test_color_json_bounds() {
        let color = ColorJson {
            r: 1.5,
            g: -0.5,
            b: 0.5,
        };

        let clamped_r = color.r.clamp(0.0, 1.0);
        let clamped_g = color.g.clamp(0.0, 1.0);
        let clamped_b = color.b.clamp(0.0, 1.0);

        assert_eq!(clamped_r, 1.0);
        assert_eq!(clamped_g, 0.0);
        assert_eq!(clamped_b, 0.5);
    }
}
