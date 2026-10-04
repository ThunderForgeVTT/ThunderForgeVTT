//! The record that says which world writes to a repository.
//!
//! # Why this exists on the repository rather than only in the database
//!
//! FR-033 — two worlds must not synchronise into one directory — is enforced
//! by a unique constraint, and **a unique constraint sees one instance.** Two
//! ThunderForge instances can bind the same repository and neither database
//! will know, because neither can see the other. The repository is the only
//! place both of them can look.
//!
//! So before a world's first synchronisation the system writes an issue there
//! naming what is about to start writing. A second world, on any instance,
//! finds it.
//!
//! # It is advisory, and saying so is the point
//!
//! Two instances racing can still both write: there is a window between
//! reading the issues and opening one, and nothing here closes it. A lock this
//! is not, and FR-036i forbids claiming otherwise.
//!
//! What it buys is that a collision becomes **visible to a human, on the
//! repository, in the place they would look** — instead of two mirrors
//! silently overwriting each other and a Game Master wondering why their lore
//! keeps changing. A system with no shared state cannot do better than that,
//! and pretending to would be worse than admitting it.
//!
//! # What it must not contain
//!
//! Lore. On a public repository this is readable by anyone (FR-036j), so it
//! carries the world's name — which its owner chose — the instance, and
//! nothing derived from what the world holds.

use uuid::Uuid;

/// The title every binding record carries, and the only thing used to find
/// one.
///
/// Fixed rather than derived from the world, because a search has to match it
/// without knowing which world wrote it — that is the entire point of looking.
/// "do not close" is in the title because a Game Master tidying their issues
/// would otherwise remove the record that protects them.
pub const BINDING_ISSUE_TITLE: &str = "ThunderForge lore binding — do not close";

/// The marker that makes a body machine-readable without parsing prose.
const WORLD_MARKER: &str = "thunderforge-world:";
const INSTANCE_MARKER: &str = "thunderforge-instance:";

/// Who holds a repository's binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    pub world_id: Uuid,
    /// How this instance identifies itself. Two instances with the same value
    /// are indistinguishable here, which is a reason for it to be specific.
    pub instance: String,
}

/// The issue body claiming a repository for a world.
///
/// Written for two readers at once: a person who wants to know why the issue
/// exists, and the next instance's parser. The markers are on their own lines
/// so the prose can be rewritten without breaking the parse.
pub fn claim_body(binding: &Binding, world_name: &str, directory: &str) -> String {
    format!(
        "ThunderForge is mirroring a world's lore into this repository.\n\
         \n\
         **World:** {world_name}\n\
         **Directory:** `{directory}`\n\
         \n\
         This issue records which world is writing here, so that a second world \
         — on this instance or any other — can find out before it starts. \
         ThunderForge cannot lock a repository it does not control; this is a \
         notice, not a lock. If you see two worlds claiming this repository, \
         they will overwrite each other, and disconnecting one of them is the \
         fix.\n\
         \n\
         Closing this issue does not stop the synchronisation. Removing the \
         connection in ThunderForge does.\n\
         \n\
         <!-- {WORLD_MARKER} {} -->\n\
         <!-- {INSTANCE_MARKER} {} -->\n",
        binding.world_id, binding.instance,
    )
}

/// Read a binding out of an issue body, or `None` if it carries none.
///
/// Tolerant of everything around the markers — the prose above them is meant
/// to be edited, and a human adding a comment or reformatting must not make a
/// repository look unclaimed.
pub fn parse_binding(body: &str) -> Option<Binding> {
    let field = |marker: &str| -> Option<String> {
        body.lines().find_map(|line| {
            let at = line.find(marker)?;
            let rest = line[at + marker.len()..].trim();
            let value = rest.trim_end_matches("-->").trim();
            (!value.is_empty()).then(|| value.to_string())
        })
    };

    let world_id = field(WORLD_MARKER)?.parse().ok()?;
    let instance = field(INSTANCE_MARKER)?;
    Some(Binding { world_id, instance })
}

/// What a second world's attempt records on the existing issue (FR-036h).
///
/// A comment rather than a new issue, so that the whole history of who tried
/// to claim a repository is in one place. A second issue would be a second
/// thing to find, and finding one and not the other is how a conflict gets
/// half-understood.
pub fn conflict_comment(attempted: &Binding, world_name: &str) -> String {
    format!(
        "Another world tried to bind this repository and was refused.\n\
         \n\
         **World:** {world_name}\n\
         **Instance:** {}\n\
         \n\
         Nothing has been written for it. Only the world named in this issue is \
         synchronising here. If the wrong world holds the binding, disconnect it \
         in ThunderForge and the next attempt will succeed.\n",
        attempted.instance,
    )
}

/// Whether an existing binding belongs to the world about to write.
///
/// Both halves matter. The same world id on a different instance is a world
/// that was moved or restored from a backup, and writing from two instances
/// would interleave two histories — so it is a conflict, not a match, and the
/// human reading the issue is the one who can tell which instance should stop.
pub fn is_held_by(existing: &Binding, world_id: Uuid, instance: &str) -> bool {
    existing.world_id == world_id && existing.instance == instance
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binding() -> Binding {
        Binding {
            world_id: Uuid::now_v7(),
            instance: "tf.example".to_string(),
        }
    }

    #[test]
    fn a_claim_round_trips_through_its_own_body() {
        let b = binding();
        let parsed = parse_binding(&claim_body(&b, "Westeros", "lore")).expect("a binding");
        assert_eq!(parsed, b);
    }

    /// The prose above the markers is meant to be edited. A human tidying it,
    /// or a host reflowing it, must not make a claimed repository look free.
    #[test]
    fn a_body_someone_has_edited_still_parses() {
        let b = binding();
        let edited = format!(
            "Someone rewrote all of this.\n\nAnd added notes.\n\n{}",
            claim_body(&b, "Westeros", "lore")
                .lines()
                .filter(|l| l.contains("thunderforge-"))
                .collect::<Vec<_>>()
                .join("\n"),
        );
        assert_eq!(parse_binding(&edited), Some(b));
    }

    #[test]
    fn an_unrelated_issue_carries_no_binding() {
        assert_eq!(parse_binding("Please add dark mode"), None);
        assert_eq!(parse_binding(""), None);
    }

    /// A truncated or malformed marker is not a binding. Treating it as one
    /// would claim a repository on the strength of a typo.
    #[test]
    fn a_malformed_marker_is_not_a_binding() {
        assert_eq!(parse_binding("<!-- thunderforge-world: -->"), None);
        assert_eq!(
            parse_binding("<!-- thunderforge-world: not-a-uuid -->"),
            None
        );
        // A world with no instance is half a record, and half is not enough to
        // tell whether it is ours.
        assert_eq!(
            parse_binding(&format!("<!-- thunderforge-world: {} -->", Uuid::now_v7())),
            None
        );
    }

    #[test]
    fn a_binding_matches_only_its_own_world_and_instance() {
        let b = binding();
        assert!(is_held_by(&b, b.world_id, &b.instance));
        assert!(!is_held_by(&b, Uuid::now_v7(), &b.instance));
    }

    /// The same world on a different instance is a conflict, not a match — a
    /// world restored from a backup elsewhere would otherwise write from two
    /// places and interleave two histories.
    #[test]
    fn the_same_world_on_another_instance_is_a_conflict() {
        let b = binding();
        assert!(!is_held_by(&b, b.world_id, "somewhere.else"));
    }

    /// FR-036j. On a public repository this is readable by anyone.
    #[test]
    fn a_claim_carries_no_lore() {
        let body = claim_body(&binding(), "Westeros", "lore");
        assert!(
            body.contains("Westeros"),
            "the world's own name is expected"
        );
        // The world id and directory are the only other specifics, and both
        // are structural rather than content.
        assert!(
            !body.contains("["),
            "a link that could carry content leaked"
        );
    }

    /// FR-036i, asserted in the words a person actually reads. A body that
    /// implied a lock would leave a Game Master trusting something the product
    /// cannot do.
    #[test]
    fn the_body_says_it_is_not_a_lock() {
        let body = claim_body(&binding(), "Westeros", "lore");
        assert!(body.contains("not a lock"));
        assert!(body.contains("overwrite each other"));
    }
}
