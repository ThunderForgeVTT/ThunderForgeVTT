//! Where an uploaded sheet is kept (spec 048 FR-031).
//!
//! One file per version, under the player who brought it, so deleting the
//! character deletes exactly its files. The key is derived, never taken from
//! the request, as `feedback::object_key` is.

use diesel::prelude::*;
use uuid::Uuid;

use crate::schema::{brought_characters, sheet_import_versions};

/// The prefix every sheet is stored under. `storage::rustfs` keeps its own
/// copy for its delete rule, and a test pins the two together.
pub const STORAGE_PREFIX: &str = "sheets/";

/// `sheets/{owner}/{character}/{version}.pdf`.
pub fn object_key(owner: Uuid, character: Uuid, version: i32) -> String {
    format!("{STORAGE_PREFIX}{owner}/{character}/{version}.pdf")
}

/// Every file `owner` keeps: the character, the version number and the key.
/// The export packs them (T083); deleting the account deletes them (T085).
pub fn file_keys_of_sync(
    conn: &mut PgConnection,
    owner: Uuid,
) -> QueryResult<Vec<(Uuid, i32, String)>> {
    sheet_import_versions::table
        .inner_join(brought_characters::table)
        .filter(brought_characters::owner_user_id.eq(owner))
        .order((
            brought_characters::created_at.asc(),
            sheet_import_versions::version_no.asc(),
        ))
        .select((
            sheet_import_versions::character_id,
            sheet_import_versions::version_no,
            sheet_import_versions::file_key,
        ))
        .load(conn)
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
