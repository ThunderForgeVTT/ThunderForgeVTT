//! Where an uploaded sheet is kept (spec 048 FR-031).
//!
//! One file per version, under the player who brought it, so deleting the
//! character deletes exactly its files. The key is derived, never taken from
//! the request, as `feedback::object_key` is.

use uuid::Uuid;

/// The prefix every sheet is stored under. `storage::rustfs` keeps its own
/// copy for its delete rule, and a test pins the two together.
pub const STORAGE_PREFIX: &str = "sheets/";

/// `sheets/{owner}/{character}/{version}.pdf`.
pub fn object_key(owner: Uuid, character: Uuid, version: i32) -> String {
    format!("{STORAGE_PREFIX}{owner}/{character}/{version}.pdf")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sheet_key_is_derived_from_its_owner_character_and_version() {
        let owner = Uuid::from_u128(1);
        let character = Uuid::from_u128(2);
        assert_eq!(
            object_key(owner, character, 3),
            format!("sheets/{owner}/{character}/3.pdf")
        );
    }
}
