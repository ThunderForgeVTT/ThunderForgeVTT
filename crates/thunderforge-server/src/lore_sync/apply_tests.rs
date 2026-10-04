//! FR-032 and FR-010, exercised against a real directory.
//!
//! No database and no remote: these are claims about files, and the cheapest
//! honest way to test a claim about files is to make some.

use std::collections::HashMap;

use super::*;
use crate::lore_sync::plan::{PlannedFile, PlannedImage};

fn temp_dir() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("tf-apply-{}", Uuid::now_v7()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

fn plan_with(files: Vec<(Uuid, &str, &str)>) -> Plan {
    Plan {
        files: files
            .into_iter()
            .map(|(entry_id, path, contents)| PlannedFile {
                entry_id,
                path: path.to_string(),
                contents: contents.to_string(),
            })
            .collect(),
        images: Vec::new(),
        notes: Vec::new(),
    }
}

fn no_images(_: &str) -> Option<Vec<u8>> {
    None
}

#[test]
fn a_plan_becomes_files_in_directories() {
    let dir = temp_dir();
    let entry = Uuid::now_v7();
    let plan = plan_with(vec![(entry, "westeros/the-red-keep.md", "A castle.")]);

    let changes = apply(&dir, &plan, &HashMap::new(), &no_images).expect("no collision");

    assert_eq!(changes.written, vec!["westeros/the-red-keep.md"]);
    assert_eq!(
        std::fs::read_to_string(dir.join("westeros/the-red-keep.md")).unwrap(),
        "A castle."
    );
    std::fs::remove_dir_all(&dir).ok();
}

/// **The rule this module exists for.** The natural implementation of "make
/// the directory match the plan" deletes everything not in the plan, which in
/// a repository the user owns is data loss dressed as correctness.
#[test]
fn a_file_we_never_wrote_is_never_removed() {
    let dir = temp_dir();
    std::fs::write(dir.join("README.md"), "the user's own notes").unwrap();
    std::fs::create_dir_all(dir.join("other-tool")).unwrap();
    std::fs::write(dir.join("other-tool/output.txt"), "not ours").unwrap();

    let entry = Uuid::now_v7();
    let plan = plan_with(vec![(entry, "an-entry.md", "ours")]);

    let changes = apply(&dir, &plan, &HashMap::new(), &no_images).expect("no collision");

    assert!(changes.removed.is_empty(), "we removed something of theirs");
    assert_eq!(
        std::fs::read_to_string(dir.join("README.md")).unwrap(),
        "the user's own notes"
    );
    assert!(dir.join("other-tool/output.txt").exists());
    std::fs::remove_dir_all(&dir).ok();
}

/// FR-032's collision. Both resolutions are wrong — overwriting destroys the
/// user's file, skipping silently produces a mirror that is quietly
/// incomplete — so the pass stops and says so.
#[test]
fn a_collision_with_someone_elses_file_stops_the_pass() {
    let dir = temp_dir();
    std::fs::write(dir.join("an-entry.md"), "the user wrote this first").unwrap();

    let entry = Uuid::now_v7();
    let plan = plan_with(vec![(entry, "an-entry.md", "ours")]);

    let outcome = apply(&dir, &plan, &HashMap::new(), &no_images);

    assert_eq!(
        outcome.unwrap_err(),
        Collision {
            path: "an-entry.md".to_string()
        }
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("an-entry.md")).unwrap(),
        "the user wrote this first",
        "the collision overwrote what it collided with",
    );
    std::fs::remove_dir_all(&dir).ok();
}

/// Rewriting a file we already own is not a collision — otherwise the second
/// pass over any world would stop.
#[test]
fn rewriting_our_own_file_is_not_a_collision() {
    let dir = temp_dir();
    let entry = Uuid::now_v7();
    let plan = plan_with(vec![(entry, "an-entry.md", "v1")]);
    apply(&dir, &plan, &HashMap::new(), &no_images).expect("first pass");

    let previous = HashMap::from([(entry, "an-entry.md".to_string())]);
    let plan2 = plan_with(vec![(entry, "an-entry.md", "v2")]);
    let changes = apply(&dir, &plan2, &previous, &no_images).expect("second pass");

    assert_eq!(changes.written, vec!["an-entry.md"]);
    assert_eq!(
        std::fs::read_to_string(dir.join("an-entry.md")).unwrap(),
        "v2"
    );
    std::fs::remove_dir_all(&dir).ok();
}

/// FR-010. Git works a rename out from content similarity, but only if the old
/// path is gone and the new one present in the same commit. A delete plus an
/// unrelated create truncates the file's history at the rename.
#[test]
fn a_rename_moves_the_file_rather_than_recreating_it() {
    let dir = temp_dir();
    let entry = Uuid::now_v7();
    apply(
        &dir,
        &plan_with(vec![(entry, "old-name.md", "body")]),
        &HashMap::new(),
        &no_images,
    )
    .expect("first pass");

    let previous = HashMap::from([(entry, "old-name.md".to_string())]);
    let changes = apply(
        &dir,
        &plan_with(vec![(entry, "new-name.md", "body")]),
        &previous,
        &no_images,
    )
    .expect("rename");

    assert_eq!(
        changes.moved,
        vec![("old-name.md".to_string(), "new-name.md".to_string())]
    );
    assert!(!dir.join("old-name.md").exists(), "the old path survived");
    assert_eq!(
        std::fs::read_to_string(dir.join("new-name.md")).unwrap(),
        "body"
    );
    std::fs::remove_dir_all(&dir).ok();
}

/// A move into a directory that does not exist yet — reparenting to a new
/// branch of the tree.
#[test]
fn a_move_creates_the_directories_it_needs() {
    let dir = temp_dir();
    let entry = Uuid::now_v7();
    apply(
        &dir,
        &plan_with(vec![(entry, "loose.md", "body")]),
        &HashMap::new(),
        &no_images,
    )
    .expect("first pass");

    let previous = HashMap::from([(entry, "loose.md".to_string())]);
    apply(
        &dir,
        &plan_with(vec![(entry, "a/b/c/loose.md", "body")]),
        &previous,
        &no_images,
    )
    .expect("deep move");

    assert!(dir.join("a/b/c/loose.md").exists());
    std::fs::remove_dir_all(&dir).ok();
}

/// FR-015: an entry that left the plan takes its file with it — but only
/// because we have a record of having written that file.
#[test]
fn an_entry_that_left_the_plan_takes_its_file() {
    let dir = temp_dir();
    let kept = Uuid::now_v7();
    let gone = Uuid::now_v7();
    apply(
        &dir,
        &plan_with(vec![(kept, "kept.md", "a"), (gone, "gone.md", "b")]),
        &HashMap::new(),
        &no_images,
    )
    .expect("first pass");

    let previous = HashMap::from([(kept, "kept.md".to_string()), (gone, "gone.md".to_string())]);
    let changes = apply(
        &dir,
        &plan_with(vec![(kept, "kept.md", "a")]),
        &previous,
        &no_images,
    )
    .expect("second pass");

    assert_eq!(changes.removed, vec!["gone.md"]);
    assert!(!dir.join("gone.md").exists());
    assert!(
        dir.join("kept.md").exists(),
        "an unrelated entry was removed"
    );
    std::fs::remove_dir_all(&dir).ok();
}

/// A pass over an unchanged world must write nothing, or every pass dirties
/// the tree and the history fills with commits that say nothing happened.
#[test]
fn an_unchanged_world_changes_nothing() {
    let dir = temp_dir();
    let entry = Uuid::now_v7();
    let plan = plan_with(vec![(entry, "steady.md", "unchanged")]);
    apply(&dir, &plan, &HashMap::new(), &no_images).expect("first pass");

    let previous = HashMap::from([(entry, "steady.md".to_string())]);
    let changes = apply(&dir, &plan, &previous, &no_images).expect("second pass");

    assert!(
        changes.is_empty(),
        "a no-op pass reported changes: {changes:?}"
    );
    std::fs::remove_dir_all(&dir).ok();
}

/// FR-014, and the failure it must not cause: a storage hiccup on one picture
/// is not a reason to stop a world synchronising. The entry's words matter
/// more than its illustration.
#[test]
fn a_missing_image_object_does_not_fail_the_pass() {
    let dir = temp_dir();
    let entry = Uuid::now_v7();
    let mut plan = plan_with(vec![(entry, "illustrated.md", "See below.")]);
    plan.images.push(PlannedImage {
        asset_id: Uuid::now_v7(),
        path: "_images/missing.webp".to_string(),
        object_key: "lore/missing.webp".to_string(),
    });

    let changes = apply(&dir, &plan, &HashMap::new(), &no_images).expect("no collision");

    assert!(changes.written.contains(&"illustrated.md".to_string()));
    assert!(!dir.join("_images/missing.webp").exists());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn an_image_that_exists_is_written_once_and_not_rewritten() {
    let dir = temp_dir();
    let entry = Uuid::now_v7();
    let mut plan = plan_with(vec![(entry, "illustrated.md", "See below.")]);
    plan.images.push(PlannedImage {
        asset_id: Uuid::now_v7(),
        path: "_images/pic.webp".to_string(),
        object_key: "lore/pic.webp".to_string(),
    });
    let bytes = |_: &str| Some(b"webp-bytes".to_vec());

    let first = apply(&dir, &plan, &HashMap::new(), &bytes).expect("first");
    assert!(first.written.contains(&"_images/pic.webp".to_string()));

    let previous = HashMap::from([(entry, "illustrated.md".to_string())]);
    let second = apply(&dir, &plan, &previous, &bytes).expect("second");
    assert!(
        !second.written.contains(&"_images/pic.webp".to_string()),
        "an unchanged image was rewritten",
    );
    std::fs::remove_dir_all(&dir).ok();
}

/// FR-018: readable without the app open.
#[test]
fn a_commit_message_says_what_happened() {
    let one = Changes {
        written: vec!["westeros/the-red-keep.md".to_string()],
        ..Default::default()
    };
    assert_eq!(commit_message(&one), "Update the-red-keep.md");

    let moved = Changes {
        moved: vec![("a.md".to_string(), "b.md".to_string())],
        ..Default::default()
    };
    assert_eq!(commit_message(&moved), "Move a.md to b.md");

    let many = Changes {
        written: vec!["a.md".to_string(), "b.md".to_string()],
        removed: vec!["c.md".to_string()],
        ..Default::default()
    };
    assert_eq!(commit_message(&many), "Update lore (2 written, 1 removed)");
}
