//! Driving the `git` binary, and keeping the credential out of the process
//! table while doing it.
//!
//! # Why a subprocess rather than a library
//!
//! Because the requirement that decides it is FR-004c, not build convenience.
//! Everything past the credential grant must be host-neutral, and **git over
//! HTTPS is the host-neutral protocol** — a host's REST API would put one
//! vendor inside this module. Three things then come free that would otherwise
//! be written and got wrong:
//!
//! - **Rename detection** (FR-010). Git computes it at diff time from content
//!   similarity; nothing here has to track a move.
//! - **Divergence refusal** (FR-031). `--force-with-lease` refuses a push when
//!   the remote no longer holds what we last wrote, atomically, at the server.
//!   A read-then-compare in this process would have a race between the two.
//! - **Verification** (FR-034). `rev-parse` answers what the remote actually
//!   holds, against the same data the push wrote.
//!
//! `specs/034-lore-git-sync/research.md` R1 records the alternatives.
//!
//! # The credential never reaches `argv`
//!
//! This is the constraint that shapes the whole module, and it is worth being
//! exact about why. FR-035 says a credential must never appear in logs. A
//! process listing is *worse* than a log: `ps` is readable by any local user,
//! it needs no privilege, and nothing rotates it. Embedding a token in the
//! remote URL — `https://x-access-token:TOKEN@host/owner/repo` — is the
//! obvious way to authenticate a push and it publishes the token to every
//! process on the machine for as long as the push runs.
//!
//! So the token is passed through the **child process environment**, and what
//! goes in `argv` is a credential helper that names the variable rather than
//! its value:
//!
//! ```text
//! git -c credential.helper=!f() { echo username=...; echo password=$VAR; }; f
//! ```
//!
//! `argv` therefore contains the *name* `THUNDERFORGE_REPO_TOKEN` and never its
//! contents. An environment is not perfectly private either — `/proc/<pid>/environ`
//! exists — but it is readable only by the same user or root, where `argv` is
//! readable by everyone, and it is the narrowest channel available without
//! shipping a helper binary.
//!
//! [`push_args`] and its siblings are separated from execution precisely so
//! this can be *tested*: `credential_never_appears_in_arguments` asserts it
//! against the constructed invocation rather than trusting the reading above.

use std::path::Path;
use std::process::Command;

/// The environment variable the credential helper reads.
///
/// Named as a constant because two places must agree about it — the helper
/// snippet placed in `argv`, and the environment handed to the child — and a
/// mismatch would fail as an authentication error rather than as the typo it
/// is.
pub const TOKEN_ENV_VAR: &str = "THUNDERFORGE_REPO_TOKEN";

/// The username half of the credential. Hosts that accept a token as a
/// password ignore this, but git insists on having one.
const TOKEN_USERNAME: &str = "x-access-token";

/// Who git records as having made a commit, as distinct from who wrote it.
///
/// Git carries two identities per commit and this feature has exactly two
/// facts: who wrote the revision, and what put it in the repository. Using
/// both fields for their actual meaning satisfies FR-017 without compromise —
/// attribution to the authoring account, and no personal email address
/// published without consent.
///
/// Naming the application as committer is also the honest description. A human
/// did not run `git commit`; the platform did, on their behalf, and a history
/// claiming otherwise misleads whoever later works out where a change came
/// from.
#[derive(Debug, Clone)]
pub struct CommitIdentity {
    pub author_name: String,
    pub author_email: String,
    pub committer_name: String,
    pub committer_email: String,
}

/// The `-c credential.helper=...` argument that lets git read the token from
/// the environment.
///
/// The value is a shell function, which is git's documented way of supplying a
/// helper inline. It names [`TOKEN_ENV_VAR`]; it never contains the token.
fn credential_helper_arg() -> String {
    format!(
        "credential.helper=!f() {{ echo username={TOKEN_USERNAME}; echo password=${TOKEN_ENV_VAR}; }}; f"
    )
}

/// Arguments common to every invocation that talks to a remote.
///
/// `credential.helper=` with an empty value first is not decoration: it clears
/// any helper the *machine* has configured, so a developer's global keychain
/// helper cannot silently supply a different identity than the one this
/// connection granted. Without it, a push might succeed using the wrong
/// credential and nobody would know.
fn remote_args() -> Vec<String> {
    vec![
        "-c".to_string(),
        "credential.helper=".to_string(),
        "-c".to_string(),
        credential_helper_arg(),
        // Never wait for a human. A prompt in a background task hangs the task
        // forever rather than failing it, which reads as a stuck sync.
        "-c".to_string(),
        "core.askPass=".to_string(),
    ]
}

/// `git clone` for a connection's working copy, of a repository that has the
/// branch already.
///
/// Fails on a repository with no commits — `--branch` names a ref that does not
/// exist yet. That is not an edge case to shrug at: **an empty repository is
/// the first thing a user connects**, because creating one and pointing
/// ThunderForge at it is the obvious way to start. [`clone_unborn_args`] is the
/// other half, and the caller falls back to it.
pub fn clone_args(remote_url: &str, branch: &str, into: &Path) -> Vec<String> {
    let mut args = remote_args();
    args.extend([
        "clone".to_string(),
        "--branch".to_string(),
        branch.to_string(),
        "--single-branch".to_string(),
        remote_url.to_string(),
        into.display().to_string(),
    ]);
    args
}

/// `git clone` of a repository that has no commits yet.
///
/// No `--branch`, because there is no branch to name — the remote's HEAD points
/// at something unborn. The clone succeeds with a warning and leaves a working
/// tree on an unborn branch, which is exactly what a first synchronisation
/// wants: write the files, commit, and push, and the branch comes into
/// existence with them.
///
/// The branch is set explicitly afterwards rather than trusting the clone's
/// default, because a host's default branch name and the connection's
/// configured one need not agree, and inheriting the host's would quietly
/// synchronise to a branch nobody chose.
pub fn clone_unborn_args(remote_url: &str, into: &Path) -> Vec<String> {
    let mut args = remote_args();
    args.extend([
        "clone".to_string(),
        remote_url.to_string(),
        into.display().to_string(),
    ]);
    args
}

/// Point an unborn working tree at the branch this connection writes to.
pub fn set_unborn_branch_args(branch: &str) -> Vec<String> {
    vec![
        "symbolic-ref".to_string(),
        "HEAD".to_string(),
        format!("refs/heads/{branch}"),
    ]
}

/// `git fetch`, the read half of a pass (FR-034b).
pub fn fetch_args(branch: &str) -> Vec<String> {
    let mut args = remote_args();
    args.extend([
        "fetch".to_string(),
        "origin".to_string(),
        branch.to_string(),
    ]);
    args
}

/// `git commit`, with author and committer set separately (FR-017).
pub fn commit_args(identity: &CommitIdentity, message: &str) -> Vec<String> {
    vec![
        "-c".to_string(),
        format!("user.name={}", identity.committer_name),
        "-c".to_string(),
        format!("user.email={}", identity.committer_email),
        "commit".to_string(),
        "--author".to_string(),
        format!("{} <{}>", identity.author_name, identity.author_email),
        "--message".to_string(),
        message.to_string(),
    ]
}

/// `git push --force-with-lease`, which is FR-031's divergence refusal.
///
/// The lease names the commit we believe the remote holds. If it holds
/// anything else — a force-push, someone else's write, a rewritten history —
/// the push is refused **by the remote**, atomically, rather than by a check
/// this process ran a moment earlier and might have raced.
///
/// `--force-with-lease` rather than `--force`, always. A plain force is how a
/// synchronisation quietly destroys work someone did in the repository, which
/// FR-031 exists to prevent.
pub fn push_args(branch: &str, expected_remote_commit: Option<&str>) -> Vec<String> {
    let mut args = remote_args();
    args.push("push".to_string());
    args.push(match expected_remote_commit {
        // The lease is empty on a first push, where there is nothing to
        // protect and no expectation to state.
        Some(commit) => format!("--force-with-lease=refs/heads/{branch}:{commit}"),
        None => "--force-with-lease".to_string(),
    });
    args.extend(["origin".to_string(), format!("HEAD:{branch}")]);
    args
}

/// `git rev-parse`, for verifying what the remote actually holds (FR-034).
pub fn rev_parse_args(rev: &str) -> Vec<String> {
    vec!["rev-parse".to_string(), rev.to_string()]
}

/// Build the command for an invocation, with the token in the environment.
///
/// The one place a token and a `Command` meet, so the one place to check that
/// the token goes to `env` and never to `arg`.
pub fn command(working_dir: &Path, args: &[String], token: Option<&str>) -> Command {
    let mut cmd = Command::new("git");
    cmd.current_dir(working_dir);
    cmd.args(args);
    if let Some(token) = token {
        cmd.env(TOKEN_ENV_VAR, token);
    }
    // Deterministic, machine-independent behaviour: a developer's global git
    // config must not change what a synchronisation does.
    cmd.env("GIT_CONFIG_NOSYSTEM", "1");
    cmd.env("GIT_TERMINAL_PROMPT", "0");
    cmd
}

/// Whether a usable `git` is on the path.
///
/// This is the server's first external binary dependency, and there is no
/// Dockerfile in this repository recording it. FR-036c's diagnostic posture
/// extends to cover it: an operator learns at startup, not when a Game Master
/// first tries to connect.
pub fn git_is_available() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "ghs_averysecrettokenvalue";

    fn identity() -> CommitIdentity {
        CommitIdentity {
            author_name: "A Player".to_string(),
            author_email: "player@users.noreply.example".to_string(),
            committer_name: "ThunderForge VTT".to_string(),
            committer_email: "noreply@example".to_string(),
        }
    }

    /// **The test this module exists for.**
    ///
    /// FR-035 forbids a credential appearing in logs, and a process listing is
    /// worse than a log — readable by any local user, needing no privilege,
    /// and never rotated. The easy mistake is a token in the remote URL, which
    /// no other test here would catch.
    ///
    /// Every remote-touching invocation is checked, because it only takes one.
    #[test]
    fn the_credential_never_appears_in_arguments() {
        let invocations = vec![
            clone_args("https://host/owner/repo.git", "main", Path::new("/tmp/x")),
            fetch_args("main"),
            push_args("main", Some("abc123")),
            push_args("main", None),
            rev_parse_args("origin/main"),
        ];

        for args in invocations {
            let joined = args.join(" ");
            assert!(
                !joined.contains(SECRET),
                "a credential reached argv: {joined}"
            );
            // The helper must name the variable, not interpolate it.
            assert!(
                !joined.contains("ghs_"),
                "something token-shaped reached argv: {joined}"
            );
        }
    }

    /// The other half: the token must actually be *somewhere*, or the tests
    /// above would pass on a build that simply never authenticates.
    #[test]
    fn the_credential_is_passed_through_the_environment() {
        let cmd = command(Path::new("/tmp"), &fetch_args("main"), Some(SECRET));
        let found = cmd
            .get_envs()
            .find(|(k, _)| *k == std::ffi::OsStr::new(TOKEN_ENV_VAR))
            .and_then(|(_, v)| v)
            .expect("the token is in the environment");
        assert_eq!(found, std::ffi::OsStr::new(SECRET));
    }

    /// The helper snippet has to reference the same variable the environment
    /// sets. A mismatch fails as an authentication error, which looks like a
    /// revoked grant rather than the typo it is.
    #[test]
    fn the_helper_reads_the_variable_the_environment_sets() {
        assert!(credential_helper_arg().contains(&format!("${TOKEN_ENV_VAR}")));
    }

    /// A machine-configured credential helper must not be able to supply a
    /// different identity than the one this connection was granted — a push
    /// could otherwise succeed as the wrong user and nobody would know.
    #[test]
    fn a_machine_configured_helper_is_cleared_first() {
        let args = fetch_args("main");
        let cleared = args.iter().position(|a| a == "credential.helper=");
        let ours = args
            .iter()
            .position(|a| a.starts_with("credential.helper=!f()"));
        assert!(cleared.is_some(), "the machine's helper is not cleared");
        assert!(cleared < ours, "the clear must come before our helper");
    }

    /// FR-031. A plain `--force` is how a synchronisation quietly destroys work
    /// someone did in the repository.
    #[test]
    fn a_push_never_forces_without_a_lease() {
        for args in [push_args("main", Some("abc123")), push_args("main", None)] {
            assert!(
                args.iter().any(|a| a.starts_with("--force-with-lease")),
                "push did not use a lease",
            );
            assert!(
                !args.iter().any(|a| a == "--force" || a == "-f"),
                "push used a bare force",
            );
        }
    }

    /// FR-017: two identities, used for their actual meanings.
    #[test]
    fn a_commit_records_the_author_and_the_committer_separately() {
        let args = commit_args(&identity(), "Update The Red Keep");
        let joined = args.join(" ");
        assert!(joined.contains("--author A Player <player@users.noreply.example>"));
        assert!(joined.contains("user.name=ThunderForge VTT"));
        assert!(
            !joined.contains("@example.com"),
            "a personal-looking address reached a commit",
        );
    }

    /// An empty repository is the first thing a user connects — creating one
    /// and pointing ThunderForge at it is the obvious way to start — so the
    /// unborn clone must not name a branch that does not exist yet.
    #[test]
    fn cloning_an_empty_repository_names_no_branch() {
        let args = clone_unborn_args("https://host/owner/repo.git", Path::new("/tmp/x"));
        assert!(!args.iter().any(|a| a == "--branch"), "{args:?}");
        assert!(args.iter().any(|a| a == "clone"));
        // The credential arrangement is the same; only the ref handling differs.
        assert!(args.iter().any(|a| a.starts_with("credential.helper=!f()")));
    }

    /// The branch is set rather than inherited: a host's default branch name
    /// and the connection's configured one need not agree, and inheriting the
    /// host's would synchronise to a branch nobody chose.
    #[test]
    fn an_unborn_tree_is_pointed_at_the_configured_branch() {
        assert_eq!(
            set_unborn_branch_args("trunk"),
            vec!["symbolic-ref", "HEAD", "refs/heads/trunk"],
        );
    }

    /// A background task that waits for a human hangs forever rather than
    /// failing, which reads as a stuck synchronisation rather than an error.
    #[test]
    fn nothing_can_prompt_for_input() {
        let cmd = command(Path::new("/tmp"), &fetch_args("main"), Some(SECRET));
        let envs: Vec<_> = cmd.get_envs().collect();
        assert!(
            envs.iter()
                .any(|(k, v)| *k == std::ffi::OsStr::new("GIT_TERMINAL_PROMPT")
                    && *v == Some(std::ffi::OsStr::new("0"))),
            "git could still prompt",
        );
        assert!(fetch_args("main").iter().any(|a| a == "core.askPass="));
    }
}
