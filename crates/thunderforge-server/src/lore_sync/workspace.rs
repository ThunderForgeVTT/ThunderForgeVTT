//! The working clone a connection synchronises through.
//!
//! # A cache, not state
//!
//! Everything here is reconstructible from the world and the remote. That is
//! the property that keeps FR-030's "converge without user reconstruction"
//! true after a disk is wiped, an instance is moved, or a container is
//! replaced: losing a working clone costs a re-clone and nothing else.
//!
//! It matters that this is written down, because a working clone *looks* like
//! state — it has a `.git` directory and a history — and the temptation is to
//! treat it as precious. Nothing in it is. If a pass finds a clone it cannot
//! use, discarding and re-cloning is always correct.
//!
//! # Why persistent rather than per-run
//!
//! A fresh clone per run is simpler and wrong at the size this has to work at.
//! SC-002 says the mirror must be faithful "for a world of any size", and
//! SC-003 puts an edit in the repository within 60 seconds — so a pass runs
//! often. Re-fetching every object each time is waste that grows with exactly
//! the worlds that matter most. A persistent clone makes the steady state a
//! small incremental fetch.

use std::path::{Path, PathBuf};

use uuid::Uuid;

/// Where a connection's working clone lives.
///
/// Keyed by connection id rather than by world or repository, because a
/// connection is what owns the clone: removing a connection removes the
/// directory (FR-005 leaves the *repository* untouched, not our copy of it),
/// and a world that connects a second time after disconnecting starts clean
/// rather than inheriting a clone whose history it may no longer share.
pub fn clone_path(root: &Path, connection_id: Uuid) -> PathBuf {
    root.join("lore-sync").join(connection_id.to_string())
}

/// Whether a directory holds a git working clone we can use.
///
/// Deliberately shallow: the presence of `.git` is enough to decide *reuse or
/// re-clone*, and anything deeper would be this module forming an opinion
/// about repository health that `git` itself will express more accurately on
/// the next fetch. A clone that exists but is broken fails the fetch, and the
/// caller re-clones — which is the same path as a missing one.
pub fn is_usable_clone(path: &Path) -> bool {
    path.join(".git").exists()
}

/// Remove a connection's working clone.
///
/// Called when a connection is removed, and safe to call when the clone is
/// already gone. Never touches anything outside the connection's own
/// directory.
pub fn discard(root: &Path, connection_id: Uuid) -> std::io::Result<()> {
    let path = clone_path(root, connection_id);
    match std::fs::remove_dir_all(&path) {
        Ok(()) => Ok(()),
        // Already absent is success. A caller removing a connection should not
        // have to care whether a pass ever ran.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// The subtree within the clone that this world owns.
///
/// **Everything outside this path is untouchable, forever** (FR-032). Files
/// the system did not write are never deleted or modified, and a collision
/// inside this directory stops the first synchronisation with an explanation
/// rather than resolving itself.
///
/// Returns `None` for a directory that would escape the clone — a `..`
/// component, an absolute path, or a root-relative empty segment. That is a
/// refusal rather than a sanitisation on purpose: a directory value that tries
/// to escape is either a bug or an attack, and quietly rewriting it into
/// something safe would hide both.
pub fn world_subtree(clone: &Path, directory: &str) -> Option<PathBuf> {
    let trimmed = directory.trim_matches('/');
    if trimmed.is_empty() {
        // The whole repository. Legitimate — a repository dedicated to one
        // world — and the caller still may not touch what it did not write.
        return Some(clone.to_path_buf());
    }
    let mut out = clone.to_path_buf();
    for component in trimmed.split('/') {
        if component.is_empty() || component == "." || component == ".." {
            return None;
        }
        out.push(component);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_clone_is_keyed_by_connection_not_by_world() {
        let root = Path::new("/var/lib/thunderforge");
        let a = Uuid::now_v7();
        let b = Uuid::now_v7();
        assert_ne!(clone_path(root, a), clone_path(root, b));
        assert!(clone_path(root, a).starts_with(root.join("lore-sync")));
    }

    #[test]
    fn discarding_an_absent_clone_is_success() {
        let root = std::env::temp_dir().join(format!("tf-ws-{}", Uuid::now_v7()));
        assert!(discard(&root, Uuid::now_v7()).is_ok());
    }

    #[test]
    fn discarding_removes_only_that_connections_clone() {
        let root = std::env::temp_dir().join(format!("tf-ws-{}", Uuid::now_v7()));
        let keep = Uuid::now_v7();
        let drop_it = Uuid::now_v7();
        std::fs::create_dir_all(clone_path(&root, keep)).unwrap();
        std::fs::create_dir_all(clone_path(&root, drop_it)).unwrap();

        discard(&root, drop_it).unwrap();

        assert!(!clone_path(&root, drop_it).exists());
        assert!(
            clone_path(&root, keep).exists(),
            "an unrelated clone was removed"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    /// FR-032's boundary. A directory that escapes the clone is refused rather
    /// than sanitised: quietly rewriting it would hide a bug or an attack.
    #[test]
    fn a_directory_cannot_escape_the_clone() {
        let clone = Path::new("/tmp/clone");
        for escape in ["..", "../elsewhere", "lore/../..", "a//b", "./.."] {
            assert!(
                world_subtree(clone, escape).is_none(),
                "escaping directory accepted: {escape}",
            );
        }
    }

    #[test]
    fn an_ordinary_directory_resolves_under_the_clone() {
        let clone = Path::new("/tmp/clone");
        assert_eq!(world_subtree(clone, "lore").unwrap(), clone.join("lore"),);
        assert_eq!(
            world_subtree(clone, "/worlds/mine/").unwrap(),
            clone.join("worlds").join("mine"),
        );
        // A repository dedicated to one world is legitimate.
        assert_eq!(world_subtree(clone, "").unwrap(), clone);
    }
}
