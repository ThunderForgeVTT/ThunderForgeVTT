//! Which tokens are under a point, and in what order.
//!
//! Tokens stack: two characters in a doorway, a familiar on its owner, a
//! swarm in one square — and every token in a freshly created scene spawns
//! on the same spot. The engine used to take the first hit and stop
//! (`break; // First hit wins`), which meant everything underneath was
//! unreachable without dragging the pile apart one token at a time.
//!
//! Reporting the whole stack instead is what lets a click move the pile
//! together and a double-click offer a choice. Both gestures read this one
//! function, so a picker can never list a token the click would have
//! missed.
//!
//! The hit area is the token's **grid footprint**, not its art. Art is
//! fitted inside the footprint and can be far narrower than its cell — a
//! side-on starship is — so testing against the sprite would make a token
//! harder to click the less square its art happened to be, which is a rule
//! no player could predict or see.

use glam::Vec2;

/// A token considered for a hit test.
#[derive(Debug, Clone, PartialEq)]
pub struct StackCandidate {
    pub id: String,
    /// Centre, in world units.
    pub center: Vec2,
    /// Side of the square grid footprint the token occupies, in world units.
    pub footprint_side: f32,
    /// Draw order; higher is nearer the viewer.
    pub z: f32,
}

/// Ids of every token whose footprint contains `point`, topmost first.
///
/// Ties on `z` fall back to id so the order is stable across calls. That is
/// not fussiness: the double-click picker renders this list, and entries
/// that reshuffle between the click and the reach are worse than no picker
/// at all.
pub fn tokens_at(candidates: &[StackCandidate], point: Vec2) -> Vec<String> {
    let mut hits: Vec<&StackCandidate> = candidates
        .iter()
        .filter(|candidate| {
            let half = candidate.footprint_side.max(0.0) / 2.0;
            (point.x - candidate.center.x).abs() <= half
                && (point.y - candidate.center.y).abs() <= half
        })
        .collect();

    hits.sort_by(|a, b| {
        b.z.total_cmp(&a.z).then_with(|| a.id.cmp(&b.id))
    });

    hits.into_iter().map(|candidate| candidate.id.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(id: &str, x: f32, y: f32, z: f32) -> StackCandidate {
        StackCandidate {
            id: id.to_string(),
            center: Vec2::new(x, y),
            footprint_side: 64.0,
            z,
        }
    }

    #[test]
    fn every_token_at_the_point_is_reported_not_just_the_top_one() {
        // The behaviour the old `break` prevented.
        let stack = vec![
            candidate("a", 0.0, 0.0, 0.0),
            candidate("b", 0.0, 0.0, 0.0),
            candidate("c", 0.0, 0.0, 0.0),
        ];
        assert_eq!(tokens_at(&stack, Vec2::ZERO).len(), 3);
    }

    #[test]
    fn ordered_topmost_first() {
        let stack = vec![
            candidate("low", 0.0, 0.0, 0.0),
            candidate("high", 0.0, 0.0, 10.0),
            candidate("mid", 0.0, 0.0, 5.0),
        ];
        assert_eq!(tokens_at(&stack, Vec2::ZERO), vec!["high", "mid", "low"]);
    }

    #[test]
    fn ties_are_broken_stably_so_a_picker_does_not_reshuffle() {
        let forward = vec![candidate("b", 0.0, 0.0, 1.0), candidate("a", 0.0, 0.0, 1.0)];
        let reversed: Vec<StackCandidate> = forward.iter().rev().cloned().collect();
        assert_eq!(tokens_at(&forward, Vec2::ZERO), vec!["a", "b"]);
        assert_eq!(tokens_at(&reversed, Vec2::ZERO), tokens_at(&forward, Vec2::ZERO));
    }

    #[test]
    fn the_footprint_is_the_hit_area_not_the_art() {
        // A 64-unit footprint is hit anywhere within +/-32 of its centre,
        // regardless of how narrow the art inside it happens to be.
        let stack = vec![candidate("t", 0.0, 0.0, 0.0)];
        assert_eq!(tokens_at(&stack, Vec2::new(31.0, -31.0)).len(), 1);
        assert_eq!(tokens_at(&stack, Vec2::new(33.0, 0.0)).len(), 0);
    }

    #[test]
    fn the_footprint_edge_counts_as_a_hit() {
        let stack = vec![candidate("t", 0.0, 0.0, 0.0)];
        assert_eq!(tokens_at(&stack, Vec2::new(32.0, 32.0)).len(), 1);
    }

    #[test]
    fn a_larger_footprint_covers_more_ground() {
        // Bigger creatures occupy N cells and must be clickable across all
        // of them.
        let big = StackCandidate {
            id: "ogre".into(),
            center: Vec2::ZERO,
            footprint_side: 256.0,
            z: 0.0,
        };
        assert_eq!(tokens_at(&[big], Vec2::new(120.0, -120.0)).len(), 1);
    }

    #[test]
    fn empty_canvas_reports_nothing_which_is_a_deselect_not_an_error() {
        let stack = vec![candidate("far", 500.0, 500.0, 0.0)];
        assert!(tokens_at(&stack, Vec2::ZERO).is_empty());
    }

    #[test]
    fn a_degenerate_footprint_is_not_a_hit_magnet() {
        // A zero or negative side must not match everything via a negative
        // half-extent comparison.
        let degenerate = StackCandidate {
            id: "zero".into(),
            center: Vec2::ZERO,
            footprint_side: -10.0,
            z: 0.0,
        };
        assert_eq!(tokens_at(&[degenerate.clone()], Vec2::ZERO).len(), 1);
        assert_eq!(tokens_at(&[degenerate], Vec2::new(1.0, 0.0)).len(), 0);
    }
}
