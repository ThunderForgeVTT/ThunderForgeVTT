//! Making a working clone match a plan, without touching anything else.
//!
//! # The rule this module exists to keep
//!
//! **Files the system did not write are never deleted or modified** (FR-032).
//! That sounds obvious and is easy to break in a way nobody notices until it
//! matters, because the natural implementation of "make the directory match
//! the plan" is *delete everything not in the plan*. In a repository the user
//! owns, containing their own notes, their own README, and possibly another
//! tool's output, that is data loss dressed as correctness.
//!
//! So deletion is driven by what we previously wrote — the
//! `lore_exported_entries` rows — and never by what happens to be on disk. A
//! file in the world's subtree that we have no record of writing is left
//! exactly where it is, forever.
//!
//! # Renames are moves, not delete-plus-create
//!
//! FR-010 requires an entry's file history to survive a rename. Git works that
//! out from content similarity at diff time, but only if the old path is gone
//! and the new one is present in the same commit. Recording the previous path
//! per entry is what lets that happen; without it a rename is a deletion and
//! an unrelated creation, and the history stops at the rename.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use uuid::Uuid;

use crate::lore_sync::plan::Plan;

/// What a pass changed, for the commit message and the run record.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Changes {
    pub written: Vec<String>,
    pub moved: Vec<(String, String)>,
    pub removed: Vec<String>,
}

impl Changes {
    pub fn is_empty(&self) -> bool {
        self.written.is_empty() && self.moved.is_empty() && self.removed.is_empty()
    }
}

/// A path collision with content we did not write (FR-032).
#[derive(Debug, PartialEq, Eq)]
pub struct Collision {
    pub path: String,
}

/// Write a plan into a world's subtree.
///
/// `previously_written` maps an entry to the path we last wrote for it — the
/// `lore_exported_entries` rows. It is the only thing that authorises a
/// deletion or a move.
///
/// Returns `Err` on a first-synchronisation collision: a file already exists
/// where an entry's file must go, and we have no record of having written it.
/// The pass stops with an explanation rather than resolving it, because both
/// resolutions are wrong — overwriting destroys the user's file, and skipping
/// silently produces a mirror that is quietly incomplete.
pub fn apply(
    subtree: &Path,
    plan: &Plan,
    previously_written: &HashMap<Uuid, String>,
    read_image: &dyn Fn(&str) -> Option<Vec<u8>>,
) -> Result<Changes, Collision> {
    let mut changes = Changes::default();
    let planned_ids: HashSet<Uuid> = plan.files.iter().map(|f| f.entry_id).collect();

    for file in &plan.files {
        let target = subtree.join(&file.path);
        let previous = previously_written.get(&file.entry_id);

        // A path we have never written, that already has something in it, is
        // somebody else's file.
        if previous.is_none_or(|p| p != &file.path) && target.exists() {
            let ours = previously_written.values().any(|p| p == &file.path);
            if !ours {
                return Err(Collision {
                    path: file.path.clone(),
                });
            }
        }

        if let Some(old_path) = previous
            && old_path != &file.path
        {
            // A move, performed as a move so git can see it as one (FR-010).
            let old = subtree.join(old_path);
            if old.exists() {
                if let Some(parent) = target.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                if std::fs::rename(&old, &target).is_ok() {
                    changes.moved.push((old_path.clone(), file.path.clone()));
                }
            }
        }

        if let Some(parent) = target.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        // Only write when the bytes differ. An unconditional write would make
        // every pass dirty the working tree, and `git commit` would then have
        // nothing to say but would still be asked.
        let unchanged = std::fs::read(&target)
            .map(|existing| existing == file.contents.as_bytes())
            .unwrap_or(false);
        if !unchanged && std::fs::write(&target, &file.contents).is_ok() {
            changes.written.push(file.path.clone());
        }
    }

    for image in &plan.images {
        let target = subtree.join(&image.path);
        if target.exists() {
            continue;
        }
        let Some(bytes) = read_image(&image.object_key) else {
            // A missing object is not a reason to fail the whole pass. The
            // entry's words are more important than its picture, and a
            // storage hiccup must not stop a world synchronising.
            continue;
        };
        if let Some(parent) = target.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if std::fs::write(&target, bytes).is_ok() {
            changes.written.push(image.path.clone());
        }
    }

    // Deletion, driven only by what we wrote. An entry that left the plan —
    // deleted, or disabled by moderation (FR-015) — takes its file with it.
    for (entry_id, path) in previously_written {
        if planned_ids.contains(entry_id) {
            continue;
        }
        let target = subtree.join(path);
        if target.exists() && std::fs::remove_file(&target).is_ok() {
            changes.removed.push(path.clone());
        }
    }

    Ok(changes)
}

/// A commit message a reader can understand without the app open (FR-018).
pub fn commit_message(changes: &Changes) -> String {
    // The filename rather than the whole path: a reader scanning `git log`
    // wants "Update the-red-keep.md", not a directory tree they can already
    // see in the diff.
    fn leaf(path: &str) -> &str {
        path.rsplit('/').next().unwrap_or(path)
    }

    match (
        changes.written.len(),
        changes.moved.len(),
        changes.removed.len(),
    ) {
        (1, 0, 0) => format!("Update {}", leaf(&changes.written[0])),
        (0, 1, 0) => {
            let (from, to) = &changes.moved[0];
            format!("Move {from} to {to}")
        }
        (0, 0, 1) => format!("Remove {}", changes.removed[0]),
        _ => {
            let mut parts = Vec::new();
            if !changes.written.is_empty() {
                parts.push(format!("{} written", changes.written.len()));
            }
            if !changes.moved.is_empty() {
                parts.push(format!("{} moved", changes.moved.len()));
            }
            if !changes.removed.is_empty() {
                parts.push(format!("{} removed", changes.removed.len()));
            }
            format!("Update lore ({})", parts.join(", "))
        }
    }
}

#[cfg(test)]
#[path = "apply_tests.rs"]
mod apply_tests;
