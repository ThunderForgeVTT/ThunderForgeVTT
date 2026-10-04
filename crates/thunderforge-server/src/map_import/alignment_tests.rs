use super::*;

/// The shape the fix produces: 48 x 27 cells of 85px.
fn recorded() -> Value {
    record_source_map(None, 48.0, 27.0, 128.0)
}

#[test]
fn a_scene_whose_grid_matches_its_background_says_nothing() {
    assert_eq!(
        grid_mismatch(true, 4080, 2295, 85, Some(&recorded())),
        None,
        "48 x 85 = 4080 and 27 x 85 = 2295 — this is the corrected shape"
    );
}

/// The reported bug, as data.
///
/// These three numbers are self-consistent — 4096/128 is exactly 32 and
/// 2304/128 is exactly 18 — which is why nothing caught it and why the source
/// map size has to be recorded for the check to be possible at all.
#[test]
fn the_reported_bug_is_named_with_both_numbers() {
    let message = grid_mismatch(true, 4096, 2304, 128, Some(&recorded()))
        .expect("a 1.5x mis-scaled grid must be reported");

    assert!(message.contains("48"), "says what the map is: {message}");
    assert!(message.contains("32"), "says what was stored: {message}");
    assert!(message.contains("Re-import"), "says what to do: {message}");
}

#[test]
fn a_scene_with_no_background_is_never_complained_about() {
    // Grid size and dimensions are meaningless without one, and a blank scene
    // is not a broken scene.
    assert_eq!(
        grid_mismatch(false, 4096, 2304, 128, Some(&recorded())),
        None
    );
    assert_eq!(grid_mismatch(false, 0, 0, 0, None), None);
}

#[test]
fn a_scene_imported_before_this_was_recorded_falls_back_to_the_cap_fingerprint() {
    // No metadata, and a dimension sitting exactly on the texture cap: the
    // signature of the code path that stored a resized image beside an
    // unadjusted grid.
    let message = grid_mismatch(true, 4096, 2304, 128, None)
        .expect("a legacy capped background should be flagged");
    assert!(message.contains("Re-import"), "{message}");

    // And the same shape *with* a recorded source size is judged on the facts
    // instead — the heuristic is only for scenes that have none.
    let aligned = record_source_map(None, 32.0, 18.0, 128.0);
    assert_eq!(
        grid_mismatch(true, 4096, 2304, 128, Some(&aligned)),
        None,
        "a map that really is 32 x 18 at 128px is correct, cap or not"
    );
}

#[test]
fn a_legacy_scene_that_never_hit_the_cap_is_left_alone() {
    // azheim-meeting's shape: small enough that no resize happened, so its
    // grid was always right and there is nothing to warn about.
    assert_eq!(grid_mismatch(true, 2048, 2048, 256, None), None);
    assert_eq!(grid_mismatch(true, 1280, 1280, 128, None), None);
}

#[test]
fn an_unusable_grid_size_is_reported_rather_than_divided_by() {
    for grid_size in [0, -1] {
        let message = grid_mismatch(true, 4080, 2295, grid_size, Some(&recorded()))
            .expect("a zero or negative grid size must be reported");
        assert!(message.contains("Re-import"), "{message}");
    }
}

#[test]
fn recording_the_source_map_keeps_whatever_else_the_scene_carried() {
    // `scenes.metadata` is a shared bag. An import that dropped a neighbouring
    // key would be a data-loss bug wearing a bug fix's clothes.
    let existing = json!({ "fogOfWar": { "enabled": true }, "note": "keep me" });
    let merged = record_source_map(Some(existing), 48.0, 27.0, 128.0);

    assert_eq!(merged["fogOfWar"]["enabled"], json!(true));
    assert_eq!(merged["note"], json!("keep me"));
    assert_eq!(merged["mapImport"]["sourceMapCellsX"], json!(48.0));
}

#[test]
fn re_importing_replaces_the_previous_source_map_rather_than_merging_into_it() {
    let first = record_source_map(None, 48.0, 27.0, 128.0);
    let second = record_source_map(Some(first), 8.0, 8.0, 256.0);

    assert_eq!(second["mapImport"]["sourceMapCellsX"], json!(8.0));
    assert_eq!(second["mapImport"]["sourcePixelsPerGrid"], json!(256.0));
}

#[test]
fn metadata_that_is_not_an_object_is_replaced_rather_than_panicked_on() {
    let merged = record_source_map(Some(json!("unexpected")), 48.0, 27.0, 128.0);
    assert_eq!(merged["mapImport"]["sourceMapCellsY"], json!(27.0));
}

#[test]
fn a_scene_with_metadata_but_no_map_import_block_uses_the_fallback() {
    let unrelated = json!({ "fogOfWar": { "enabled": true } });
    assert!(grid_mismatch(true, 4096, 2304, 128, Some(&unrelated)).is_some());
    assert_eq!(grid_mismatch(true, 2048, 2048, 256, Some(&unrelated)), None);
}

/// The sentence a Game Master actually reads, pinned.
///
/// These are the numbers a live regression produced: the fix was reverted, a
/// map was imported through the real UI, and the scene came out 4096x2341 at
/// 128px with the file's own 35 x 20 recorded beside it. The row looks
/// plausible on its own — it is the recorded source size that makes it
/// provably wrong, which is the whole reason for writing it down.
#[test]
fn the_message_names_the_map_the_storage_and_the_remedy() {
    let recorded = record_source_map(None, 35.0, 20.0, 128.0);
    let message = grid_mismatch(true, 4096, 2341, 128, Some(&recorded))
        .expect("a regression must be reported");

    assert_eq!(
        message,
        "This scene's grid does not match its background: the map is 35 x 20 \
         squares, but the stored image covers 32.0 x 18.3. Re-import the map \
         to correct it."
    );
}
