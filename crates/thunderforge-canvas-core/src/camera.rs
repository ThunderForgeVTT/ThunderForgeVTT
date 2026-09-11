//! Camera pan and zoom math.
//!
//! # What `scale` means
//!
//! `scale` is **world units per screen unit** — the orthographic projection's
//! scale. A larger scale fits more world on screen, so *larger scale means
//! zoomed out*. That inversion is the source of a bug this module exists to
//! prevent: the engine previously had `zoom_in()` multiply the scale by 1.1,
//! which zooms **out**. The functions here take an explicit direction instead
//! of a bare multiplier so the caller cannot get it backwards silently.

use glam::Vec2;

/// How far the camera may zoom, as world-units-per-screen-unit.
#[derive(Clone, Copy, Debug)]
pub struct ZoomLimits {
    /// Most zoomed *in* (smallest scale).
    pub min: f32,
    /// Most zoomed *out* (largest scale).
    pub max: f32,
}

impl Default for ZoomLimits {
    /// Wide enough for a battlemap.
    ///
    /// The old range was 0.25..=1.0, which capped zoom-out at 1:1 — a 6144px
    /// imported map could never be seen whole in a 1600px viewport, since that
    /// needs roughly 4x. Zooming out is the operation a VTT needs most, so the
    /// range is deliberately lopsided toward it.
    fn default() -> Self {
        Self {
            min: 0.1,
            max: 12.0,
        }
    }
}

impl ZoomLimits {
    pub fn clamp(&self, scale: f32) -> f32 {
        // A non-finite scale would poison every subsequent camera update, so
        // it resolves to 1:1 rather than propagating.
        if !scale.is_finite() {
            return 1.0;
        }
        scale.clamp(self.min.min(self.max), self.max.max(self.min))
    }
}

/// Multiplier applied per zoom step. ~11% feels responsive without
/// overshooting on a trackpad's many small deltas.
pub const ZOOM_STEP: f32 = 1.11;

/// Pixels of scroll one detent of a mouse wheel reports, on platforms that
/// measure the wheel in pixels rather than lines.
///
/// A browser is one such platform: Chrome answers a single wheel notch with
/// `deltaY` of 100, which winit forwards as a pixel delta. Feeding that
/// straight into [`zoom_steps`] as a step count asks for `ZOOM_STEP^100` —
/// which is not a zoom, it is a teleport to the nearest limit. One notch
/// should be one step, so pixels are divided back down by this.
pub const WHEEL_PIXELS_PER_NOTCH: f32 = 100.0;

/// How many zoom steps a raw wheel delta is worth.
///
/// The wheel reports in one of two units and *which one is not a constant* —
/// it varies by platform, by browser and between a mouse and a trackpad on
/// the same machine. Reading the delta without consulting the unit therefore
/// works on whichever device it was written on and is wildly wrong on the
/// next one, so this takes the unit as an argument rather than assuming.
///
/// Trackpads report many small pixel deltas rather than discrete notches;
/// dividing keeps their fractional steps fractional, which is what makes a
/// two-finger zoom feel continuous instead of ratcheting.
pub fn wheel_notches(delta: f32, in_pixels: bool) -> f32 {
    if !delta.is_finite() {
        return 0.0;
    }
    if in_pixels {
        delta / WHEEL_PIXELS_PER_NOTCH
    } else {
        delta
    }
}

/// The scale after zooming by `steps`, positive being **in**.
///
/// Fractional steps are meaningful: a trackpad delta of 0.3 zooms a third of
/// a step.
pub fn zoom_steps(scale: f32, steps: f32, limits: ZoomLimits) -> f32 {
    // Zooming in reduces the scale, hence the negated exponent. Exponential
    // rather than linear so each step feels the same at any magnification —
    // linear stepping crawls when zoomed out and lurches when zoomed in.
    limits.clamp(scale * ZOOM_STEP.powf(-steps))
}

/// Zooms while keeping the world point under `anchor` pinned to the same place
/// on screen.
///
/// This is what makes wheel-zoom feel like a map rather than a slideshow: the
/// thing under the cursor stays under the cursor. Without it the view drifts
/// toward the camera centre and the user has to pan back after every zoom.
///
/// Returns the new `(translation, scale)`.
pub fn zoom_toward(
    translation: Vec2,
    scale: f32,
    anchor_world: Vec2,
    steps: f32,
    limits: ZoomLimits,
) -> (Vec2, f32) {
    let new_scale = zoom_steps(scale, steps, limits);

    // Clamped at a limit, so nothing moves. Returning early avoids a
    // divide-by-old-scale that would be a no-op anyway.
    if !scale.is_finite() || scale.abs() <= f32::EPSILON {
        return (translation, new_scale);
    }

    // The anchor's screen offset is `(anchor - translation) / scale`. Holding
    // that constant across the scale change gives the new translation
    // directly.
    let ratio = new_scale / scale;
    let new_translation = anchor_world + (translation - anchor_world) * ratio;

    (new_translation, new_scale)
}

/// The scale at which `content` exactly fits inside `viewport`.
///
/// Uses the larger of the two axis ratios so the whole of `content` fits,
/// letterboxing the other axis rather than cropping.
pub fn fit_scale(content: Vec2, viewport: Vec2, limits: ZoomLimits) -> f32 {
    if viewport.x <= f32::EPSILON || viewport.y <= f32::EPSILON {
        return limits.clamp(1.0);
    }
    let by_width = content.x / viewport.x;
    let by_height = content.y / viewport.y;
    limits.clamp(by_width.max(by_height))
}

/// How far, in screen pixels, a pointer may travel with a button held and
/// still be a click rather than a drag.
///
/// Right-click both pans (by dragging) and opens the context menu (by
/// clicking), so something has to tell the two apart, and a hand on a mouse
/// is never perfectly still. A few pixels absorbs the wobble without making a
/// deliberate drag feel sticky.
pub const DRAG_THRESHOLD_PX: f32 = 4.0;

/// Whether a press whose pointer has travelled `travelled` screen pixels from
/// where it went down is a drag.
///
/// `travelled` is the furthest the pointer got, not where it ended: a drag
/// that wanders back to its starting point was still a drag, and must not
/// open a menu on release.
pub fn is_drag(travelled: f32) -> bool {
    travelled.is_finite() && travelled > DRAG_THRESHOLD_PX
}

/// How far the camera moves, in world units, when the pointer drags from
/// `from` to `to` (screen pixels) at `scale`.
///
/// Grab-the-map: the world point under the pointer stays under the pointer,
/// so the camera moves *opposite* to the pointer. Screen y grows downward and
/// world y grows upward, hence the sign flip on x and not on y. Multiplied by
/// `scale` because one screen pixel is `scale` world units — a drag across the
/// screen zoomed out covers more of the map than the same drag zoomed in.
pub fn drag_pan(from: Vec2, to: Vec2, scale: f32) -> Vec2 {
    let moved = to - from;
    if !moved.is_finite() || !scale.is_finite() {
        return Vec2::ZERO;
    }
    Vec2::new(-moved.x, moved.y) * scale
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits() -> ZoomLimits {
        ZoomLimits::default()
    }

    /// Where a screen point lands in the world, for a camera at `translation`
    /// and `scale` over a viewport centred on `center` — the inverse of what
    /// the renderer does, written out so the grab property can be checked.
    fn world_at(screen: Vec2, center: Vec2, translation: Vec2, scale: f32) -> Vec2 {
        let offset = screen - center;
        translation + Vec2::new(offset.x, -offset.y) * scale
    }

    #[test]
    fn the_point_you_grab_stays_under_the_pointer() {
        let center = Vec2::new(800.0, 450.0);
        let (from, to) = (Vec2::new(300.0, 200.0), Vec2::new(520.0, 90.0));
        for scale in [0.5, 1.0, 3.0] {
            let camera = Vec2::new(40.0, -25.0);
            let grabbed = world_at(from, center, camera, scale);
            let moved = camera + drag_pan(from, to, scale);
            let now_under = world_at(to, center, moved, scale);
            assert!(
                (grabbed - now_under).length() < 1e-3,
                "at scale {scale}, {grabbed:?} slid to {now_under:?}",
            );
        }
    }

    #[test]
    fn dragging_right_and_down_moves_the_camera_left_and_up() {
        let pan = drag_pan(Vec2::ZERO, Vec2::new(10.0, 10.0), 1.0);
        assert!(
            pan.x < 0.0,
            "a rightward drag moves the camera left: {pan:?}"
        );
        assert!(pan.y > 0.0, "a downward drag moves the camera up: {pan:?}");
    }

    #[test]
    fn a_wobble_is_a_click_and_a_deliberate_move_is_a_drag() {
        assert!(!is_drag(0.0));
        assert!(!is_drag(DRAG_THRESHOLD_PX));
        assert!(is_drag(DRAG_THRESHOLD_PX + 1.0));
        assert!(
            !is_drag(f32::NAN),
            "an unknown distance must not swallow a click"
        );
    }

    #[test]
    fn a_non_finite_drag_moves_nothing() {
        assert_eq!(drag_pan(Vec2::ZERO, Vec2::splat(f32::NAN), 1.0), Vec2::ZERO);
        assert_eq!(drag_pan(Vec2::ZERO, Vec2::ONE, f32::INFINITY), Vec2::ZERO);
    }

    #[test]
    fn a_pixel_wheel_notch_is_one_step() {
        // The bug this guards: a browser reports one wheel notch as a pixel
        // delta of 100, and feeding that in as a step count zoomed by
        // ZOOM_STEP^100 — every notch slammed the camera into a limit.
        assert_eq!(wheel_notches(WHEEL_PIXELS_PER_NOTCH, true), 1.0);
        assert_eq!(wheel_notches(-WHEEL_PIXELS_PER_NOTCH, true), -1.0);
    }

    #[test]
    fn a_line_wheel_delta_is_already_in_steps() {
        // Line-unit platforms report notches directly; dividing those too
        // would make the wheel almost inert instead of almost instant.
        assert_eq!(wheel_notches(1.0, false), 1.0);
        assert_eq!(wheel_notches(-3.0, false), -3.0);
    }

    #[test]
    fn a_trackpads_small_pixel_deltas_stay_fractional() {
        // Rounding these to whole notches is what turns a smooth two-finger
        // zoom into a ratchet.
        let notches = wheel_notches(12.0, true);
        assert!(notches > 0.0 && notches < 1.0, "got {notches}");
    }

    #[test]
    fn one_notch_zooms_by_one_step_not_a_hundred() {
        // End to end, in the units the caller actually receives: a single
        // browser wheel notch must be an ~11% change, not a jump to the
        // zoom limit.
        let steps = wheel_notches(WHEEL_PIXELS_PER_NOTCH, true);
        let zoomed = zoom_steps(1.0, steps, limits());
        assert!(
            (zoomed - 1.0 / ZOOM_STEP).abs() < 1e-6,
            "one notch should be one step, got {zoomed}"
        );
        assert!(
            zoomed > limits().min,
            "one notch must not reach the zoom-in limit"
        );
    }

    #[test]
    fn a_non_finite_wheel_delta_scrolls_nothing() {
        assert_eq!(wheel_notches(f32::NAN, true), 0.0);
        assert_eq!(wheel_notches(f32::INFINITY, false), 0.0);
    }

    #[test]
    fn zooming_in_reduces_the_scale() {
        // The bug this guards: the engine's `zoom_in()` multiplied the scale,
        // which zooms out. Direction is now explicit in the signature.
        let zoomed_in = zoom_steps(1.0, 1.0, limits());
        assert!(zoomed_in < 1.0, "zooming in should shrink the scale");

        let zoomed_out = zoom_steps(1.0, -1.0, limits());
        assert!(zoomed_out > 1.0, "zooming out should grow the scale");
    }

    #[test]
    fn zoom_steps_are_reversible() {
        let start = 2.0;
        let there = zoom_steps(start, 3.0, limits());
        let back = zoom_steps(there, -3.0, limits());
        assert!(
            (back - start).abs() < 1e-4,
            "{back} should return to {start}"
        );
    }

    #[test]
    fn zoom_is_clamped_at_both_ends() {
        let l = limits();
        assert_eq!(zoom_steps(l.min, 50.0, l), l.min);
        assert_eq!(zoom_steps(l.max, -50.0, l), l.max);
    }

    #[test]
    fn the_default_range_can_frame_a_large_imported_map() {
        // A 6144px map in a 1600px viewport needs ~3.84. The old 1.0 ceiling
        // made that impossible.
        let needed = fit_scale(
            Vec2::new(6144.0, 3456.0),
            Vec2::new(1600.0, 900.0),
            limits(),
        );
        assert!(needed > 3.8 && needed < 4.0, "got {needed}");
        assert!(needed < limits().max, "the limit must allow framing it");
    }

    #[test]
    fn the_anchor_point_stays_put_while_zooming() {
        let anchor = Vec2::new(300.0, -120.0);
        let (translation, scale) = zoom_toward(Vec2::new(50.0, 50.0), 2.0, anchor, 2.0, limits());

        // Screen offset of the anchor, before and after.
        let before = (anchor - Vec2::new(50.0, 50.0)) / 2.0;
        let after = (anchor - translation) / scale;
        assert!(
            (before - after).length() < 1e-3,
            "anchor moved on screen: {before:?} -> {after:?}",
        );
    }

    #[test]
    fn zooming_on_the_camera_centre_does_not_pan() {
        let center = Vec2::new(-40.0, 90.0);
        let (translation, _) = zoom_toward(center, 1.5, center, 4.0, limits());
        assert!((translation - center).length() < 1e-4);
    }

    #[test]
    fn fit_uses_the_axis_that_needs_the_most_room() {
        // Tall content in a wide viewport must fit by height.
        let scale = fit_scale(Vec2::new(100.0, 4000.0), Vec2::new(1600.0, 900.0), limits());
        assert!((scale - 4000.0 / 900.0).abs() < 1e-4, "got {scale}");
    }

    #[test]
    fn degenerate_input_does_not_produce_a_broken_camera() {
        let l = limits();
        assert!(l.clamp(f32::NAN).is_finite());
        assert!(fit_scale(Vec2::new(100.0, 100.0), Vec2::ZERO, l).is_finite());

        let (translation, scale) = zoom_toward(Vec2::ZERO, 0.0, Vec2::new(5.0, 5.0), 1.0, l);
        assert!(translation.is_finite() && scale.is_finite());
    }
}
