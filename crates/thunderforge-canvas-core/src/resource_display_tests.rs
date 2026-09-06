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
    assert_eq!(quarter(std::slice::from_ref(&counter)), None);
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

// --- reading entries out of a system's stored data --------------------

fn genie_health() -> ResourceSource {
    ResourceSource {
        slot: "resourceData".into(),
        entries: vec![EntrySource {
            current: "current_health".into(),
            max: Some("max_health".into()),
            label: None,
            max_value: None,
            optional: false,
        }],
    }
}

/// D&D 5e's shape, and the reason `allowStacking` has a real caller.
///
/// That system represents temporary hit points by letting `current_hp`
/// exceed `max_hp` — its own validator notes the case and permits it. That
/// is precisely the "value above its maximum" ambiguity the entry model
/// removes: temp HP is a second layer, not an overflowing first one.
fn dnd5e_hit_points() -> ResourceSource {
    ResourceSource {
        slot: "resourceData".into(),
        entries: vec![
            EntrySource {
                current: "current_hp".into(),
                max: Some("max_hp".into()),
                label: None,
                max_value: None,
                optional: false,
            },
            EntrySource {
                current: "temporary_hp".into(),
                max: None,
                label: Some("Temporary".into()),
                max_value: None,
                optional: true,
            },
        ],
    }
}

#[test]
fn a_single_pool_reads_its_two_named_fields() {
    let stored = serde_json::json!({ "current_health": 7, "max_health": 12 });
    let entries = entries_from(&stored, &genie_health());

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].current, 7);
    assert_eq!(entries[0].max, Some(12));
}

#[test]
fn temporary_hit_points_become_a_second_entry_rather_than_an_overflow() {
    let stored = serde_json::json!({
        "current_hp": 20, "max_hp": 20, "temporary_hp": 5
    });
    let entries = entries_from(&stored, &dnd5e_hit_points());

    assert_eq!(entries.len(), 2, "base pool plus the temporary layer");
    assert_eq!(entries[1].current, 5);
    assert_eq!(entries[1].max, None, "granted, not capped");
    assert_eq!(entries[1].label.as_deref(), Some("Temporary"));

    // And the combination is legal, where `current 25 / max 20` was not.
    let definition = bar("hp", true);
    assert_eq!(validate_entries(&definition, &entries), Ok(()));
}

#[test]
fn an_absent_optional_layer_produces_no_entry() {
    let stored = serde_json::json!({ "current_hp": 14, "max_hp": 20 });
    let entries = entries_from(&stored, &dnd5e_hit_points());

    assert_eq!(
        entries.len(),
        1,
        "no empty Temporary layer on every character"
    );
}

#[test]
fn a_zeroed_optional_layer_is_also_omitted() {
    let stored = serde_json::json!({
        "current_hp": 14, "max_hp": 20, "temporary_hp": 0
    });
    assert_eq!(entries_from(&stored, &dnd5e_hit_points()).len(), 1);
}

#[test]
fn a_missing_required_field_yields_no_entry_rather_than_a_zero() {
    // A zero would draw an empty bar, which claims the creature is at
    // zero — a far stronger statement than "this system stored nothing".
    let stored = serde_json::json!({ "max_health": 12 });
    assert!(entries_from(&stored, &genie_health()).is_empty());
}

#[test]
fn a_non_numeric_field_is_ignored_rather_than_guessed_at() {
    let stored = serde_json::json!({ "current_health": "lots", "max_health": 12 });
    assert!(entries_from(&stored, &genie_health()).is_empty());
}

/// Blades in the Dark caps stress at nine, and no character stores that
/// nine anywhere — it is a rule, not data.
#[test]
fn a_maximum_fixed_by_the_rules_still_makes_a_bar() {
    let stress = ResourceSource {
        slot: "resourceData".into(),
        entries: vec![EntrySource {
            current: "stress".into(),
            max: None,
            max_value: Some(9),
            label: None,
            optional: false,
        }],
    };
    let stored = serde_json::json!({ "stress": 6 });
    let entries = entries_from(&stored, &stress);

    assert_eq!(entries[0].current, 6);
    assert_eq!(entries[0].max, Some(9), "the cap comes from the rules");
    // And it is therefore a proportion rather than a bare count, which is
    // the thing a player actually needs: how close to nine they are.
    assert_eq!(quarter(&entries), Some(2));
}

#[test]
fn a_stored_maximum_beats_a_rules_maximum() {
    // A stored value describes *this* character; a literal describes
    // everyone. When a system offers both, the specific one wins.
    let source = ResourceSource {
        slot: "resourceData".into(),
        entries: vec![EntrySource {
            current: "current_hp".into(),
            max: Some("max_hp".into()),
            max_value: Some(10),
            label: None,
            optional: false,
        }],
    };
    let stored = serde_json::json!({ "current_hp": 30, "max_hp": 40 });
    assert_eq!(entries_from(&stored, &source)[0].max, Some(40));
}

/// A counter has no maximum from either source, and must stay a count.
#[test]
fn a_counter_with_no_maximum_anywhere_reports_no_proportion() {
    let coin = ResourceSource {
        slot: "resourceData".into(),
        entries: vec![EntrySource {
            current: "coin".into(),
            max: None,
            max_value: None,
            label: None,
            optional: false,
        }],
    };
    let entries = entries_from(&serde_json::json!({ "coin": 3 }), &coin);
    assert_eq!(entries[0].current, 3);
    assert_eq!(entries[0].max, None);
    assert_eq!(proportion(&entries), None, "a count is not a fraction");
}

// --- the derived default ---------------------------------------------

#[test]
fn a_player_reads_their_own_character_exactly() {
    assert_eq!(
        default_disclosure(TokenSubject::OwnCharacter),
        DisclosureState::Visible
    );
}

#[test]
fn party_members_read_each_other_exactly() {
    // A table shares this. Coarsening it would make the party worse at
    // coordinating than four people sitting round an actual table.
    assert_eq!(
        default_disclosure(TokenSubject::PartyCharacter),
        DisclosureState::Visible
    );
}

#[test]
fn an_npc_is_chunked_rather_than_exact_or_hidden() {
    // Chunked rather than greyed: a board where every NPC bar is blank is
    // a board that gives players nothing, and they will ask the GM for the
    // number instead — which is worse than telling them a quarter band.
    assert_eq!(
        default_disclosure(TokenSubject::NonPlayerCharacter),
        DisclosureState::Chunked
    );
}

/// The default never discloses more than the GM could have chosen.
#[test]
fn no_derived_default_is_more_revealing_than_visible() {
    for subject in [
        TokenSubject::OwnCharacter,
        TokenSubject::PartyCharacter,
        TokenSubject::NonPlayerCharacter,
    ] {
        let state = default_disclosure(subject);
        assert!(
            matches!(
                state,
                DisclosureState::Visible
                    | DisclosureState::Chunked
                    | DisclosureState::Percentage
                    | DisclosureState::Greyed
            ),
            "{subject:?} produced an unexpected state"
        );
    }
}

#[test]
fn the_tag_is_the_discriminant_so_the_shape_is_self_describing() {
    let json = serde_json::to_string(&Disclosed::Greyed).unwrap();
    assert_eq!(json, r#"{"disclosure":"greyed"}"#);
}

/// The property the default palette exists for (FR-024, SC-007).
///
/// Two bars that look alike are worse than one bar, because they promise
/// a distinction they do not deliver — and unlike token kinds, these sit
/// stacked and touching, where a viewer compares them directly. Roughly
/// one man in twelve has a red-green deficiency; a health bar and a
/// stamina bar that collapse for them collapse in the middle of a fight.
///
/// Deliberately the same thresholds as `token_kind`, so one standard
/// governs everything drawn on the canvas rather than each feature
/// inventing its own idea of "different enough".
#[test]
fn every_pair_in_the_default_palette_is_distinguishable() {
    fn separation(a: Rgb, b: Rgb) -> f32 {
        let (dr, dg, db) = (a.0 - b.0, a.1 - b.1, a.2 - b.2);
        dr * dr + dg * dg + db * db
    }

    let palette = DisplayAppearance::default().palette;
    assert_eq!(palette.len(), DEFAULT_PALETTE_LEN);

    for (i, a) in palette.iter().enumerate() {
        for (j, b) in palette.iter().enumerate().skip(i + 1) {
            let rgb_gap = separation(*a, *b);
            assert!(
                rgb_gap > 0.05,
                "slots {i} and {j} are too close in colour ({rgb_gap:.3})"
            );

            let luma_gap = (luma(*a) - luma(*b)).abs();
            assert!(
                luma_gap > 0.05,
                "slots {i} and {j} differ by only {luma_gap:.3} in \
                 lightness — they would collapse for a viewer who cannot \
                 use hue"
            );
        }
    }
}

/// The withheld colour must not read as one of the real ones.
///
/// This is the pair that matters most and that the loop above does not
/// cover: a coarsened bar says "you are not being told this", and if it
/// looks like a resource fill then the player reads a value that was
/// deliberately withheld as a real one.
#[test]
fn the_undisclosed_fill_is_not_mistakable_for_a_resource() {
    let appearance = DisplayAppearance::default();
    for (i, colour) in appearance.palette.iter().enumerate() {
        let gap = (luma(*colour) - luma(appearance.undisclosed)).abs();
        assert!(
            gap > 0.04,
            "slot {i} is within {gap:.3} lightness of the withheld fill"
        );
    }
}

/// A system may declare more resources than the palette has slots.
#[test]
fn the_palette_wraps_rather_than_running_out() {
    let appearance = DisplayAppearance::default();
    assert_eq!(appearance.fill_for(0), appearance.fill_for(4));
    assert_eq!(appearance.fill_for(1), appearance.fill_for(5));
    // And never silently becomes the withheld colour, which would mean
    // something else entirely.
    for order in 0..12 {
        assert_ne!(appearance.fill_for(order), appearance.undisclosed);
    }
}

/// An application may legitimately clear the palette; that must not panic.
#[test]
fn an_empty_palette_falls_back_instead_of_panicking() {
    let appearance = DisplayAppearance {
        palette: Vec::new(),
        ..DisplayAppearance::default()
    };
    assert_eq!(appearance.fill_for(0), appearance.undisclosed);
}

/// The property partial overrides exist for.
///
/// The wrong implementation folds each override onto the *defaults*
/// rather than onto what is currently in effect. It is indistinguishable
/// from the right one for a single call, and silently discards the first
/// of any two — so an application that sets its palette at startup and
/// its bar height later would lose the palette, with nothing to explain
/// where it went.
#[test]
fn successive_overrides_accumulate_rather_than_replacing() {
    let mut appearance = DisplayAppearance::default();
    let default_palette = appearance.palette.clone();

    AppearanceOverride {
        bar_height: Some(14.0),
        ..Default::default()
    }
    .apply_to(&mut appearance);

    AppearanceOverride {
        bar_gap: Some(6.0),
        ..Default::default()
    }
    .apply_to(&mut appearance);

    assert_eq!(appearance.bar_height, 14.0, "the first override survived");
    assert_eq!(appearance.bar_gap, 6.0, "and the second applied");
    assert_eq!(
        appearance.palette, default_palette,
        "a field nobody mentioned must be left alone, not reset"
    );
}

/// An empty override asks for nothing and must change nothing.
#[test]
fn an_empty_override_is_a_no_op() {
    let mut appearance = DisplayAppearance::default();
    AppearanceOverride::default().apply_to(&mut appearance);
    assert_eq!(appearance, DisplayAppearance::default());
}

/// Every field must actually be wired through.
///
/// A field added to the struct and forgotten in `apply_to` compiles
/// perfectly and is simply ignored for ever — the exact silent-drop
/// failure this spec exists to retire, reintroduced one field at a time.
#[test]
fn every_field_of_an_override_reaches_the_appearance() {
    let mut appearance = DisplayAppearance::default();
    AppearanceOverride {
        track: Some((0.1, 0.2, 0.3)),
        track_alpha: Some(0.5),
        undisclosed: Some((0.4, 0.5, 0.6)),
        palette: Some(vec![(0.7, 0.8, 0.9)]),
        bar_height: Some(11.0),
        bar_gap: Some(2.0),
        first_bar_offset: Some(20.0),
    }
    .apply_to(&mut appearance);

    assert_eq!(appearance.track, (0.1, 0.2, 0.3));
    assert_eq!(appearance.track_alpha, 0.5);
    assert_eq!(appearance.undisclosed, (0.4, 0.5, 0.6));
    assert_eq!(appearance.palette, vec![(0.7, 0.8, 0.9)]);
    assert_eq!(appearance.bar_height, 11.0);
    assert_eq!(appearance.bar_gap, 2.0);
    assert_eq!(appearance.first_bar_offset, 20.0);
}

/// FR-014: an estimate must not be mistakable for a reading.
///
/// The bug this replaces was quiet and complete: percentage and chunked
/// both reported themselves as "disclosed", so a bar showing "somewhere in
/// the second quarter" was drawn in exactly the same colour, at a width
/// derived from the band, as one showing a real figure. A player had no
/// way to tell an estimate from a measurement.
#[test]
fn a_coarse_fill_is_distinguishable_from_an_exact_one() {
    fn separation(a: Rgb, b: Rgb) -> f32 {
        let (dr, dg, db) = (a.0 - b.0, a.1 - b.1, a.2 - b.2);
        dr * dr + dg * dg + db * db
    }

    let appearance = DisplayAppearance::default();
    for (i, base) in appearance.palette.iter().enumerate() {
        let exact = fill_for_precision(*base, appearance.undisclosed, Precision::Exact);
        let coarse = fill_for_precision(*base, appearance.undisclosed, Precision::Coarse);

        let gap = (luma(exact) - luma(coarse)).abs() + separation(exact, coarse);
        assert!(
            gap > 0.02,
            "slot {i}: a coarse fill is only {gap:.4} from its exact one — \
             an estimate would read as a measurement"
        );
    }
}

/// And still identifiable as the resource it belongs to.
///
/// The opposite failure, and just as bad: if every coarse bar collapsed
/// toward the same grey, a player could no longer tell which resource was
/// being estimated — so coarsening health would look like coarsening mana.
#[test]
fn coarse_fills_remain_distinguishable_from_each_other() {
    fn separation(a: Rgb, b: Rgb) -> f32 {
        let (dr, dg, db) = (a.0 - b.0, a.1 - b.1, a.2 - b.2);
        dr * dr + dg * dg + db * db
    }

    let appearance = DisplayAppearance::default();
    let coarse: Vec<Rgb> = appearance
        .palette
        .iter()
        .map(|c| fill_for_precision(*c, appearance.undisclosed, Precision::Coarse))
        .collect();

    for (i, a) in coarse.iter().enumerate() {
        for (j, b) in coarse.iter().enumerate().skip(i + 1) {
            let gap = separation(*a, *b);
            assert!(
                gap > 0.01,
                "coarse slots {i} and {j} are only {gap:.4} apart — \
                 coarsening would hide which resource is which"
            );
        }
    }
}

/// A withheld fill is the withheld colour, whatever resource it belongs to.
#[test]
fn a_withheld_fill_does_not_depend_on_the_resource() {
    let appearance = DisplayAppearance::default();
    for base in &appearance.palette {
        assert_eq!(
            fill_for_precision(*base, appearance.undisclosed, Precision::Withheld),
            appearance.undisclosed,
            "a withheld bar that kept its resource colour would say which \
             resource is being hidden, and how many there are"
        );
    }
}

/// FR-016, stated as a property rather than an audit.
///
/// A withheld bar is drawn full whatever is behind it. There is nothing to
/// vary it *with* — `Greyed` carries no value — so this asserts the shape
/// the guarantee rests on rather than sampling a few figures and hoping.
#[test]
fn a_withheld_bar_is_the_same_bar_whatever_it_hides() {
    let (fraction, precision) = bar_fill(&Disclosed::Greyed);
    assert_eq!(fraction, 1.0);
    assert_eq!(precision, Precision::Withheld);
}

/// A coarse bar reports itself as coarse, at every band.
#[test]
fn every_coarse_band_is_reported_as_coarse() {
    for quarter in 0..=4u8 {
        let (fraction, precision) = bar_fill(&Disclosed::Chunked { quarter });
        assert_eq!(
            precision,
            Precision::Coarse,
            "quarter {quarter} must not claim to be a reading"
        );
        assert!((0.0..=1.0).contains(&fraction));
    }

    for proportion in [0.0, 0.33, 1.0, 2.5, -1.0] {
        let (fraction, precision) = bar_fill(&Disclosed::Percentage { proportion });
        assert_eq!(precision, Precision::Coarse);
        assert!(
            (0.0..=1.0).contains(&fraction),
            "a proportion outside 0-1 must be clamped, not drawn past the track"
        );
    }
}

/// A quarter is drawn at the bottom of its band, never the top.
///
/// Drawing quarter 1 as half-full would tell a player more than the band
/// contains, which is the coarsening undone at the last step.
#[test]
fn a_quarter_never_reads_as_more_than_its_band() {
    for quarter in 0..=4u8 {
        let (fraction, _) = bar_fill(&Disclosed::Chunked { quarter });
        assert_eq!(fraction, quarter as f32 / 4.0);
    }
}
