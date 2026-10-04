//! What the camera can see, padded — and which tokens fall outside it.
//!
//! A token nobody can see should not pay for the furniture drawn around it.
//! The renderer's own frustum test already skips *drawing* an off-screen
//! sprite, but the measured cost on a crowded board is not fill: it is that
//! the entities exist at all (3,200 tokens with status bars carried 16,003
//! sprites against 3,203 without, and ran at 20fps against 59). So the engine
//! removes an off-screen token's decorations outright, and this module decides
//! what "off-screen" means.
//!
//! # Padded by a quarter of the view
//!
//! A token is kept fully present while it is inside the view **expanded by
//! 25% of the view's own width and height on every side** ([`VIEW_PAD`]). The
//! padding is proportional rather than a fixed distance because the thing it
//! protects against is proportional: a fast pan or a zoom-out moves the edge
//! by some fraction of the screen per frame, whatever the zoom. A fixed 200
//! units is a generous margin zoomed in and nothing at all zoomed out to a
//! whole battlemap.
//!
//! # Two edges, not one
//!
//! With a single edge, a token sitting on it would have its decorations
//! built and torn down every time the camera breathed. So there are two
//! ([`CullBand`]): a token becomes present on crossing the inner one and is
//! only culled on leaving the outer one. Between them it keeps whatever state
//! it had.
//!
//! # Why the inner edge is 30%, not 25%
//!
//! The engine does not re-evaluate every token on every frame the camera
//! moves; it waits until the view has drifted by more than [`VIEW_DRIFT`] of
//! its size. The promise is still about the view *as it is now*, so the band
//! built from the last evaluated view has to cover everything the live view's
//! 25% padding could reach before the next evaluation. With corners allowed to
//! drift 2% of the size, the live view can be 4% larger and 2% further out:
//! `0.02 + 0.25 × 1.04 = 0.28`. [`SHOW_PAD`] is 0.30. The test
//! `the_show_edge_covers_the_promised_padding_of_any_view_within_drift` holds
//! that arithmetic to account.

use glam::Vec2;

/// The padding the feature promises: a token inside the view expanded by this
/// fraction of its size, on each side, is fully present.
pub const VIEW_PAD: f32 = 0.25;

/// How far the view may drift, as a fraction of its size, before every token
/// is evaluated again. Small enough that [`SHOW_PAD`] can absorb it, large
/// enough that a camera at rest — which still jitters in the low bits — is
/// never mistaken for one that moved.
pub const VIEW_DRIFT: f32 = 0.02;

/// Where a culled token becomes present again. See the module docs for why
/// this is wider than [`VIEW_PAD`].
pub const SHOW_PAD: f32 = 0.30;

/// Where a present token is culled. The gap between this and [`SHOW_PAD`] is
/// the hysteresis: 10% of the view is far more than a camera jitters.
pub const HIDE_PAD: f32 = 0.40;

/// An axis-aligned rectangle in world units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewRect {
    pub min: Vec2,
    pub max: Vec2,
}

impl ViewRect {
    /// A rectangle from any two opposite corners.
    pub fn from_corners(a: Vec2, b: Vec2) -> Self {
        Self {
            min: a.min(b),
            max: a.max(b),
        }
    }

    pub fn size(&self) -> Vec2 {
        self.max - self.min
    }

    /// Expanded by `fraction` of its own width and height on every side.
    ///
    /// This is the one definition of "the padded view" — a wide view gains
    /// more world on its long axis than its short one, because that is the
    /// axis a pan covers more of per frame.
    pub fn padded(&self, fraction: f32) -> Self {
        let pad = self.size() * fraction;
        Self {
            min: self.min - pad,
            max: self.max + pad,
        }
    }

    /// Expanded by a fixed distance on every side — for the size of the thing
    /// being tested, where [`padded`](Self::padded) is for the camera.
    pub fn inflated(&self, units: f32) -> Self {
        Self {
            min: self.min - Vec2::splat(units),
            max: self.max + Vec2::splat(units),
        }
    }

    /// Edges count as inside.
    pub fn contains(&self, point: Vec2) -> bool {
        point.x >= self.min.x
            && point.x <= self.max.x
            && point.y >= self.min.y
            && point.y <= self.max.y
    }

    /// Whether this view has moved or resized by more than `fraction` of
    /// `earlier`'s size, on either axis, at either corner.
    ///
    /// Relative, so the same threshold serves a close-up and a whole-map view.
    /// A view with no area is always called drifted: nothing sensible can be
    /// measured against it, and re-evaluating is the safe answer.
    pub fn drifted_from(&self, earlier: &Self, fraction: f32) -> bool {
        let size = earlier.size();
        if size.x <= 0.0 || size.y <= 0.0 || !size.is_finite() {
            return true;
        }
        let allowed = size * fraction;
        let moved = (self.min - earlier.min)
            .abs()
            .max((self.max - earlier.max).abs());
        moved.x > allowed.x || moved.y > allowed.y
    }
}

/// The two edges a token is judged against. See the module docs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CullBand {
    /// Inside this, a token is present.
    pub show: ViewRect,
    /// Outside this, a token is culled.
    pub hide: ViewRect,
}

impl CullBand {
    /// The band for a view, as evaluated now.
    pub fn around(view: ViewRect) -> Self {
        Self {
            show: view.padded(SHOW_PAD),
            hide: view.padded(HIDE_PAD),
        }
    }

    /// Whether a token centred at `centre` should be culled.
    ///
    /// `reach` is how far the token and everything drawn around it extends
    /// from that centre: a token whose centre is past the edge still has a
    /// body, bars and a name on the near side of it. `was_culled` is the
    /// state it had, which decides the answer between the two edges and
    /// nowhere else.
    pub fn culls(&self, centre: Vec2, reach: f32, was_culled: bool) -> bool {
        if self.show.inflated(reach).contains(centre) {
            return false;
        }
        if !self.hide.inflated(reach).contains(centre) {
            return true;
        }
        was_culled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 1600 × 900 view centred on the origin.
    fn view() -> ViewRect {
        ViewRect {
            min: Vec2::new(-800.0, -450.0),
            max: Vec2::new(800.0, 450.0),
        }
    }

    #[test]
    fn padding_adds_a_quarter_of_each_dimension_to_each_side() {
        let padded = view().padded(VIEW_PAD);
        assert_eq!(padded.min, Vec2::new(-1200.0, -675.0));
        assert_eq!(padded.max, Vec2::new(1200.0, 675.0));
        // Half as wide again on each axis: 25% on the left, 25% on the right.
        assert_eq!(padded.size(), view().size() * 1.5);
    }

    #[test]
    fn padding_is_proportional_so_a_zoomed_out_view_pads_further() {
        let close = view().padded(VIEW_PAD);
        let far = ViewRect {
            min: view().min * 4.0,
            max: view().max * 4.0,
        }
        .padded(VIEW_PAD);
        assert_eq!(close.max.x - view().max.x, 400.0);
        assert_eq!(far.max.x - view().max.x * 4.0, 1600.0);
    }

    #[test]
    fn padding_follows_a_view_that_is_not_centred_on_the_origin() {
        let moved = ViewRect {
            min: Vec2::new(1000.0, 2000.0),
            max: Vec2::new(1400.0, 2200.0),
        }
        .padded(VIEW_PAD);
        assert_eq!(moved.min, Vec2::new(900.0, 1950.0));
        assert_eq!(moved.max, Vec2::new(1500.0, 2250.0));
    }

    #[test]
    fn corners_given_in_either_order_make_the_same_rectangle() {
        let a = ViewRect::from_corners(Vec2::new(5.0, -3.0), Vec2::new(-1.0, 9.0));
        assert_eq!(a.min, Vec2::new(-1.0, -3.0));
        assert_eq!(a.max, Vec2::new(5.0, 9.0));
    }

    #[test]
    fn a_point_on_the_edge_is_inside() {
        assert!(view().contains(Vec2::new(800.0, 450.0)));
        assert!(view().contains(Vec2::new(-800.0, 0.0)));
        assert!(!view().contains(Vec2::new(800.1, 0.0)));
        assert!(!view().contains(Vec2::new(0.0, -450.1)));
    }

    #[test]
    fn a_view_at_rest_has_not_drifted_and_a_panned_or_zoomed_one_has() {
        let earlier = view();
        assert!(!earlier.drifted_from(&earlier, VIEW_DRIFT));

        // Low-bit jitter, and a nudge well inside 2% of 1600.
        let nudged = ViewRect {
            min: earlier.min + Vec2::new(10.0, 0.0),
            max: earlier.max + Vec2::new(10.0, 0.0),
        };
        assert!(!nudged.drifted_from(&earlier, VIEW_DRIFT));

        // 2% of the width is 32, of the height 18: the short axis is stricter.
        let panned_x = ViewRect {
            min: earlier.min + Vec2::new(33.0, 0.0),
            max: earlier.max + Vec2::new(33.0, 0.0),
        };
        assert!(panned_x.drifted_from(&earlier, VIEW_DRIFT));
        let panned_y = ViewRect {
            min: earlier.min + Vec2::new(0.0, 19.0),
            max: earlier.max + Vec2::new(0.0, 19.0),
        };
        assert!(panned_y.drifted_from(&earlier, VIEW_DRIFT));

        // Zooming out moves both corners outward without moving the centre.
        let zoomed = ViewRect {
            min: earlier.min * 1.1,
            max: earlier.max * 1.1,
        };
        assert!(zoomed.drifted_from(&earlier, VIEW_DRIFT));
    }

    #[test]
    fn a_view_with_no_area_is_always_treated_as_drifted() {
        let empty = ViewRect {
            min: Vec2::ZERO,
            max: Vec2::ZERO,
        };
        assert!(view().drifted_from(&empty, VIEW_DRIFT));
        assert!(empty.drifted_from(&empty, VIEW_DRIFT));
    }

    #[test]
    fn a_token_is_present_inside_the_show_edge_and_culled_outside_the_hide_edge() {
        let band = CullBand::around(view());
        // In plain view, whatever it was before.
        assert!(!band.culls(Vec2::ZERO, 0.0, true));
        // Off-screen but inside the padding: 1600 wide, so 30% is 480 past 800.
        assert!(!band.culls(Vec2::new(1200.0, 0.0), 0.0, true));
        // Far away, whatever it was before: 40% is 640 past 800.
        assert!(band.culls(Vec2::new(1500.0, 0.0), 0.0, false));
        assert!(band.culls(Vec2::new(0.0, -5000.0), 0.0, false));
    }

    #[test]
    fn between_the_edges_a_token_keeps_the_state_it_had() {
        let band = CullBand::around(view());
        // 1350 is past the show edge (1280) and short of the hide edge (1440).
        let between = Vec2::new(1350.0, 0.0);
        assert!(band.culls(between, 0.0, true), "stays culled");
        assert!(!band.culls(between, 0.0, false), "stays present");
    }

    #[test]
    fn a_tokens_reach_counts_so_its_near_side_is_never_cut_off() {
        let band = CullBand::around(view());
        // Centre 100 past the hide edge, but the token reaches 150.
        let centre = Vec2::new(1540.0, 0.0);
        assert!(band.culls(centre, 0.0, false));
        assert!(!band.culls(centre, 150.0, false));
    }

    /// The arithmetic in the module docs. The engine re-evaluates only once
    /// the view has drifted past [`VIEW_DRIFT`]; until then the band from the
    /// *earlier* view must still cover the 25% padding of the *live* one.
    #[test]
    fn the_show_edge_covers_the_promised_padding_of_any_view_within_drift() {
        let evaluated = view();
        let show = CullBand::around(evaluated).show;
        let size = evaluated.size();
        // The extremes of what `drifted_from` still calls "not drifted":
        // every corner pushed as far out, and as far across, as it may go.
        let d = size * VIEW_DRIFT;
        for (dmin, dmax) in [
            (-d, d),  // grown: zoomed out as far as allowed
            (d, d),   // panned up and right
            (-d, -d), // panned down and left
            (d, -d),  // shrunk
            (-d, Vec2::ZERO),
            (Vec2::ZERO, d),
        ] {
            let live = ViewRect {
                min: evaluated.min + dmin,
                max: evaluated.max + dmax,
            };
            assert!(!live.drifted_from(&evaluated, VIEW_DRIFT));
            let promised = live.padded(VIEW_PAD);
            assert!(
                show.contains(promised.min) && show.contains(promised.max),
                "live view {live:?} promises {promised:?}, outside {show:?}"
            );
        }
    }
}
