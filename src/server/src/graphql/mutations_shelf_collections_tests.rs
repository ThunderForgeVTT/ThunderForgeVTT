//! The resolvers' own checks: the manifest decides which systems a collection
//! may be kept for and which kinds go in it, and a download crosses the wire
//! as a file a person can open (spec 050 FR-009, FR-009a, FR-009b).

use super::*;
use crate::compendium::collections::FILE_FORMAT;
use crate::test_support::{insert_test_user, test_app_state};

/// A system that declares a `creature`, written to disk the way a pack writes
/// one.
struct DeclaredSystem {
    root: std::path::PathBuf,
    id: String,
}

impl DeclaredSystem {
    fn new() -> Self {
        let id = format!("test-system-{}", Uuid::now_v7().simple());
        let root = std::env::temp_dir().join(format!("tf-shelf-{}", Uuid::now_v7().simple()));
        let dir = root.join(&id);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("system.json"),
            serde_json::json!({
                "id": id,
                "contentPatterns": [{
                    "kind": "creature",
                    "shape": "anchored",
                    "anchor": "Armour Class",
                    "name": { "position": "before", "withinLines": 6, "prefer": "largest" },
                    "fields": [{ "key": "armourClass", "label": "Armour Class", "as": "integer" }],
                }],
            })
            .to_string(),
        )
        .unwrap();
        Self { root, id }
    }

    fn dir(&self) -> &str {
        self.root.to_str().unwrap()
    }
}

impl Drop for DeclaredSystem {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.root).ok();
    }
}

#[tokio::test]
async fn a_collection_is_kept_only_for_a_system_that_declares_its_kinds() {
    let state = test_app_state();
    let system = DeclaredSystem::new();
    let owner = insert_test_user(&mut state.db_pool.get().unwrap());

    let undeclared = create_shelf_collection_impl(
        &state,
        system.dir(),
        owner,
        "Nowhere".to_string(),
        "no-such-system".to_string(),
    )
    .await;
    assert!(undeclared.is_err(), "a system with no declared kinds");

    let collection = create_shelf_collection_impl(
        &state,
        system.dir(),
        owner,
        "Fen Folk".to_string(),
        system.id.clone(),
    )
    .await
    .expect("a declared system");
    assert_eq!(
        collection.origin,
        crate::graphql::queries::compendium::GraphQLContentOrigin::Authored
    );
    assert_eq!(collection.source_hash, None);

    let wrong_kind = write_shelf_collection_entry_impl(
        &state,
        system.dir(),
        owner,
        collection.id,
        "spaceship".to_string(),
        "Nebula".to_string(),
        None,
        Some("Not a creature.".to_string()),
    )
    .await;
    assert!(wrong_kind.is_err(), "a kind the system does not declare");

    let hag = write_shelf_collection_entry_impl(
        &state,
        system.dir(),
        owner,
        collection.id,
        "creature".to_string(),
        "Mire Hag".to_string(),
        Some(Json(
            [(
                "armourClass".to_string(),
                ReadValue::Clear("17".to_string()),
            )]
            .into_iter()
            .collect(),
        )),
        None,
    )
    .await
    .expect("a declared kind");
    assert_eq!(hag.page, None);
    assert_eq!(hag.book_title, "Fen Folk");

    let file = download_shelf_collection_impl(&state, owner, collection.id)
        .await
        .expect("the owner downloads their collection");
    assert_eq!(file.file_name, "Fen Folk.json");
    assert_eq!(file.entry_count, 1);
    assert!(file.excluded.is_empty());
    let opened: serde_json::Value = serde_json::from_str(&file.contents).expect("the file is JSON");
    assert_eq!(opened["format"], FILE_FORMAT);
    assert_eq!(opened["entries"][0]["name"], "Mire Hag");
    assert_eq!(
        opened["entries"][0]["fieldValues"]["armourClass"]["value"],
        "17"
    );

    let stranger = insert_test_user(&mut state.db_pool.get().unwrap());
    assert!(
        download_shelf_collection_impl(&state, stranger, collection.id)
            .await
            .is_err()
    );
    assert!(
        remove_shelf_collection_entry_impl(&state, owner, collection.id, hag.id)
            .await
            .unwrap()
    );
}

#[test]
fn a_file_name_is_safe_to_save() {
    assert_eq!(file_name_for("Fen Folk"), "Fen Folk.json");
    assert_eq!(file_name_for("../../etc/passwd"), "------etc-passwd.json");
    assert_eq!(file_name_for("???"), "---.json");
    assert_eq!(file_name_for("   "), "collection.json");
}
