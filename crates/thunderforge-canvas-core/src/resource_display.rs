//! What a token's resources are, and how much of that a given viewer is told.
//!
//! Spec 029. The rules for bars and counters — health, stamina, mana, or
//! whatever the active game system declares — live here rather than in the
//! engine crate for the usual reason: the engine's tests compile and never
//! run, so a rule placed there is untested by construction. These execute.
//!
//! # A resource is a list of entries, not a current-and-maximum pair
//!
//! The obvious model is `{ current, max }`, and it immediately raises a
//! question it cannot answer: what does a value above the maximum mean?
//! Temporary hit points, a shield, the second stage of a boss — all real, all
//! expressible only as "more than full", which then needs a rule about
//! clamping that will be wrong for at least one of them.
//!
//! [`ResourceEntry`] removes the question. Overflow is not a value exceeding
//! a bound; it is a further entry. A boss with three stages is three entries.
//! A shield is an entry stacked above the base pool. Damage takes the topmost
//! first. There is no state in which a value exceeds its maximum, so nothing
//! has to decide what to do about one.
//!
//! # Disclosure is part of the model, not a filter over it
//!
//! A bar is a disclosure channel: a player watching a boss's health bar learns
//! something whether or not anybody meant them to. So what a viewer is told is
//! decided here and applied on the server — see [`Disclosed`] — and the client
//! receives only the shape its state permits. A client that never receives a
//! figure cannot leak one.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// How a resource is presented.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../apps/web/src/engine/sdk/")]
#[serde(rename_all = "camelCase")]
pub enum ResourceKind {
    /// Has a maximum, and is drawn as a proportion of it.
    Bar,
    /// Has no maximum. A count, drawn as a number.
    Counter,
}

/// What a game system declares it tracks.
///
/// The engine holds no built-in notion of "health": one system tracks hit
/// points, another health/stamina/mana, a third health/energy. Hard-coding the
/// first would make every system after it a special case.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../apps/web/src/engine/sdk/")]
#[serde(rename_all = "camelCase")]
pub struct ResourceDefinition {
    pub id: String,
    pub label: String,
    pub kind: ResourceKind,
    /// Display order. The engine imposes none.
    pub order: i32,
    /// Whether more than one entry is permitted.
    pub allow_stacking: bool,
}

/// One layer of a resource: a pool with its own maximum.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../apps/web/src/engine/sdk/")]
#[serde(rename_all = "camelCase")]
pub struct ResourceEntry {
    pub current: i32,
    /// Absent for a counter, which has no maximum to be a proportion of.
    pub max: Option<i32>,
    /// Optional name for this layer — "Shield", "Stage 2".
    pub label: Option<String>,
}

/// Why a set of entries could not be accepted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryError {
    /// More than one entry where the definition forbids stacking.
    StackingNotAllowed { got: usize },
    /// A current value outside `0..=max`.
    ///
    /// Not a state to clamp. Because overflow is a further entry, a value
    /// above its own entry's maximum cannot arise from ordinary play, so it
    /// means something upstream is wrong and should say so.
    ValueOutOfRange {
        index: usize,
        current: i32,
        max: i32,
    },
}

/// Check a set of entries against its definition.
pub fn validate_entries(
    definition: &ResourceDefinition,
    entries: &[ResourceEntry],
) -> Result<(), EntryError> {
    if !definition.allow_stacking && entries.len() > 1 {
        return Err(EntryError::StackingNotAllowed { got: entries.len() });
    }

    for (index, entry) in entries.iter().enumerate() {
        if let Some(max) = entry.max
            && (entry.current < 0 || entry.current > max)
        {
            return Err(EntryError::ValueOutOfRange {
                index,
                current: entry.current,
                max,
            });
        }
    }

    Ok(())
}

/// Apply `amount` of depletion, consuming the topmost entry first.
///
/// Spent entries stay in the list at zero rather than being removed: a boss on
/// its last stage should still read as being on its *last* stage, and that
/// needs the exhausted ones to remain visible.
///
/// Returns any amount that could not be absorbed.
pub fn deplete(entries: &mut [ResourceEntry], mut amount: i32) -> i32 {
    for entry in entries.iter_mut().rev() {
        if amount <= 0 {
            break;
        }
        let taken = amount.min(entry.current.max(0));
        entry.current -= taken;
        amount -= taken;
    }
    amount
}

/// Total current across every entry.
pub fn total_current(entries: &[ResourceEntry]) -> i32 {
    entries.iter().map(|e| e.current).sum()
}

/// Total maximum across every entry that has one.
pub fn total_max(entries: &[ResourceEntry]) -> i32 {
    entries.iter().filter_map(|e| e.max).sum()
}

/// What a viewer other than the Game Master is permitted to learn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../apps/web/src/engine/sdk/")]
#[serde(rename_all = "camelCase")]
pub enum DisclosureState {
    /// The exact entries.
    Visible,
    /// That the resource exists, and nothing else.
    ///
    /// The honest form of "hidden": removing the bar entirely also discloses
    /// something, because a token conspicuously lacking a bar every other
    /// token has is itself a signal.
    Greyed,
    /// A proportion, with no maximum.
    ///
    /// Discloses more than it appears to. A viewer who knows the damage they
    /// dealt can divide it by the change, recover the maximum, and read exact
    /// values from then on. Offered because a readable boss fight is a
    /// legitimate thing to want — but not equivalent in safety to
    /// [`DisclosureState::Chunked`], which rarely moves on a single hit.
    Percentage,
    /// The proportion rounded down to quarters.
    Chunked,
}

/// A resource as one viewer receives it.
///
/// Tagged on `disclosure`, so the shape carries exactly the one field its
/// state permits and no other. An over-disclosing payload is unrepresentable
/// rather than forbidden by a rule somebody has to remember — and on the
/// TypeScript side this generates a discriminated union that narrows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../apps/web/src/engine/sdk/")]
#[serde(tag = "disclosure", rename_all = "camelCase")]
pub enum Disclosed {
    Visible { entries: Vec<ResourceEntry> },
    Greyed,
    Percentage { proportion: f32 },
    Chunked { quarter: u8 },
}

/// Fraction of the pool remaining, 0.0–1.0.
///
/// A resource with no maximum anywhere — a pure counter — has no proportion to
/// report and answers `None`.
pub fn proportion(entries: &[ResourceEntry]) -> Option<f32> {
    let max = total_max(entries);
    if max <= 0 {
        return None;
    }
    let current = total_current(entries).clamp(0, max);
    Some(current as f32 / max as f32)
}

/// Which quarter the pool sits in: 0 (empty) through 4 (full).
///
/// Rounds **down**, so anything short of full reads as less than full and only
/// a genuinely empty pool reads as empty. Rounding to nearest would show a
/// creature at 88% as "full" and one at 12% as "empty", both of which are lies
/// a player would act on.
pub fn quarter(entries: &[ResourceEntry]) -> Option<u8> {
    let fraction = proportion(entries)?;
    Some(match fraction {
        f if f <= 0.0 => 0,
        f if f >= 1.0 => 4,
        f => (f * 4.0).floor() as u8,
    })
}

/// Reduce a set of entries to what `state` permits a viewer to see.
///
/// This is the function the server calls. Everything it returns is safe to put
/// on the wire; everything it drops never leaves the server.
pub fn disclose(entries: &[ResourceEntry], state: DisclosureState) -> Disclosed {
    match state {
        DisclosureState::Visible => Disclosed::Visible {
            entries: entries.to_vec(),
        },
        DisclosureState::Greyed => Disclosed::Greyed,
        DisclosureState::Percentage => Disclosed::Percentage {
            proportion: proportion(entries).unwrap_or(0.0),
        },
        DisclosureState::Chunked => Disclosed::Chunked {
            quarter: quarter(entries).unwrap_or(0),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bar(id: &str, allow_stacking: bool) -> ResourceDefinition {
        ResourceDefinition {
            id: id.to_string(),
            label: id.to_string(),
            kind: ResourceKind::Bar,
            order: 0,
            allow_stacking,
        }
    }

    fn entry(current: i32, max: i32) -> ResourceEntry {
        ResourceEntry {
            current,
            max: Some(max),
            label: None,
        }
    }

    // --- entries and stacking -------------------------------------------

    #[test]
    fn a_single_entry_is_always_acceptable() {
        assert_eq!(validate_entries(&bar("hp", false), &[entry(5, 10)]), Ok(()));
    }

    #[test]
    fn a_second_entry_is_refused_where_stacking_is_forbidden() {
        // Refused rather than merged: merging loses which pool was temporary,
        // and a shield that silently becomes health is a rules bug.
        assert_eq!(
            validate_entries(&bar("hp", false), &[entry(10, 10), entry(5, 5)]),
            Err(EntryError::StackingNotAllowed { got: 2 })
        );
    }

    #[test]
    fn a_second_entry_is_accepted_where_stacking_is_allowed() {
        assert_eq!(
            validate_entries(&bar("hp", true), &[entry(10, 10), entry(5, 5)]),
            Ok(())
        );
    }

    /// The state the entry model exists to make impossible.
    #[test]
    fn a_value_above_its_own_maximum_is_an_error_not_a_state() {
        assert_eq!(
            validate_entries(&bar("hp", true), &[entry(12, 10)]),
            Err(EntryError::ValueOutOfRange {
                index: 0,
                current: 12,
                max: 10
            })
        );
    }

    #[test]
    fn a_negative_value_is_refused() {
        assert!(validate_entries(&bar("hp", true), &[entry(-1, 10)]).is_err());
    }

    // --- depletion -------------------------------------------------------

    #[test]
    fn depletion_consumes_the_topmost_entry_first() {
        // A shield stacked over health: the shield goes first.
        let mut entries = vec![entry(10, 10), entry(5, 5)];
        let unabsorbed = deplete(&mut entries, 3);

        assert_eq!(unabsorbed, 0);
        assert_eq!(entries[1].current, 2, "the shield took it");
        assert_eq!(entries[0].current, 10, "health is untouched");
    }

    #[test]
    fn depletion_spills_into_the_entry_below_once_the_top_is_spent() {
        let mut entries = vec![entry(10, 10), entry(5, 5)];
        deplete(&mut entries, 8);

        assert_eq!(entries[1].current, 0);
        assert_eq!(entries[0].current, 7);
    }

    /// A boss on its last stage must still read as being on its *last* stage.
    #[test]
    fn a_spent_entry_stays_in_the_list_rather_than_disappearing() {
        let mut entries = vec![entry(100, 100), entry(100, 100), entry(100, 100)];
        deplete(&mut entries, 250);

        assert_eq!(entries.len(), 3, "three stages, still three entries");
        assert_eq!(entries[2].current, 0);
        assert_eq!(entries[1].current, 0);
        assert_eq!(entries[0].current, 50);
    }

    #[test]
    fn damage_beyond_the_whole_pool_is_reported_rather_than_swallowed() {
        let mut entries = vec![entry(5, 10)];
        assert_eq!(deplete(&mut entries, 8), 3);
        assert_eq!(entries[0].current, 0);
    }

    // --- banding ---------------------------------------------------------

    #[test]
    fn a_full_pool_is_the_top_quarter_and_an_empty_one_is_the_bottom() {
        assert_eq!(quarter(&[entry(10, 10)]), Some(4));
        assert_eq!(quarter(&[entry(0, 10)]), Some(0));
    }

    /// Rounding down, tested at the boundary it matters on.
    #[test]
    fn banding_rounds_down_so_nearly_full_never_reads_as_full() {
        assert_eq!(quarter(&[entry(99, 100)]), Some(3), "99% is not full");
        assert_eq!(
            quarter(&[entry(75, 100)]),
            Some(3),
            "exactly three quarters"
        );
        assert_eq!(quarter(&[entry(74, 100)]), Some(2));
        assert_eq!(quarter(&[entry(25, 100)]), Some(1), "exactly one quarter");
        assert_eq!(
            quarter(&[entry(1, 100)]),
            Some(0),
            "1% is not empty-looking by accident — it is the lowest band"
        );
    }

    #[test]
    fn banding_spans_every_entry_rather_than_only_the_top_one() {
        // Two stages, the top one spent: half the total pool remains.
        assert_eq!(quarter(&[entry(100, 100), entry(0, 100)]), Some(2));
    }

    #[test]
    fn a_pool_with_no_maximum_has_no_band() {
        let counter = ResourceEntry {
            current: 3,
            max: None,
            label: None,
        };
        assert_eq!(quarter(&[counter.clone()]), None);
        assert_eq!(proportion(&[counter]), None);
    }

    // --- disclosure ------------------------------------------------------

    /// The property the whole disclosure model rests on: each state yields
    /// exactly the one field it permits, and never a second.
    #[test]
    fn each_state_yields_only_what_it_permits() {
        let entries = vec![entry(30, 100)];

        match disclose(&entries, DisclosureState::Visible) {
            Disclosed::Visible { entries: e } => assert_eq!(e.len(), 1),
            other => panic!("expected Visible, got {other:?}"),
        }

        assert_eq!(
            disclose(&entries, DisclosureState::Greyed),
            Disclosed::Greyed,
            "greyed carries no value, no maximum, no proportion"
        );

        match disclose(&entries, DisclosureState::Percentage) {
            Disclosed::Percentage { proportion } => {
                assert!((proportion - 0.3).abs() < f32::EPSILON);
            }
            other => panic!("expected Percentage, got {other:?}"),
        }

        match disclose(&entries, DisclosureState::Chunked) {
            Disclosed::Chunked { quarter } => assert_eq!(quarter, 1),
            other => panic!("expected Chunked, got {other:?}"),
        }
    }

    /// Serialised, the coarse states must not carry the figures they hide.
    ///
    /// This is the assertion that matters: the type makes over-disclosure
    /// unrepresentable, and this proves the serialised form agrees.
    #[test]
    fn the_serialised_form_of_a_coarse_state_contains_no_exact_figure() {
        let entries = vec![entry(37, 250)];

        let chunked = serde_json::to_string(&disclose(&entries, DisclosureState::Chunked)).unwrap();
        assert!(!chunked.contains("37"), "exact current leaked: {chunked}");
        assert!(!chunked.contains("250"), "maximum leaked: {chunked}");
        assert!(chunked.contains("quarter"));

        let greyed = serde_json::to_string(&disclose(&entries, DisclosureState::Greyed)).unwrap();
        assert!(
            !greyed.contains("37") && !greyed.contains("250"),
            "{greyed}"
        );

        let percentage =
            serde_json::to_string(&disclose(&entries, DisclosureState::Percentage)).unwrap();
        assert!(
            !percentage.contains("250"),
            "percentage must not carry the maximum: {percentage}"
        );
    }

    #[test]
    fn the_tag_is_the_discriminant_so_the_shape_is_self_describing() {
        let json = serde_json::to_string(&Disclosed::Greyed).unwrap();
        assert_eq!(json, r#"{"disclosure":"greyed"}"#);
    }
}
