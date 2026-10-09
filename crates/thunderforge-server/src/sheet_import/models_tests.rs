//! The rules the sheet-import tables keep themselves, whatever writes to
//! them (spec 048 data-model §2).

use super::*;
use crate::compendium::origin::ContentOrigin;
use crate::schema::world_staged_content;
use crate::staged_content::{NewStagedContent, StagedContent, StagedState};
use crate::test_support::{
    insert_test_actor, insert_test_scene, insert_test_user, insert_test_world, test_app_state,
};
use serde_json::json;

fn a_version(conn: &mut PgConnection, owner: Uuid) -> SheetImportVersion {
    let character: BroughtCharacter = diesel::insert_into(brought_characters::table)
        .values(&NewBroughtCharacter {
            owner_user_id: owner,
            system_id: "dnd5e",
            name: "Invented Person",
            created_by: owner,
            updated_by: owner,
        })
        .returning(BroughtCharacter::as_returning())
        .get_result(conn)
        .unwrap();
    let reading = json!({});
    let corrections = json!({});
    let key = format!("sheets/{owner}/{}/1.pdf", character.id);
    diesel::insert_into(sheet_import_versions::table)
        .values(&NewSheetImportVersion {
            character_id: character.id,
            version_no: 1,
            file_key: &key,
            file_sha256: &"0".repeat(64),
            file_bytes: 1,
            file_pages: 1,
            reader_id: "ddb-pdf",
            reader_version: "1",
            reading: &reading,
            corrections: &corrections,
            created_by: owner,
            updated_by: owner,
        })
        .returning(SheetImportVersion::as_returning())
        .get_result(conn)
        .unwrap()
}

fn staged(world: Uuid, owner: Uuid, origin: ContentOrigin, value: &Value) -> NewStagedContent<'_> {
    NewStagedContent {
        world_id: world,
        player_user_id: owner,
        kind: "spell",
        name: "Invented Bolt",
        normalized_name: "invented bolt",
        content_hash: "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2",
        field_values: value,
        origin,
        differs_from: None,
        first_actor_id: None,
        created_by: owner,
        updated_by: owner,
    }
}

#[test]
fn staged_content_is_uploaded_and_starts_pending() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let world = insert_test_world(&mut conn, owner);
    let fields = json!({ "level": 1 });

    let refused = diesel::insert_into(world_staged_content::table)
        .values(&staged(world, owner, ContentOrigin::Authored, &fields))
        .execute(&mut conn);
    assert!(refused.is_err(), "authored content cannot be staged");

    let row: StagedContent = diesel::insert_into(world_staged_content::table)
        .values(&staged(world, owner, ContentOrigin::Uploaded, &fields))
        .returning(StagedContent::as_returning())
        .get_result(&mut conn)
        .unwrap();
    assert_eq!(row.state, StagedState::Pending);

    // The same piece read twice is one row.
    let again = diesel::insert_into(world_staged_content::table)
        .values(&staged(world, owner, ContentOrigin::Uploaded, &fields))
        .execute(&mut conn);
    assert!(again.is_err());

    // Adopted names what it became; nothing else may claim to be adopted.
    let bare = diesel::update(world_staged_content::table.find(row.id))
        .set(world_staged_content::state.eq(StagedState::Adopted))
        .execute(&mut conn);
    assert!(bare.is_err(), "adopted with no adopted row");
}

#[test]
fn an_import_names_its_version_and_a_rollback_what_it_restores() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let world = insert_test_world(&mut conn, owner);
    let scene = insert_test_scene(&mut conn, world, owner);
    let actor = insert_test_actor(&mut conn, world, scene, owner);
    let version = a_version(&mut conn, owner);
    let snapshot = json!({ "fields": {}, "links": [] });
    let row = |kind, version_id, restored_from| NewActorImport {
        world_id: world,
        actor_id: actor,
        version_id,
        kind,
        restored_from,
        before_snapshot: &snapshot,
        written: &snapshot,
        plan_hash: None,
        created_by: owner,
        updated_by: owner,
    };

    let import: ActorImport = diesel::insert_into(actor_imports::table)
        .values(&row(ActorImportKind::Import, Some(version.id), None))
        .returning(ActorImport::as_returning())
        .get_result(&mut conn)
        .unwrap();
    assert_eq!(import.kind, ActorImportKind::Import);

    for (kind, version_id, restored_from) in [
        (ActorImportKind::Import, None, None),
        (ActorImportKind::Import, Some(version.id), Some(import.id)),
        (ActorImportKind::Rollback, None, None),
    ] {
        let refused = diesel::insert_into(actor_imports::table)
            .values(&row(kind, version_id, restored_from))
            .execute(&mut conn);
        assert!(
            refused.is_err(),
            "{kind:?} {version_id:?} {restored_from:?}"
        );
    }
    diesel::insert_into(actor_imports::table)
        .values(&row(ActorImportKind::Rollback, None, Some(import.id)))
        .execute(&mut conn)
        .expect("a rollback names the import it undoes");
}
