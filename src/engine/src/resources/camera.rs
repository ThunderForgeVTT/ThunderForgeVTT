use bevy::prelude::*;

/// Camera manager for pan/zoom control
#[derive(Resource)]
pub struct CameraManager {
    /// Camera translation (pan offset in pixels)
    pub translation: Vec2,

    /// Camera zoom scale (0.25 to 1.0, where 1.0 = normal, 0.25 = 4x zoom in)
    pub scale: f32,

    /// Min zoom (4x zoom in)
    pub zoom_min: f32,

    /// Max zoom (1x normal)
    pub zoom_max: f32,
}

impl Default for CameraManager {
    fn default() -> Self {
        Self {
            translation: Vec2::ZERO,
            scale: 1.0,
            zoom_min: 0.25,
            zoom_max: 1.0,
        }
    }
}

impl CameraManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Pan the camera by a delta
    pub fn pan(&mut self, delta: Vec2) {
        self.translation += delta;
    }

    /// Apply a zoom factor (multiplier)
    pub fn zoom(&mut self, factor: f32) {
        let new_scale = (self.scale * factor).clamp(self.zoom_min, self.zoom_max);
        self.scale = new_scale;
    }

    /// Zoom in by 10%
    pub fn zoom_in(&mut self) {
        self.zoom(1.1);
    }

    /// Zoom out by 10%
    pub fn zoom_out(&mut self) {
        self.zoom(0.909);
    }

    /// Reset camera to initial state (pan=0, zoom=1.0x)
    pub fn reset(&mut self) {
        self.translation = Vec2::ZERO;
        self.scale = 1.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_camera_manager_default() {
        let cam = CameraManager::default();
        assert_eq!(cam.translation, Vec2::ZERO);
        assert_eq!(cam.scale, 1.0);
    }

    #[test]
    fn test_pan() {
        let mut cam = CameraManager::default();
        cam.pan(Vec2::new(50.0, 30.0));
        assert_eq!(cam.translation, Vec2::new(50.0, 30.0));

        cam.pan(Vec2::new(-20.0, 10.0));
        assert_eq!(cam.translation, Vec2::new(30.0, 40.0));
    }

    #[test]
    fn test_zoom_in() {
        let mut cam = CameraManager::default();
        cam.zoom_in();
        assert!((cam.scale - 1.1).abs() < 0.001, "Zoom in should multiply by 1.1");
    }

    #[test]
    fn test_zoom_out() {
        let mut cam = CameraManager::default();
        cam.zoom_out();
        assert!((cam.scale - 0.909).abs() < 0.001, "Zoom out should multiply by ~0.909");
    }

    #[test]
    fn test_zoom_clamped() {
        let mut cam = CameraManager::default();

        // Zoom in to max
        for _ in 0..100 {
            cam.zoom_in();
        }
        assert_eq!(cam.scale, cam.zoom_max, "Should clamp to max");

        // Zoom out to min
        for _ in 0..100 {
            cam.zoom_out();
        }
        assert_eq!(cam.scale, cam.zoom_min, "Should clamp to min");
    }

    #[test]
    fn test_infinite_pan() {
        let mut cam = CameraManager::default();

        // Pan far beyond scene bounds (infinite canvas)
        cam.pan(Vec2::new(10000.0, -5000.0));
        assert_eq!(cam.translation, Vec2::new(10000.0, -5000.0));
        // No clamp, pan is unbounded
    }

    #[test]
    fn test_reset() {
        let mut cam = CameraManager::default();

        // Pan and zoom
        cam.pan(Vec2::new(100.0, -50.0));
        cam.zoom_in();
        cam.zoom_in();
        assert_ne!(cam.translation, Vec2::ZERO);
        assert_ne!(cam.scale, 1.0);

        // Reset
        cam.reset();
        assert_eq!(cam.translation, Vec2::ZERO);
        assert_eq!(cam.scale, 1.0);
    }

    // Phase 4.7.G1: Additional camera tests for robustness

    #[test]
    fn test_multiple_pans() {
        let mut cam = CameraManager::default();

        cam.pan(Vec2::new(10.0, 20.0));
        cam.pan(Vec2::new(5.0, -10.0));
        cam.pan(Vec2::new(-15.0, 30.0));

        assert_eq!(cam.translation, Vec2::new(0.0, 40.0));
    }

    #[test]
    fn test_zoom_in_multiple_times() {
        let mut cam = CameraManager::default();
        let initial_scale = cam.scale;

        for _ in 0..5 {
            cam.zoom_in();
        }

        assert!(cam.scale > initial_scale);
        assert!(cam.scale <= cam.zoom_max);
    }

    #[test]
    fn test_zoom_out_multiple_times() {
        let mut cam = CameraManager::default();

        for _ in 0..5 {
            cam.zoom_out();
        }

        assert!(cam.scale >= cam.zoom_min);
    }

    #[test]
    fn test_zoom_oscillation() {
        let mut cam = CameraManager::default();
        let initial_scale = cam.scale;

        for _ in 0..10 {
            cam.zoom_in();
            cam.zoom_out();
        }

        // After zoom in/out cycles, scale should be close to initial
        assert!((cam.scale - initial_scale).abs() < 0.01);
    }

    #[test]
    fn test_negative_pan() {
        let mut cam = CameraManager::default();

        // Pan into negative space (infinite canvas)
        cam.pan(Vec2::new(-10000.0, -5000.0));
        assert_eq!(cam.translation, Vec2::new(-10000.0, -5000.0));

        // Pan back
        cam.pan(Vec2::new(10000.0, 5000.0));
        assert_eq!(cam.translation, Vec2::ZERO);
    }

    #[test]
    fn test_zoom_clamped_exactly_on_boundaries() {
        let mut cam = CameraManager::default();

        // Zoom to exact min
        for _ in 0..1000 {
            cam.zoom_out();
        }
        assert_eq!(cam.scale, cam.zoom_min);

        // Zoom to exact max
        for _ in 0..1000 {
            cam.zoom_in();
        }
        assert_eq!(cam.scale, cam.zoom_max);
    }

    #[test]
    fn test_large_pan_values() {
        let mut cam = CameraManager::default();

        // Very large pan values (beyond typical viewport)
        cam.pan(Vec2::new(1_000_000.0, -500_000.0));
        assert_eq!(cam.translation, Vec2::new(1_000_000.0, -500_000.0));

        cam.reset();
        assert_eq!(cam.translation, Vec2::ZERO);
    }

    #[test]
    fn test_zoom_with_specific_factors() {
        let mut cam = CameraManager::default();
        let initial = 1.0;

        // Test 1.1x factor
        cam.zoom_in();
        assert!((cam.scale - 1.1).abs() < 0.001);

        // Test 0.909x factor
        cam.zoom_out();
        assert!((cam.scale - 1.0).abs() < 0.001);

        // Test exact zoom(factor)
        cam.zoom(2.0);
        assert!((cam.scale - 2.0).abs() < 0.001);
    }

    #[test]
    fn test_zoom_below_minimum() {
        let mut cam = CameraManager::default();
        cam.zoom(0.1);  // Try to zoom below min

        assert!(cam.scale >= cam.zoom_min);
    }

    #[test]
    fn test_zoom_above_maximum() {
        let mut cam = CameraManager::default();
        cam.zoom(10.0);  // Try to zoom above max

        assert!(cam.scale <= cam.zoom_max);
    }
}
