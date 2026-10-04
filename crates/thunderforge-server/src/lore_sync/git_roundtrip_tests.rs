//! The mirror, proven against a real repository.
//!
//! # Why a local bare repository rather than a mock
//!
//! Every claim this feature makes that is worth anything is a claim about
//! *git*: that a rename preserves a file's history, that a push refuses to
//! overwrite divergent history, that a clone contains what the app shows. A
//! mocked git can only confirm that we called the functions we meant to call,
//! which is the part that was never in doubt.
//!
//! A bare repository in a temporary directory is a real remote. It needs no
//! network, no credential and no host, so these run anywhere the suite runs —
//! and they exercise the actual binary, which is what the product will use.
//!
//! These tests deliberately do not use the credential helper: a `file://`
//! remote needs no authentication. That the helper keeps the token out of
//! `argv` is `git.rs`'s own test, and asserting it here too would test the
//! same thing twice while making these need a fake credential.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use uuid::Uuid;

use crate::lore_sync::apply::{self, Changes};
use crate::lore_sync::git::{self, CommitIdentity};
use crate::lore_sync::plan::{Plan, PlannedFile};

struct Fixture {
    root: PathBuf,
    remote: PathBuf,
    clone: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("tf-git-{}", Uuid::now_v7()));
        let remote = root.join("remote.git");
        let clone = root.join("clone");
        std::fs::create_dir_all(&remote).expect("remote dir");

        run(
            &root,
            &[
                "init",
                "--bare",
                "--initial-branch=main",
                remote.to_str().unwrap(),
            ],
        );

        // A bare repository has no commits, and `clone --branch main` of an
        // empty repository fails. Seed it the way a real repository would
        // already be seeded — with the user's own file, which doubles as the
        // FR-032 fixture.
        let seed = root.join("seed");
        std::fs::create_dir_all(&seed).expect("seed dir");
        run(&seed, &["init", "--initial-branch=main"]);
        std::fs::write(seed.join("README.md"), "the user's own notes\n").unwrap();
        run(&seed, &["add", "README.md"]);
        commit(&seed, "Initial");
        run(
            &seed,
            &["remote", "add", "origin", remote.to_str().unwrap()],
        );
        run(&seed, &["push", "origin", "main"]);

        run(
            &root,
            &[
                "clone",
                "--branch",
                "main",
                remote.to_str().unwrap(),
                clone.to_str().unwrap(),
            ],
        );

        Self {
            root,
            remote,
            clone,
        }
    }

    fn subtree(&self) -> PathBuf {
        crate::lore_sync::workspace::world_subtree(&self.clone, "lore").expect("a subtree")
    }

    /// Commit whatever `apply` changed, and push it.
    fn publish(&self, message: &str) {
        run(&self.clone, &["add", "--all"]);
        let identity = CommitIdentity {
            author_name: "A Player".into(),
            author_email: "player@users.noreply.example".into(),
            committer_name: "ThunderForge VTT".into(),
            committer_email: "noreply@example".into(),
        };
        let args = git::commit_args(&identity, message);
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        run(&self.clone, &refs);
        run(&self.clone, &["push", "origin", "main"]);
    }

    /// A fresh clone of the remote — what a reader would actually get.
    fn fresh_clone(&self) -> PathBuf {
        let into = self.root.join(format!("read-{}", Uuid::now_v7()));
        run(
            &self.root,
            &[
                "clone",
                "--branch",
                "main",
                self.remote.to_str().unwrap(),
                into.to_str().unwrap(),
            ],
        );
        into
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.root).ok();
    }
}

fn run(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .current_dir(dir)
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .expect("git runs");
    assert!(
        out.status.success(),
        "git {args:?} failed in {dir:?}: {}",
        String::from_utf8_lossy(&out.stderr),
    );
    String::from_utf8_lossy(&out.stdout).to_string()
}

fn commit(dir: &Path, message: &str) {
    let identity = CommitIdentity {
        author_name: "A Player".into(),
        author_email: "player@users.noreply.example".into(),
        committer_name: "ThunderForge VTT".into(),
        committer_email: "noreply@example".into(),
    };
    let args = git::commit_args(&identity, message);
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    run(dir, &refs);
}

fn plan_of(files: &[(Uuid, &str, &str)]) -> Plan {
    Plan {
        files: files
            .iter()
            .map(|(id, path, contents)| PlannedFile {
                entry_id: *id,
                path: (*path).to_string(),
                contents: (*contents).to_string(),
            })
            .collect(),
        images: Vec::new(),
        notes: Vec::new(),
    }
}

fn no_images(_: &str) -> Option<Vec<u8>> {
    None
}

/// User Story 1's independent test, end to end: a plan becomes a tree in a
/// repository that someone else can clone and read.
#[test]
fn a_plan_reaches_a_repository_a_reader_can_clone() {
    let fx = Fixture::new();
    let parent = Uuid::now_v7();
    let child = Uuid::now_v7();

    let plan = plan_of(&[
        (parent, "westeros.md", "---\nid: x\n---\nA continent."),
        (
            child,
            "westeros/the-red-keep.md",
            "---\nid: y\n---\nA castle.",
        ),
    ]);
    apply::apply(&fx.subtree(), &plan, &HashMap::new(), &no_images).expect("applied");
    fx.publish("Update lore");

    let read = fx.fresh_clone();
    assert_eq!(
        std::fs::read_to_string(read.join("lore/westeros/the-red-keep.md")).unwrap(),
        "---\nid: y\n---\nA castle.",
    );
    assert!(read.join("lore/westeros.md").exists());
}

/// FR-032, against a real repository rather than a directory: the user's own
/// file survives every pass, forever.
#[test]
fn the_users_own_files_are_untouched_by_a_pass() {
    let fx = Fixture::new();
    let entry = Uuid::now_v7();
    apply::apply(
        &fx.subtree(),
        &plan_of(&[(entry, "an-entry.md", "ours")]),
        &HashMap::new(),
        &no_images,
    )
    .expect("applied");
    fx.publish("Update lore");

    let read = fx.fresh_clone();
    assert_eq!(
        std::fs::read_to_string(read.join("README.md")).unwrap(),
        "the user's own notes\n",
        "the user's file was modified",
    );
}

/// **FR-010, and the reason this test uses real git.** History survival is a
/// property of git's rename detection, not of our code — the only way to know
/// it holds is to ask git.
#[test]
fn a_renamed_entry_keeps_its_file_history() {
    let fx = Fixture::new();
    let entry = Uuid::now_v7();

    apply::apply(
        &fx.subtree(),
        &plan_of(&[(
            entry,
            "old-name.md",
            "The body, which is long enough to match on.",
        )]),
        &HashMap::new(),
        &no_images,
    )
    .expect("first");
    fx.publish("Create the entry");

    let previous = HashMap::from([(entry, "old-name.md".to_string())]);
    apply::apply(
        &fx.subtree(),
        &plan_of(&[(
            entry,
            "new-name.md",
            "The body, which is long enough to match on.",
        )]),
        &previous,
        &no_images,
    )
    .expect("rename");
    fx.publish("Rename the entry");

    let read = fx.fresh_clone();
    let log = Command::new("git")
        .current_dir(&read)
        .args(["log", "--follow", "--format=%s", "--", "lore/new-name.md"])
        .output()
        .expect("git log");
    let history = String::from_utf8_lossy(&log.stdout);

    assert!(
        history.contains("Create the entry"),
        "history was truncated at the rename: {history}",
    );
}

/// FR-017, checked in the artefact rather than in the arguments: the commit
/// carries two identities and no personal address.
#[test]
fn a_commit_records_both_identities_and_no_personal_address() {
    let fx = Fixture::new();
    let entry = Uuid::now_v7();
    apply::apply(
        &fx.subtree(),
        &plan_of(&[(entry, "an-entry.md", "body")]),
        &HashMap::new(),
        &no_images,
    )
    .expect("applied");
    fx.publish("Update an-entry.md");

    let read = fx.fresh_clone();
    let out = Command::new("git")
        .current_dir(&read)
        .args(["log", "-1", "--format=%an|%ae|%cn|%ce"])
        .output()
        .expect("git log");
    let line = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let parts: Vec<&str> = line.split('|').collect();

    assert_eq!(parts[0], "A Player", "the author is not the writer");
    assert_eq!(
        parts[2], "ThunderForge VTT",
        "the committer is not the platform"
    );
    assert!(
        parts[1].contains("noreply"),
        "an author address that is not a no-reply reached a commit: {}",
        parts[1],
    );
}

/// **FR-031.** A lease naming a commit the remote no longer holds must be
/// refused *by the remote*, not by a check we ran a moment earlier and might
/// have raced.
#[test]
fn a_push_is_refused_when_the_remote_has_moved_underneath_us() {
    let fx = Fixture::new();
    let entry = Uuid::now_v7();
    apply::apply(
        &fx.subtree(),
        &plan_of(&[(entry, "an-entry.md", "ours")]),
        &HashMap::new(),
        &no_images,
    )
    .expect("applied");
    run(&fx.clone, &["add", "--all"]);
    commit(&fx.clone, "Update lore");

    // Someone else rewrites the branch while our pass is in flight.
    let other = fx.root.join("other");
    run(
        &fx.root,
        &[
            "clone",
            fx.remote.to_str().unwrap(),
            other.to_str().unwrap(),
        ],
    );
    std::fs::write(other.join("theirs.txt"), "a change we never saw\n").unwrap();
    run(&other, &["add", "--all"]);
    commit(&other, "Someone else's work");
    run(&other, &["push", "origin", "main"]);

    // A lease naming what we *believed* the remote held.
    let stale = run(&fx.clone, &["rev-parse", "origin/main"])
        .trim()
        .to_string();
    let args = git::push_args("main", Some(&stale));
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let out = Command::new("git")
        .current_dir(&fx.clone)
        .args(&refs)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .expect("git runs");

    assert!(
        !out.status.success(),
        "a push overwrote history it had not seen",
    );

    let read = fx.fresh_clone();
    assert!(
        read.join("theirs.txt").exists(),
        "someone else's commit was destroyed",
    );
}

/// A pass over an unchanged world must produce no commit at all, or the
/// history fills with commits that say nothing happened and stops being
/// readable — which is most of what a repository is for.
#[test]
fn an_unchanged_world_produces_no_commit() {
    let fx = Fixture::new();
    let entry = Uuid::now_v7();
    let plan = plan_of(&[(entry, "steady.md", "unchanged")]);
    apply::apply(&fx.subtree(), &plan, &HashMap::new(), &no_images).expect("first");
    fx.publish("Create the entry");

    let before = run(&fx.clone, &["rev-parse", "HEAD"]).trim().to_string();

    let previous = HashMap::from([(entry, "steady.md".to_string())]);
    let changes: Changes =
        apply::apply(&fx.subtree(), &plan, &previous, &no_images).expect("second");
    assert!(changes.is_empty());

    let after = run(&fx.clone, &["rev-parse", "HEAD"]).trim().to_string();
    assert_eq!(before, after, "a no-op pass moved HEAD");
}
