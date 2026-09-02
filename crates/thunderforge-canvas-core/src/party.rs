//! Who comes with the party when the scene changes, and who is already there.
//!
//! Spec 031, US4 (FR-019). A Game Master moves the table from the tavern to
//! the cellar and asks for the party to come along. This module answers the
//! one question that has a wrong answer: for each character being brought,
//! does the destination need a new token, or does it already have one?
//!
//! # Why this is a rule rather than a query
//!
//! Research R2 settles that a token belongs to exactly one scene (ADR-040
//! unified the backing store onto the scene-scoped `tokens` table), so
//! "bring the party" means *creating* tokens in the destination. That makes
//! double-creation the natural failure: a Game Master who moves to the cellar,
//! moves back, and moves to the cellar again gets two of every character
//! unless something says no. The spec's edge case names exactly that.
//!
//! The decision is pure set arithmetic over ids, which is why it lives here
//! instead of in the mutation or the engine: it is the kind of thing that is
//! either right for every input or quietly wrong for one, and this crate's
//! tests actually execute.
//!
//! # Why ids and not records
//!
//! The predicate needs to know which characters are being brought and which
//! characters the destination already holds a token for. Handing it token
//! rows, actor rows or anything richer would let it start making a second
//! decision — which token to keep, which art to copy — and those belong to the
//! caller that has the records. See `item`'s note on the same shape.
//!
//! # What "already has one" means
//!
//! One token for that character anywhere in the destination scene, regardless
//! of where it sits or who placed it. A character standing in the cellar is in
//! the cellar; arriving with the party does not entitle them to a second body
//! on the map, and a Game Master who genuinely wants two copies of a character
//! places the second one deliberately.

use std::collections::BTreeSet;

/// What bringing one character to the destination should do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Retention {
    /// The destination has no token for this character; create one.
    Create,
    /// The destination already has one. Leave it exactly as it is.
    ///
    /// Deliberately not "move it" or "refresh it". The token in the
    /// destination was placed by somebody, at a position they chose, and a
    /// scene change the character was already present for is not a reason to
    /// disturb it.
    AlreadyPresent,
}

impl Retention {
    /// Whether this outcome requires the caller to create a token.
    pub fn creates(self) -> bool {
        matches!(self, Retention::Create)
    }
}

/// What to do about one character.
///
/// `destination_occupants` is every character id the destination scene already
/// holds a token for.
pub fn retention_for<S: AsRef<str>>(character_id: &str, destination_occupants: &[S]) -> Retention {
    if destination_occupants
        .iter()
        .any(|occupant| occupant.as_ref() == character_id)
    {
        Retention::AlreadyPresent
    } else {
        Retention::Create
    }
}

/// The characters that need a token created in the destination.
///
/// Returns them in the order they were selected, so a caller that creates
/// tokens in sequence lays them out in the order the Game Master picked rather
/// than in whatever order a set iterates. Duplicates within the selection
/// collapse to one entry: asking for a character twice in the same move is a
/// slip in the caller, not a request for two tokens, and this is the layer
/// that already knows the answer to "does this character have a token yet".
pub fn characters_to_create<S: AsRef<str>, T: AsRef<str>>(
    selection: &[S],
    destination_occupants: &[T],
) -> Vec<String> {
    let occupants: BTreeSet<&str> = destination_occupants.iter().map(AsRef::as_ref).collect();

    let mut created: BTreeSet<&str> = BTreeSet::new();
    let mut out = Vec::new();
    for character in selection.iter().map(AsRef::as_ref) {
        // Skipped rather than reported: the two reasons to skip — already in
        // the destination, and already queued by an earlier entry in this same
        // selection — both mean "this character will have exactly one token",
        // which is the whole promise.
        if occupants.contains(character) || !created.insert(character) {
            continue;
        }
        out.push(character.to_string());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_character_with_no_token_in_the_destination_gets_one() {
        assert_eq!(retention_for("alice", &["bob"]), Retention::Create);
        assert!(retention_for("alice", &["bob"]).creates());
    }

    #[test]
    fn a_character_who_is_already_there_does_not_gain_a_second_token() {
        // The spec's edge case, stated once: this is the assertion the whole
        // module exists for.
        assert_eq!(
            retention_for("alice", &["bob", "alice"]),
            Retention::AlreadyPresent
        );
        assert!(!retention_for("alice", &["alice"]).creates());
    }

    #[test]
    fn an_empty_destination_creates_every_selected_character() {
        let empty: [&str; 0] = [];
        assert_eq!(
            characters_to_create(&["alice", "bob"], &empty),
            vec!["alice".to_string(), "bob".to_string()],
        );
    }

    #[test]
    fn an_empty_selection_creates_nothing() {
        let empty: [&str; 0] = [];
        assert!(characters_to_create(&empty, &["alice"]).is_empty());
    }

    #[test]
    fn selection_order_is_preserved() {
        // A caller placing tokens one after another lays them out in the order
        // the Game Master picked. Sorting here would be a silent reordering
        // nobody asked for.
        let empty: [&str; 0] = [];
        assert_eq!(
            characters_to_create(&["zara", "alice", "mox"], &empty),
            vec!["zara".to_string(), "alice".to_string(), "mox".to_string()],
        );
    }

    #[test]
    fn a_character_listed_twice_still_gets_exactly_one_token() {
        let empty: [&str; 0] = [];
        assert_eq!(
            characters_to_create(&["alice", "bob", "alice"], &empty),
            vec!["alice".to_string(), "bob".to_string()],
        );
    }

    #[test]
    fn moving_back_and_forth_never_duplicates_the_party() {
        // The failure this exists to prevent, played out: tavern -> cellar
        // creates three tokens; going back and returning must create none,
        // because the cellar still holds them.
        let party = ["alice", "bob", "mox"];
        let first_visit = characters_to_create(&party, &[] as &[&str]);
        assert_eq!(first_visit.len(), 3);

        let second_visit = characters_to_create(&party, &first_visit);
        assert!(
            second_visit.is_empty(),
            "returning created {second_visit:?} on top of the party already there",
        );
    }

    #[test]
    fn a_partly_present_party_creates_only_the_missing_half() {
        // The realistic case: one character was already scouting ahead.
        assert_eq!(
            characters_to_create(&["alice", "bob", "mox"], &["bob"]),
            vec!["alice".to_string(), "mox".to_string()],
        );
    }

    #[test]
    fn occupants_the_party_did_not_bring_are_left_alone() {
        // The predicate answers only "what to create". An NPC standing in the
        // destination is not the party's business, and nothing here should
        // suggest touching it.
        let created = characters_to_create(&["alice"], &["innkeeper"]);
        assert_eq!(created, vec!["alice".to_string()]);
    }

    #[test]
    fn ids_are_matched_exactly() {
        // Ids are opaque identifiers, not names. Trimming or case-folding here
        // would make two genuinely different characters collide, and the
        // failure would be one of them silently never arriving.
        assert_eq!(retention_for("Alice", &["alice"]), Retention::Create);
        assert_eq!(retention_for("alice ", &["alice"]), Retention::Create);
    }
}
