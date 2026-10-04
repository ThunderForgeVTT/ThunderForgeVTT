//! Spec 027 (T005, FR-006): the one generator for every user-facing link code.
//!
//! Before this, four copies existed — `generate_share_code()` in each of
//! `mutations_actor_shares.rs`, `mutations_item_shares.rs` and
//! `mutations_ability_shares.rs`, plus an inline variant in
//! `mutations_invites.rs` that took only **8** characters. An invite code
//! grants membership in a world, so ~32 bits of entropy did not meet ADR-049's
//! unguessable-code invariant while content share links already used ~80.
//! Consolidating here raises invites to share-code strength and leaves one
//! place to change if that bar ever moves.
//!
//! # The v4 requirement is load-bearing
//!
//! The source MUST be an independent, fully-random v4 UUID. It must never be a
//! v7 UUID, and never anything else derived from a clock.
//!
//! This is not a stylistic preference — it is a fix for a real, reproduced
//! defect. v7 UUIDs front-load a millisecond timestamp, so taking the leading
//! hex characters captures mostly that timestamp. Two links created inside the
//! same millisecond then produced identical codes and collided on
//! `world_invites_invite_code_key`. Spec 005 US4 hit this under ordinary
//! concurrent load and again in its own rapid-succession e2e test. Deriving
//! from v4 removes the collision class entirely rather than narrowing it.

use uuid::Uuid;

/// Characters taken from the hex representation of a v4 UUID.
///
/// 20 hex characters is ~80 bits — far past the point where guessing is
/// feasible, and comfortably inside the `VARCHAR(32)` that both
/// `world_invites.invite_code` and every `world_*_shares.share_code` column
/// already declare, so raising invites needs no width migration.
const CODE_LENGTH: usize = 20;

/// Generates an unguessable, non-time-derived link code.
///
/// Uppercase hex, so a code stays readable when a GM pastes it into chat and
/// survives being retyped without case ambiguity.
pub fn generate_link_code() -> String {
    Uuid::new_v4()
        .to_string()
        .replace('-', "")
        .chars()
        .take(CODE_LENGTH)
        .collect::<String>()
        .to_uppercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// FR-006 / SC-007: codes are 20 characters of uppercase hex.
    #[test]
    fn codes_are_twenty_uppercase_hex_characters() {
        for _ in 0..64 {
            let code = generate_link_code();
            assert_eq!(code.len(), CODE_LENGTH, "code length must be {CODE_LENGTH}");
            assert!(
                code.chars()
                    .all(|c| c.is_ascii_hexdigit() && !c.is_lowercase()),
                "code must be uppercase hex, got {code}"
            );
        }
    }

    /// The spec 005 regression guard. A v7-derived code would front-load a
    /// millisecond timestamp, so a rapid burst would share a long common
    /// prefix and eventually collide outright. Generating a large batch as
    /// fast as possible and asserting both uniqueness and prefix diversity
    /// fails loudly if the source is ever swapped back to a clock-based UUID.
    #[test]
    fn rapid_succession_codes_are_unique_and_show_no_time_ordering() {
        let batch: Vec<String> = (0..5_000).map(|_| generate_link_code()).collect();

        let unique: HashSet<&String> = batch.iter().collect();
        assert_eq!(
            unique.len(),
            batch.len(),
            "codes generated in rapid succession must not collide"
        );

        // With a v7 source, thousands of codes made in the same few
        // milliseconds would share their leading characters. With v4 they
        // should not: 5,000 draws over 16 first-characters makes a single
        // dominant prefix vanishingly unlikely.
        let mut first_char_counts = std::collections::HashMap::new();
        for code in &batch {
            *first_char_counts
                .entry(code.chars().next().unwrap())
                .or_insert(0usize) += 1;
        }
        assert!(
            first_char_counts.len() >= 8,
            "expected the leading character to vary widely; saw only {} distinct \
             values, which is what a time-derived source looks like",
            first_char_counts.len()
        );

        // Sorting must not reconstruct generation order for a v4 source. A
        // time-derived source would be almost perfectly sorted already.
        let mut sorted = batch.clone();
        sorted.sort();
        assert_ne!(
            sorted, batch,
            "generation order must not match sorted order — that is the \
             signature of a timestamp-derived code"
        );
    }
}
