//! Distance in the numbers a table actually says out loud.
//!
//! A grid measures in cells, but nobody at a table says "three cells" — they
//! say "fifteen feet", or "ten metres", or "three hexes". The conversion is
//! two values: how much distance one cell represents, and what that unit is
//! called. Both belong to the scene, not to this crate, because they differ
//! per ruleset:
//!
//! | ruleset            | per cell | label  |
//! |--------------------|----------|--------|
//! | D&D 5e             | 5        | `ft`   |
//! | most metric systems| 1.5      | `m`    |
//! | abstract / hex     | 1        | `Unit` |
//!
//! Keeping the label free text rather than an enum is deliberate: a system
//! this crate has never heard of can label its grid `squares`, `hexes`,
//! `parsecs` or anything else without a change here.

/// How a scene converts cells into spoken distance.
#[derive(Clone, Debug, PartialEq)]
pub struct GridUnits {
    /// Distance one cell represents. Must be positive.
    pub per_cell: f32,
    /// What that distance is called — `ft`, `m`, `Unit`, `hexes`.
    pub label: String,
}

impl Default for GridUnits {
    /// D&D 5e's five-foot square, the most common tabletop default.
    fn default() -> Self {
        Self {
            per_cell: 5.0,
            label: "ft".to_string(),
        }
    }
}

impl GridUnits {
    pub fn new(per_cell: f32, label: impl Into<String>) -> Self {
        Self {
            per_cell,
            label: label.into(),
        }
    }

    /// A usable scale, guarding against zero or nonsense arriving from a
    /// scene's configuration.
    fn safe_per_cell(&self) -> f32 {
        if self.per_cell.is_finite() && self.per_cell > 0.0 {
            self.per_cell
        } else {
            Self::default().per_cell
        }
    }

    /// Distance covered by `cells`, in this scene's units.
    pub fn distance(&self, cells: f32) -> f32 {
        cells * self.safe_per_cell()
    }

    /// Distance as a label: `"15 ft"`, `"7.5 m"`, `"3 Unit"`.
    ///
    /// Trailing zeros are dropped — a half-cell step at 5ft is `2.5 ft`, but
    /// three whole cells is `15 ft`, never `15.0 ft`. Formatting a distance
    /// with spurious decimals is the sort of thing that makes a tool feel
    /// unfinished.
    pub fn format(&self, cells: f32) -> String {
        let distance = self.distance(cells);
        let rounded = (distance * 100.0).round() / 100.0;

        let number = if (rounded - rounded.round()).abs() < f32::EPSILON {
            format!("{}", rounded.round() as i64)
        } else {
            // Up to two decimals, then trim what is not needed.
            let mut text = format!("{rounded:.2}");
            while text.ends_with('0') {
                text.pop();
            }
            if text.ends_with('.') {
                text.pop();
            }
            text
        };

        if self.label.is_empty() {
            number
        } else {
            format!("{number} {}", self.label)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_is_a_five_foot_square() {
        let units = GridUnits::default();
        assert_eq!(units.format(1.0), "5 ft");
        assert_eq!(units.format(3.0), "15 ft");
    }

    #[test]
    fn a_scene_can_speak_any_ruleset() {
        assert_eq!(GridUnits::new(1.5, "m").format(4.0), "6 m");
        assert_eq!(GridUnits::new(1.0, "Unit").format(3.0), "3 Unit");
        assert_eq!(GridUnits::new(1.0, "hexes").format(7.0), "7 hexes");
        // A label this crate has never heard of works exactly the same.
        assert_eq!(GridUnits::new(10.0, "parsecs").format(2.0), "20 parsecs");
    }

    #[test]
    fn whole_distances_carry_no_decimal_point() {
        // "15.0 ft" reads as unfinished; "15 ft" is what a person says.
        assert_eq!(GridUnits::default().format(2.0), "10 ft");
        assert_eq!(GridUnits::new(1.5, "m").format(2.0), "3 m");
    }

    #[test]
    fn fractional_distances_keep_only_the_digits_they_need() {
        // A Tiny token's half-cell step.
        assert_eq!(GridUnits::default().format(0.5), "2.5 ft");
        assert_eq!(GridUnits::new(1.5, "m").format(1.0), "1.5 m");
        assert_eq!(GridUnits::new(1.5, "m").format(3.0), "4.5 m");
    }

    #[test]
    fn an_empty_label_yields_a_bare_number() {
        assert_eq!(GridUnits::new(2.0, "").format(3.0), "6");
    }

    #[test]
    fn a_nonsense_scale_falls_back_instead_of_reporting_zero() {
        // A scene misconfigured to 0 would otherwise report every move as
        // costing nothing.
        for bad in [0.0, -5.0, f32::NAN] {
            let units = GridUnits::new(bad, "ft");
            assert_eq!(units.format(3.0), "15 ft");
        }
    }

    #[test]
    fn zero_distance_is_reported_plainly() {
        assert_eq!(GridUnits::default().format(0.0), "0 ft");
    }
}
