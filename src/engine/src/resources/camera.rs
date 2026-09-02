use bevy::prelude::*;
use thunderforge_canvas_core::camera::{ZoomLimits, fit_scale, zoom_steps, zoom_toward};

/// Camera manager for pan/zoom control
#[derive(Resource)]
pub struct CameraManager {
    /// Camera translation (pan offset in pixels)
    pub translation: Vec2,

    /// World units per screen unit. 1.0 is 1:1; **larger is zoomed out**.
    pub scale: f32,

    /// Most zoomed in (smallest scale).
    pub zoom_min: f32,

    /// Most zoomed out (largest scale).
    pub zoom_max: f32,

    /// Where zoom is heading. `scale` eases toward this each frame.
    ///
    /// Zoom used to jump straight to its new value, which reads as a series of
    /// discrete snaps rather than a camera moving — especially on a trackpad,
    /// where a single gesture delivers dozens of small deltas and each one
    /// teleported the view. Separating "where the user asked to be" from
    /// "where the camera is" is what makes it glide.
    pub target_scale: f32,
    /// Likewise for panning, so a cursor-anchored zoom eases rather than
    /// snapping the world sideways.
    pub target_translation: Vec2,
}

impl Default for CameraManager {
    fn default() -> Self {
        Self {
            translation: Vec2::ZERO,
            target_translation: Vec2::ZERO,
            scale: 1.0,
            target_scale: 1.0,
            // Was 0.25..=1.0, which capped zoom-*out* at 1:1 — a 6144px
            // imported map could never be framed in a 1600px viewport, which
            // needs roughly 4x. See `ZoomLimits::default`.
            zoom_min: 0.1,
            zoom_max: 12.0,
        }
    }
}

impl CameraManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Pan the camera by a delta, in world units.
    pub fn pan(&mut self, delta: Vec2) {
        self.target_translation += delta;
    }

    fn limits(&self) -> ZoomLimits {
        ZoomLimits {
            min: self.zoom_min,
            max: self.zoom_max,
        }
    }

    /// Zoom by `steps`, positive being **in**.
    pub fn zoom_by(&mut self, steps: f32) {
        self.target_scale = zoom_steps(self.target_scale, steps, self.limits());
    }

    /// Zoom by `steps` while keeping `anchor_world` pinned on screen — what
    /// the mouse wheel uses, so the point under the cursor stays there.
    pub fn zoom_toward(&mut self, anchor_world: Vec2, steps: f32) {
        // Anchored against the *target*, not the current position. Anchoring
        // against a mid-glide value would make each wheel notch during a
        // gesture correct for a camera that is still moving, and the view
        // would drift away from the cursor over a fast scroll.
        let (translation, scale) = zoom_toward(
            self.target_translation,
            self.target_scale,
            anchor_world,
            steps,
            self.limits(),
        );
        self.target_translation = translation;
        self.target_scale = scale;
    }

    /// Set an absolute zoom, clamped to the configured range.
    pub fn set_zoom(&mut self, scale: f32) {
        self.target_scale = self.limits().clamp(scale);
    }

    /// Frame `content_size` (world units) inside `viewport` (world units at
    /// 1:1), centred on `center`.
    pub fn fit_to(&mut self, center: Vec2, content_size: Vec2, viewport: Vec2) {
        self.target_scale = fit_scale(content_size, viewport, self.limits());
        self.target_translation = center;
    }

    /// Reset camera to initial state (pan=0, zoom=1:1).
    pub fn reset(&mut self) {
        self.target_translation = Vec2::ZERO;
        self.target_scale = 1.0;
    }

    /// Jumps the camera to its target with no glide.
    ///
    /// For the cases where easing would be wrong: the first frame of a scene,
    /// or a test that needs a deterministic camera without pumping frames.
    pub fn snap_to_target(&mut self) {
        self.translation = self.target_translation;
        self.scale = self.target_scale;
    }

    /// Eases the camera toward its target. Call once per frame with the
    /// frame's delta time.
    ///
    /// Exponential smoothing rather than a fixed step per frame, so the glide
    /// takes the same wall-clock time at any frame rate — a fixed step would
    /// make the camera twice as fast on a 120Hz display.
    pub fn advance(&mut self, delta_seconds: f32) {
        // 1 - e^(-k*dt): frame-rate independent, and `k` sets how quickly the
        // remaining distance is eaten. ~18 lands a zoom in roughly 150ms,
        // which reads as immediate but not instantaneous.
        const RESPONSIVENESS: f32 = 18.0;
        let t = 1.0 - (-RESPONSIVENESS * delta_seconds.max(0.0)).exp();

        self.scale += (self.target_scale - self.scale) * t;
        self.translation += (self.target_translation - self.translation) * t;

        // Settle exactly, so the camera stops rather than asymptotically
        // approaching forever and marking itself changed every frame.
        if (self.target_scale - self.scale).abs() < 0.0001 {
            self.scale = self.target_scale;
        }
        if self.target_translation.distance_squared(self.translation) < 0.0001 {
            self.translation = self.target_translation;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // # Why almost every test in here changed
    //
    // These had never been compiled, let alone run: the crate's test build was
    // broken (spec 032 T081-T083). In the meantime `pan` and `zoom_by` stopped
    // writing `translation`/`scale` and started writing `target_translation`/
    // `target_scale`, with the visible values easing toward them — so tests
    // that panned and then read `cam.translation` were reading a value the
    // call no longer touches, and would have failed on the first run. They now
    // call `snap_to_target()`, which exists for exactly this, before asserting
    // on the observable value.
    //
    // `zoom_in()`, `zoom_out()` and `zoom(factor)` are gone; `zoom_by(steps)`
    // (positive is in) and `set_zoom(scale)` replaced them.

    /// Pan and zoom, applied.
    fn settled(cam: &mut CameraManager) -> &mut CameraManager {
        cam.snap_to_target();
        cam
    }

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
        assert_eq!(settled(&mut cam).translation, Vec2::new(50.0, 30.0));

        cam.pan(Vec2::new(-20.0, 10.0));
        assert_eq!(settled(&mut cam).translation, Vec2::new(30.0, 40.0));
    }

    // These previously asserted the inverted behaviour: `zoom_in()` multiplied
    // the scale by 1.1 and the test's own message said so ("Zoom in should
    // multiply by 1.1"). Since `scale` is world-units-per-screen-unit, growing
    // it fits *more* world on screen — that is zooming out. The test pinned
    // the bug rather than catching it.

    #[test]
    fn zooming_in_shrinks_the_scale() {
        let mut cam = CameraManager::default();
        cam.zoom_by(1.0);
        assert!(
            settled(&mut cam).scale < 1.0,
            "zooming in should shrink the scale"
        );
    }

    #[test]
    fn zooming_out_grows_the_scale() {
        let mut cam = CameraManager::default();
        cam.zoom_by(-1.0);
        assert!(
            settled(&mut cam).scale > 1.0,
            "zooming out should grow the scale"
        );
    }

    /// One notch is exactly one `ZOOM_STEP`, in the direction the name says.
    ///
    /// Replaces `test_zoom_with_specific_factors`, which asserted
    /// `zoom_in()` landed on 1.1 — the inverted factor, written as the
    /// expectation.
    #[test]
    fn one_notch_is_one_zoom_step() {
        use thunderforge_canvas_core::camera::ZOOM_STEP;

        let mut cam = CameraManager::default();
        cam.zoom_by(1.0);
        assert!((settled(&mut cam).scale - 1.0 / ZOOM_STEP).abs() < 1e-6);

        cam.zoom_by(-1.0);
        assert!((settled(&mut cam).scale - 1.0).abs() < 1e-6);

        cam.set_zoom(2.0);
        assert!((settled(&mut cam).scale - 2.0).abs() < 1e-6);
    }

    #[test]
    fn test_zoom_clamped() {
        let mut cam = CameraManager::default();
        let (min, max) = (cam.zoom_min, cam.zoom_max);

        for _ in 0..200 {
            cam.zoom_by(1.0);
        }
        assert_eq!(
            settled(&mut cam).scale,
            min,
            "zooming in clamps at the min scale"
        );

        for _ in 0..200 {
            cam.zoom_by(-1.0);
        }
        assert_eq!(
            settled(&mut cam).scale,
            max,
            "zooming out clamps at the max scale"
        );
    }

    /// `set_zoom` is clamped too, not only the stepped path.
    #[test]
    fn setting_an_absolute_zoom_is_clamped_at_both_ends() {
        let mut cam = CameraManager::default();
        let (min, max) = (cam.zoom_min, cam.zoom_max);

        cam.set_zoom(0.0001);
        assert_eq!(settled(&mut cam).scale, min);

        cam.set_zoom(10_000.0);
        assert_eq!(settled(&mut cam).scale, max);

        // And a value inside the range is taken exactly.
        cam.set_zoom(3.0);
        assert_eq!(settled(&mut cam).scale, 3.0);
    }

    #[test]
    fn test_infinite_pan() {
        let mut cam = CameraManager::default();

        // Pan far beyond scene bounds (infinite canvas)
        cam.pan(Vec2::new(10000.0, -5000.0));
        assert_eq!(settled(&mut cam).translation, Vec2::new(10000.0, -5000.0));
        // No clamp, pan is unbounded
    }

    #[test]
    fn test_reset() {
        let mut cam = CameraManager::default();

        // Pan and zoom
        cam.pan(Vec2::new(100.0, -50.0));
        cam.zoom_by(2.0);
        cam.snap_to_target();
        assert_ne!(cam.translation, Vec2::ZERO);
        assert_ne!(cam.scale, 1.0);

        // Reset
        cam.reset();
        assert_eq!(settled(&mut cam).translation, Vec2::ZERO);
        assert_eq!(cam.scale, 1.0);
    }

    // Phase 4.7.G1: Additional camera tests for robustness

    #[test]
    fn test_multiple_pans() {
        let mut cam = CameraManager::default();

        cam.pan(Vec2::new(10.0, 20.0));
        cam.pan(Vec2::new(5.0, -10.0));
        cam.pan(Vec2::new(-15.0, 30.0));

        assert_eq!(settled(&mut cam).translation, Vec2::new(0.0, 40.0));
    }

    #[test]
    fn test_zoom_in_multiple_times() {
        let mut cam = CameraManager::default();
        let initial_scale = cam.scale;

        cam.zoom_by(5.0);
        cam.snap_to_target();

        assert!(cam.scale < initial_scale, "five notches in, scale shrinks");
        assert!(cam.scale >= cam.zoom_min);
    }

    #[test]
    fn test_zoom_out_multiple_times() {
        let mut cam = CameraManager::default();
        let initial_scale = cam.scale;

        cam.zoom_by(-5.0);
        cam.snap_to_target();

        assert!(cam.scale > initial_scale, "five notches out, scale grows");
        assert!(cam.scale <= cam.zoom_max);
    }

    #[test]
    fn test_zoom_oscillation() {
        let mut cam = CameraManager::default();
        let initial_scale = cam.scale;

        for _ in 0..10 {
            cam.zoom_by(1.0);
            cam.zoom_by(-1.0);
        }
        cam.snap_to_target();

        // After zoom in/out cycles, scale should be close to initial
        assert!((cam.scale - initial_scale).abs() < 0.01);
    }

    #[test]
    fn test_negative_pan() {
        let mut cam = CameraManager::default();

        // Pan into negative space (infinite canvas)
        cam.pan(Vec2::new(-10000.0, -5000.0));
        assert_eq!(settled(&mut cam).translation, Vec2::new(-10000.0, -5000.0));

        // Pan back
        cam.pan(Vec2::new(10000.0, 5000.0));
        assert_eq!(settled(&mut cam).translation, Vec2::ZERO);
    }

    #[test]
    fn test_large_pan_values() {
        let mut cam = CameraManager::default();

        // Very large pan values (beyond typical viewport)
        cam.pan(Vec2::new(1_000_000.0, -500_000.0));
        assert_eq!(
            settled(&mut cam).translation,
            Vec2::new(1_000_000.0, -500_000.0)
        );

        cam.reset();
        assert_eq!(settled(&mut cam).translation, Vec2::ZERO);
    }

    /// The glide is the reason every other test in here calls `snap_to_target`:
    /// a pan is a request, and `advance` is what carries the camera there.
    #[test]
    fn the_camera_eases_toward_its_target_rather_than_jumping() {
        let mut cam = CameraManager::default();
        cam.pan(Vec2::new(100.0, 0.0));

        assert_eq!(
            cam.translation,
            Vec2::ZERO,
            "panning moves the target, not the camera"
        );

        cam.advance(1.0 / 60.0);
        assert!(
            cam.translation.x > 0.0 && cam.translation.x < 100.0,
            "one frame gets part of the way, not all of it: {}",
            cam.translation.x
        );

        // Enough frames and it settles exactly, rather than approaching forever.
        for _ in 0..120 {
            cam.advance(1.0 / 60.0);
        }
        assert_eq!(cam.translation, Vec2::new(100.0, 0.0));
    }
}
