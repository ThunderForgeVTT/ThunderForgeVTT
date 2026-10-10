//! An account's sheets in its export, and what deleting the account does to
//! them (spec 048 T082, T084).
//!
//! Every value is invented. The files written to storage are a few bytes of
//! made-up PDF header, not anybody's sheet.

use std::io::Read;

use diesel::prelude::*;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::compendium::origin::ContentOrigin;
use crate::schema::{
    actor_imports, brought_characters, sheet_import_versions, world_actors, world_staged_content,
    worlds,
};
use crate::sheet_import::{
    ActorImportKind, BroughtCharacter, NewBroughtCharacter, NewSheetImportVersion,
    SheetImportVersion, storage,
};
use crate::staged_content::NewStagedContent;
use crate::test_support::{
    insert_test_actor, insert_test_scene, insert_test_user, insert_test_world,
    insert_test_world_member, test_app_state,
};

/// A character `owner` brought, with one version, and the version's key.
fn a_sheet(conn: &mut PgConnection, owner: Uuid) -> (BroughtCharacter, SheetImportVersion) {
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
    let key = storage::object_key(owner, character.id, 1);
    let reading = json!({ "identity.name": "Invented Person" });
    let corrections = json!({});
    let version = diesel::insert_into(sheet_import_versions::table)
        .values(&NewSheetImportVersion {
            character_id: character.id,
            version_no: 1,
            file_key: &key,
            file_sha256: &"0".repeat(64),
            file_bytes: 8,
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
        .unwrap();
    (character, version)
}

fn empty_snapshot() -> Value {
    json!({ "label": "Test Actor", "systemData": {}, "abilities": [], "inventory": [] })
}

/// `by` imports `version` onto `actor`.
fn an_import(conn: &mut PgConnection, world: Uuid, actor: Uuid, version: Uuid, by: Uuid) -> Uuid {
    let id = Uuid::now_v7();
    diesel::insert_into(actor_imports::table)
        .values((
            actor_imports::id.eq(id),
            actor_imports::world_id.eq(world),
            actor_imports::actor_id.eq(actor),
            actor_imports::version_id.eq(Some(version)),
            actor_imports::kind.eq(ActorImportKind::Import),
            actor_imports::before_snapshot.eq(empty_snapshot()),
            actor_imports::written.eq(json!({ "fields": ["resources.hp_max"] })),
            actor_imports::created_by.eq(by),
            actor_imports::updated_by.eq(by),
        ))
        .execute(conn)
        .unwrap();
    id
}

fn owned_by(conn: &mut PgConnection, actor: Uuid, owner: Uuid) {
    diesel::update(world_actors::table.find(actor))
        .set(world_actors::owned_by.eq(owner))
        .execute(conn)
        .unwrap();
}

const PDF: &[u8] = b"%PDF-1.7";

#[tokio::test]
async fn the_export_carries_the_sheets_at_v4_and_the_zip_keeps_each_file() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let world = insert_test_world(&mut conn, owner);
    let scene = insert_test_scene(&mut conn, world, owner);
    let actor = insert_test_actor(&mut conn, world, scene, owner);
    let (character, version) = a_sheet(&mut conn, owner);
    let import = an_import(&mut conn, world, actor, version.id, owner);
    drop(conn);

    let export = super::export_user_data_payload(&state, owner)
        .await
        .expect("export");

    assert_eq!(export.manifest.schema_version, "v4");
    let counts = &export.manifest.counts;
    assert_eq!(
        (
            counts.brought_characters,
            counts.sheet_versions,
            counts.actor_imports
        ),
        (1, 1, 1)
    );
    let brought = &export.brought_characters[0];
    assert_eq!(brought.id, character.id);
    assert_eq!(brought.versions.len(), 1);
    let path = super::export_content::sheet_zip_path(character.id, 1);
    assert_eq!(brought.versions[0].file, path);
    assert_eq!(brought.versions[0].reading, version.reading);
    let record = &export.actor_imports[0];
    assert_eq!((record.id, record.kind), (import, "import"));
    assert_eq!(record.version_id, Some(version.id));

    let as_json = serde_json::to_value(&export).unwrap();
    assert!(
        as_json["actor_imports"][0].get("before_snapshot").is_none(),
        "the snapshot is the server's working state, not the person's data"
    );

    let zip = super::build_zip_export(&export, &[(path.clone(), PDF.to_vec())]).unwrap();
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip)).unwrap();
    let mut file = archive.by_name(&path).expect("the sheet is in the ZIP");
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, PDF);
}

#[test]
fn deleting_the_account_deletes_its_sheets_and_collects_their_files() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let world = insert_test_world(&mut conn, owner);
    let scene = insert_test_scene(&mut conn, world, owner);
    let actor = insert_test_actor(&mut conn, world, scene, owner);
    let (character, version) = a_sheet(&mut conn, owner);
    an_import(&mut conn, world, actor, version.id, owner);

    let summary = crate::users::delete_user_data_on(&mut conn, owner).unwrap();

    assert_eq!(summary.brought_characters_deleted, 1);
    assert_eq!(summary.sheet_file_keys, vec![version.file_key.clone()]);
    let left: (i64, i64, i64) = (
        brought_characters::table
            .filter(brought_characters::id.eq(character.id))
            .count()
            .get_result(&mut conn)
            .unwrap(),
        sheet_import_versions::table
            .filter(sheet_import_versions::character_id.eq(character.id))
            .count()
            .get_result(&mut conn)
            .unwrap(),
        actor_imports::table
            .filter(actor_imports::actor_id.eq(actor))
            .count()
            .get_result(&mut conn)
            .unwrap(),
    );
    assert_eq!(left, (0, 0, 0));
    assert!(
        serde_json::to_value(&summary)
            .unwrap()
            .get("sheet_file_keys")
            .is_none(),
        "the keys are not in the response"
    );
}

#[tokio::test]
async fn the_files_are_deleted_once_the_deletion_has_committed() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let (_, version) = a_sheet(&mut conn, owner);
    drop(conn);

    let cfg = crate::storage::rustfs::RustFsConfig::resolve(&state).await;
    crate::storage::rustfs::write_object(&cfg, &version.file_key, PDF.to_vec(), "application/pdf")
        .await
        .expect("write the invented sheet");
    assert!(
        crate::storage::rustfs::read_object(&cfg, &version.file_key)
            .await
            .is_ok()
    );

    let summary = super::delete_user_data_owned(&state, owner)
        .await
        .expect("deleted");

    assert_eq!(summary.users_deleted, 1);
    assert!(
        crate::storage::rustfs::read_object(&cfg, &version.file_key)
            .await
            .is_err(),
        "the file went with the account"
    );
}

#[test]
fn a_rescued_character_keeps_its_import_records() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let gm = insert_test_user(&mut conn);
    let player = insert_test_user(&mut conn);
    let campaign = insert_test_world(&mut conn, gm);
    diesel::update(worlds::table.find(campaign))
        .set(worlds::game_system_id.eq(Some("dnd5e".to_string())))
        .execute(&mut conn)
        .unwrap();
    let scene = insert_test_scene(&mut conn, campaign, gm);
    insert_test_world_member(&mut conn, campaign, player, "Player");
    let actor = insert_test_actor(&mut conn, campaign, scene, gm);
    owned_by(&mut conn, actor, player);

    // The player brought their sheet; later the GM brought one of their own
    // onto the same character.
    let (players_character, players_version) = a_sheet(&mut conn, player);
    an_import(&mut conn, campaign, actor, players_version.id, player);
    let (_, gms_version) = a_sheet(&mut conn, gm);
    an_import(&mut conn, campaign, actor, gms_version.id, gm);

    let summary = crate::users::delete_user_data_on(&mut conn, gm).unwrap();
    assert_eq!(summary.sheet_file_keys, vec![gms_version.file_key]);

    let rescued: Uuid = world_actors::table
        .inner_join(worlds::table)
        .filter(worlds::created_by.eq(player))
        .filter(world_actors::owned_by.eq(player))
        .select(world_actors::id)
        .first(&mut conn)
        .expect("the character was rescued");
    let records: Vec<(Option<Uuid>, Uuid)> = actor_imports::table
        .filter(actor_imports::actor_id.eq(rescued))
        .order(actor_imports::applied_at.asc())
        .select((actor_imports::version_id, actor_imports::created_by))
        .load(&mut conn)
        .unwrap();
    assert_eq!(
        records,
        vec![(Some(players_version.id), player), (None, player)],
        "both records came along; the GM's file was not kept"
    );
    let players_still: i64 = brought_characters::table
        .filter(brought_characters::id.eq(players_character.id))
        .count()
        .get_result(&mut conn)
        .unwrap();
    assert_eq!(players_still, 1, "the player's own sheet is theirs");
}

#[test]
fn what_a_co_gm_wrote_in_a_world_that_stays_passes_to_its_owner() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let co_gm = insert_test_user(&mut conn);
    let world = insert_test_world(&mut conn, owner);
    let scene = insert_test_scene(&mut conn, world, owner);
    insert_test_world_member(&mut conn, world, co_gm, "GM");
    let npc = insert_test_actor(&mut conn, world, scene, owner);
    let (_, version) = a_sheet(&mut conn, co_gm);
    let import = an_import(&mut conn, world, npc, version.id, co_gm);
    let fields = json!({ "level": 1 });
    let staged: Uuid = diesel::insert_into(world_staged_content::table)
        .values(&NewStagedContent {
            world_id: world,
            player_user_id: co_gm,
            kind: "spell",
            name: "Invented Bolt",
            normalized_name: "invented bolt",
            content_hash: &"b".repeat(64),
            field_values: &fields,
            origin: ContentOrigin::Uploaded,
            differs_from: None,
            first_actor_id: Some(npc),
            created_by: co_gm,
            updated_by: co_gm,
        })
        .returning(world_staged_content::id)
        .get_result(&mut conn)
        .unwrap();
    diesel::update(world_staged_content::table.find(staged))
        .set(world_staged_content::decided_by.eq(Some(co_gm)))
        .execute(&mut conn)
        .unwrap();

    crate::users::delete_user_data_on(&mut conn, co_gm).expect("the co-GM can leave");

    let record: (Option<Uuid>, Uuid, Uuid) = actor_imports::table
        .find(import)
        .select((
            actor_imports::version_id,
            actor_imports::created_by,
            actor_imports::updated_by,
        ))
        .first(&mut conn)
        .unwrap();
    assert_eq!(record, (None, owner, owner), "kept, file no longer kept");
    let row: (Uuid, Uuid, Option<Uuid>) = world_staged_content::table
        .find(staged)
        .select((
            world_staged_content::player_user_id,
            world_staged_content::created_by,
            world_staged_content::decided_by,
        ))
        .first(&mut conn)
        .unwrap();
    assert_eq!(row, (owner, owner, Some(owner)));
}
