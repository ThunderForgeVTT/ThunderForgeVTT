use bevy::prelude::*;
use crate::resources::SceneData;

/// Phase 4.7.A2: Coordinate transformation system
///
/// Converts between:
/// - Grid coordinates (database): logical game board position
/// - Pixel coordinates (Bevy): screen/canvas pixels
///
/// Accounts for:
/// - Y-axis inversion (database Y-down → Bevy Y-up)
/// - Camera position (pan/translate)
/// - Camera zoom (scale)
/// - Grid size (pixels per cell)
#[derive(Component, Clone, Copy, Debug)]
pub struct CoordinateTransform;

/// Transform grid coordinates to pixel coordinates
pub fn grid_to_pixel(
    grid_x: f32,
    grid_y: f32,
    scene: &SceneData,
    camera_translation: Vec2,
    camera_scale: f32,
) -> Vec2 {
    // 1. Convert grid coordinates to pixel space (no camera yet)
    let pixel_x_base = grid_x * scene.grid_size;
    let pixel_y_base = scene.database_y_to_bevy_y(grid_y);

    // 2. Apply camera translation (pan) and scale (zoom)
    let pixel_x = (pixel_x_base - camera_translation.x) / camera_scale;
    let pixel_y = (pixel_y_base - camera_translation.y) / camera_scale;

    Vec2::new(pixel_x, pixel_y)
}

/// Transform pixel coordinates to grid coordinates
pub fn pixel_to_grid(
    pixel_x: f32,
    pixel_y: f32,
    scene: &SceneData,
    camera_translation: Vec2,
    camera_scale: f32,
) -> (f32, f32) {
    // 1. Reverse camera transform (zoom + pan)
    let pixel_x_base = pixel_x * camera_scale + camera_translation.x;
    let pixel_y_base = pixel_y * camera_scale + camera_translation.y;

    // 2. Convert pixel space to grid coordinates
    let grid_x = pixel_x_base / scene.grid_size;
    let grid_y = scene.bevy_y_to_database_y(pixel_y_base);

    (grid_x, grid_y)
}

/// Snap grid coordinates to nearest cell center
pub fn snap_to_grid(grid_x: f32, grid_y: f32) -> (i32, i32) {
    (grid_x.round() as i32, grid_y.round() as i32)
}

/// Check if pixel position is within scene bounds
pub fn is_within_bounds(pixel_x: f32, pixel_y: f32, scene: &SceneData) -> bool {
    let (x, y, w, h) = scene.bounds();
    pixel_x >= x && pixel_x <= w && pixel_y >= y && pixel_y <= h
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::GridType;

    #[test]
    fn test_grid_to_pixel_no_camera() {
        let scene = SceneData::new(
            "test".to_string(),
            "test".to_string(),
            GridType::Square,
            32.0,
            10,
            10,
            None,
        );

        let pixel = grid_to_pixel(0.0, 0.0, &scene, Vec2::ZERO, 1.0);
        assert_eq!(pixel.x, 0.0);
        assert_eq!(pixel.y, 320.0);

        let pixel = grid_to_pixel(1.0, 1.0, &scene, Vec2::ZERO, 1.0);
        assert_eq!(pixel.x, 32.0);
        assert_eq!(pixel.y, 288.0);
    }

    #[test]
    fn test_pixel_to_grid_no_camera() {
        let scene = SceneData::new(
            "test".to_string(),
            "test".to_string(),
            GridType::Square,
            32.0,
            10,
            10,
            None,
        );

        let (grid_x, grid_y) = pixel_to_grid(0.0, 320.0, &scene, Vec2::ZERO, 1.0);
        assert_eq!(grid_x, 0.0);
        assert_eq!(grid_y, 0.0);

        let (grid_x, grid_y) = pixel_to_grid(32.0, 288.0, &scene, Vec2::ZERO, 1.0);
        assert_eq!(grid_x, 1.0);
        assert_eq!(grid_y, 1.0);
    }

    #[test]
    fn test_bidirectional_consistency() {
        let scene = SceneData::new(
            "test".to_string(),
            "test".to_string(),
            GridType::Square,
            32.0,
            100,
            100,
            None,
        );

        let original_grid = (5.5, 3.2);
        let pixel = grid_to_pixel(original_grid.0, original_grid.1, &scene, Vec2::ZERO, 1.0);
        let converted_grid = pixel_to_grid(pixel.x, pixel.y, &scene, Vec2::ZERO, 1.0);

        assert!((converted_grid.0 - original_grid.0).abs() < 0.01);
        assert!((converted_grid.1 - original_grid.1).abs() < 0.01);
    }

    #[test]
    fn test_camera_pan() {
        let scene = SceneData::new(
            "test".to_string(),
            "test".to_string(),
            GridType::Square,
            32.0,
            10,
            10,
            None,
        );

        let pixel_no_cam = grid_to_pixel(0.0, 0.0, &scene, Vec2::ZERO, 1.0);
        let pixel_with_cam = grid_to_pixel(0.0, 0.0, &scene, Vec2::new(32.0, 32.0), 1.0);

        assert_eq!(pixel_no_cam.x - pixel_with_cam.x, 32.0);
        assert_eq!(pixel_no_cam.y - pixel_with_cam.y, 32.0);
    }

    #[test]
    fn test_camera_zoom() {
        let scene = SceneData::new(
            "test".to_string(),
            "test".to_string(),
            GridType::Square,
            32.0,
            10,
            10,
            None,
        );

        let pixel_1x = grid_to_pixel(1.0, 1.0, &scene, Vec2::ZERO, 1.0);
        let pixel_2x = grid_to_pixel(1.0, 1.0, &scene, Vec2::ZERO, 2.0);

        assert_eq!(pixel_1x.x / 2.0, pixel_2x.x);
        assert_eq!(pixel_1x.y / 2.0, pixel_2x.y);
    }

    #[test]
    fn test_snap_to_grid() {
        assert_eq!(snap_to_grid(1.2, 3.7), (1, 4));
        assert_eq!(snap_to_grid(5.5, 5.5), (6, 6));
        assert_eq!(snap_to_grid(0.0, 0.0), (0, 0));
    }

    #[test]
    fn test_bounds_check() {
        let scene = SceneData::new(
            "test".to_string(),
            "test".to_string(),
            GridType::Square,
            32.0,
            10,
            10,
            None,
        );

        assert!(is_within_bounds(160.0, 160.0, &scene));
        assert!(is_within_bounds(0.0, 0.0, &scene));
        assert!(is_within_bounds(320.0, 320.0, &scene));
        assert!(!is_within_bounds(-1.0, 160.0, &scene));
        assert!(!is_within_bounds(321.0, 160.0, &scene));
    }

    // Phase 4.7.G1: Additional edge case tests for robustness

    #[test]
    fn test_grid_to_pixel_negative_coordinates() {
        let scene = SceneData::new(
            "test".to_string(),
            "test".to_string(),
            GridType::Square,
            32.0,
            20,
            20,
            None,
        );

        // Negative grid coordinates (infinite canvas, left/up from origin)
        let pixel = grid_to_pixel(-5.0, -5.0, &scene, Vec2::ZERO, 1.0);
        assert_eq!(pixel.x, -160.0);
        assert_eq!(pixel.y, 480.0);  // Y inverted: -5 * 32 + (20*32)
    }

    #[test]
    fn test_pixel_to_grid_negative_coordinates() {
        let scene = SceneData::new(
            "test".to_string(),
            "test".to_string(),
            GridType::Square,
            32.0,
            20,
            20,
            None,
        );

        let (grid_x, grid_y) = pixel_to_grid(-160.0, 480.0, &scene, Vec2::ZERO, 1.0);
        assert_eq!(grid_x, -5.0);
        assert_eq!(grid_y, -5.0);
    }

    #[test]
    fn test_large_camera_pan() {
        let scene = SceneData::new(
            "test".to_string(),
            "test".to_string(),
            GridType::Square,
            32.0,
            100,
            100,
            None,
        );

        let camera_pan = Vec2::new(1000.0, 500.0);
        let pixel1 = grid_to_pixel(0.0, 0.0, &scene, Vec2::ZERO, 1.0);
        let pixel2 = grid_to_pixel(0.0, 0.0, &scene, camera_pan, 1.0);

        // Large pan should move pixel position proportionally
        assert!(pixel1.x - pixel2.x > 900.0);
        assert!(pixel1.y - pixel2.y > 400.0);
    }

    #[test]
    fn test_zoom_precision() {
        let scene = SceneData::new(
            "test".to_string(),
            "test".to_string(),
            GridType::Square,
            32.0,
            10,
            10,
            None,
        );

        // Test various zoom levels
        let zoom_levels = vec![0.25, 0.5, 0.909, 1.1, 2.0, 4.0];
        for zoom in zoom_levels {
            let pixel_1x = grid_to_pixel(5.0, 5.0, &scene, Vec2::ZERO, 1.0);
            let pixel_zx = grid_to_pixel(5.0, 5.0, &scene, Vec2::ZERO, zoom);

            // Zoomed should be 1/zoom scaled
            assert!((pixel_1x.x / zoom - pixel_zx.x).abs() < 0.01);
            assert!((pixel_1x.y / zoom - pixel_zx.y).abs() < 0.01);
        }
    }

    #[test]
    fn test_fractional_grid_positions() {
        let scene = SceneData::new(
            "test".to_string(),
            "test".to_string(),
            GridType::Square,
            32.0,
            10,
            10,
            None,
        );

        // Test positions between grid cells
        let (grid_x, grid_y) = pixel_to_grid(16.0, 304.0, &scene, Vec2::ZERO, 1.0);
        assert!((grid_x - 0.5).abs() < 0.01);
        assert!((grid_y - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_camera_pan_and_zoom_combined() {
        let scene = SceneData::new(
            "test".to_string(),
            "test".to_string(),
            GridType::Square,
            32.0,
            10,
            10,
            None,
        );

        let camera_pan = Vec2::new(100.0, 50.0);
        let camera_zoom = 2.0;

        // Grid position should be consistent regardless of camera
        let original_grid = (5.0, 3.0);
        let pixel = grid_to_pixel(original_grid.0, original_grid.1, &scene, camera_pan, camera_zoom);
        let converted_grid = pixel_to_grid(pixel.x, pixel.y, &scene, camera_pan, camera_zoom);

        assert!((converted_grid.0 - original_grid.0).abs() < 0.01);
        assert!((converted_grid.1 - original_grid.1).abs() < 0.01);
    }

    #[test]
    fn test_grid_size_scaling() {
        // Test different grid sizes
        for grid_size in [16.0, 32.0, 48.0, 64.0].iter() {
            let scene = SceneData::new(
                "test".to_string(),
                "test".to_string(),
                GridType::Square,
                *grid_size,
                10,
                10,
                None,
            );

            let pixel = grid_to_pixel(2.0, 2.0, &scene, Vec2::ZERO, 1.0);
            assert_eq!(pixel.x, 2.0 * grid_size);
            assert_eq!(pixel.y, (10.0 - 2.0) * grid_size);
        }
    }
}
