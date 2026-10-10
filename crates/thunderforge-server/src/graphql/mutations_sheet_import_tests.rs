//! Spec 048 T035: bringing a D&D Beyond sheet onto an actor, end to end on
//! the server — preview, apply, the rules on who may, and what a refusal
//! leaves behind.
//!
//! The sheets are the generated fixtures in `packs/systems/dnd5e/sheet`,
//! with invented values. The flag is written on for every case here; the
//! flag-off refusal is asked of `require_enabled`, so no test switches the
//! instance's flag off under another.

use diesel::prelude::*;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::sheet_import::apply::{ApplyInput, apply_sheet_import_impl, read_natively};
use crate::sheet_import::preview::{require_enabled, sheet_import_preview_impl};
use crate::sheet_import::records::actor_imports_impl;
use crate::state::AppState;
use crate::storage::rustfs::{RustFsConfig, read_object};
use crate::test_support::{
    insert_test_actor, insert_test_scene, insert_test_user, insert_test_world,
    insert_test_world_member, test_app_state,
};
use thunderforge_sheet_import::plan_hash;

const FIGHTER_WIZARD: &[u8] =
    include_bytes!("../../../../packs/systems/dnd5e/sheet/tests/fixtures/fighter3-wizard2.pdf");
const NOT_A_SHEET: &[u8] =
    include_bytes!("../../../../packs/systems/dnd5e/sheet/tests/fixtures/not-a-ddb-sheet.pdf");

fn systems_dir() -> String {
    format!("{}/../../packs/systems", env!("CARGO_MANIFEST_DIR"))
}

fn flag_on(state: &AppState) {
    use crate::schema::instance_settings;
    let mut conn = state.db_pool.get().expect("conn");
    let now = chrono::Utc::now().naive_utc();
    diesel::insert_into(instance_settings::table)
        .values((
            instance_settings::key.eq(crate::settings::features::SHEET_IMPORT),
            instance_settings::value.eq("true"),
            instance_settings::updated_at.eq(now),
            instance_settings::created_at.eq(now),
        ))
        .on_conflict(instance_settings::key)
        .do_update()
        .set(instance_settings::value.eq("true"))
        .execute(&mut conn)
        .expect("flag written");
}

struct Table {
    state: AppState,
    gm: Uuid,
    world: Uuid,
    actor: Uuid,
}

/// A world with its GM and one claimable 5e player character.
fn table() -> Table {
    use crate::schema::world_actors;
    let state = test_app_state();
    flag_on(&state);
    let mut conn = state.db_pool.get().expect("conn");
    let gm = insert_test_user(&mut conn);
    let world = insert_test_world(&mut conn, gm);
    insert_test_world_member(&mut conn, world, gm, "GM");
    let scene = insert_test_scene(&mut conn, world, gm);
    let actor = insert_test_actor(&mut conn, world, scene, gm);
    diesel::update(world_actors::table.find(actor))
        .set((
            world_actors::is_npc.eq(false),
            world_actors::actor_type.eq("character"),
            world_actors::available_for_claim.eq(true),
        ))
        .execute(&mut conn)
        .expect("a claimable character");
    drop(conn);
    Table {
        state,
        gm,
        world,
        actor,
    }
}

impl Table {
    fn member(&self, role: &str) -> Uuid {
        let mut conn = self.state.db_pool.get().expect("conn");
        let user = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, self.world, user, role);
        user
    }

    fn grant(&self, user: Uuid, level: &str) {
        use crate::schema::world_actor_permissions as perms;
        let mut conn = self.state.db_pool.get().expect("conn");
        let now = chrono::Utc::now().naive_utc();
        diesel::insert_into(perms::table)
            .values((
                perms::id.eq(Uuid::now_v7()),
                perms::actor_id.eq(self.actor),
                perms::user_id.eq(user),
                perms::level.eq(level),
                perms::created_at.eq(now),
                perms::updated_at.eq(now),
            ))
            .execute(&mut conn)
            .expect("permission granted");
    }

    async fn claimant(&self) -> Uuid {
        let player = self.member("Player");
        crate::graphql::mutations_actor_claims::claim_actor_impl(
            &self.state,
            player,
            self.world,
            self.actor,
        )
        .await
        .expect("the player claims the character");
        player
    }

    async fn preview(&self, user: Uuid, corrections: Option<Value>) -> Result<String, String> {
        let (_, reading) = read_natively("dnd5e", FIGHTER_WIZARD).expect("the fixture reads");
        sheet_import_preview_impl(
            &self.state,
            &systems_dir(),
            user,
            false,
            self.actor,
            &reading,
            corrections.as_ref(),
        )
        .await
        .map(|plan| plan_hash(&plan))
        .map_err(|e| e.code())
    }

    async fn apply(
        &self,
        user: Uuid,
        bytes: &[u8],
        corrections: Option<Value>,
        hash: String,
    ) -> Result<crate::sheet_import::ActorImport, String> {
        apply_sheet_import_impl(
            &self.state,
            &systems_dir(),
            user,
            false,
            ApplyInput {
                actor_id: self.actor,
                bytes: bytes.to_vec(),
                corrections,
                overwrite_play_state: Vec::new(),
                plan_hash: hash,
            },
        )
        .await
        .map_err(|e| e.code())
    }

    /// What an import writes, counted.
    fn counts(&self) -> (i64, i64, i64, i64) {
        use crate::schema::{
            actor_imports, world_actor_abilities, world_actor_inventory, world_staged_content,
        };
        let mut conn = self.state.db_pool.get().expect("conn");
        let imports = actor_imports::table
            .filter(actor_imports::actor_id.eq(self.actor))
            .count()
            .get_result(&mut conn)
            .unwrap();
        let abilities = world_actor_abilities::table
            .filter(world_actor_abilities::actor_id.eq(self.actor))
            .count()
            .get_result(&mut conn)
            .unwrap();
        let items = world_actor_inventory::table
            .filter(world_actor_inventory::actor_id.eq(self.actor))
            .count()
            .get_result(&mut conn)
            .unwrap();
        let staged = world_staged_content::table
            .filter(world_staged_content::world_id.eq(self.world))
            .count()
            .get_result(&mut conn)
            .unwrap();
        (imports, abilities, items, staged)
    }

    fn system_data(&self) -> Option<(Option<Value>, Option<Value>)> {
        use crate::schema::world_actor_system_data as data;
        let mut conn = self.state.db_pool.get().expect("conn");
        data::table
            .filter(data::actor_id.eq(self.actor))
            .select((data::ability_data, data::trait_data))
            .first(&mut conn)
            .optional()
            .unwrap()
    }
}

#[tokio::test]
async fn the_preview_writes_nothing() {
    let t = table();
    let player = t.claimant().await;
    let before = t.counts();
    t.preview(player, None).await.expect("a plan");
    assert_eq!(t.counts(), before);
    assert_eq!(t.system_data(), None);
}

#[tokio::test]
async fn a_declined_review_uploads_nothing() {
    use crate::schema::{brought_characters, sheet_import_versions};
    let t = table();
    let player = t.claimant().await;
    // Declining is not calling apply: the preview is all that ran.
    t.preview(player, None).await.expect("a plan");
    let mut conn = t.state.db_pool.get().expect("conn");
    let characters: i64 = brought_characters::table
        .filter(brought_characters::owner_user_id.eq(player))
        .count()
        .get_result(&mut conn)
        .unwrap();
    let versions: i64 = sheet_import_versions::table
        .filter(sheet_import_versions::created_by.eq(player))
        .count()
        .get_result(&mut conn)
        .unwrap();
    assert_eq!((characters, versions), (0, 0));
}

#[tokio::test]
async fn an_apply_writes_the_sheet_links_version_record_origin_and_event() {
    use crate::schema::{sheet_import_versions, world_actors, world_events};
    let t = table();
    let player = t.claimant().await;
    let hash = t.preview(player, None).await.expect("a plan");
    let import = t
        .apply(player, FIGHTER_WIZARD, None, hash)
        .await
        .expect("applied");

    let (imports, abilities, items, staged) = t.counts();
    assert_eq!(imports, 1);
    assert!(abilities > 0, "spells and features are linked");
    assert!(items > 0, "the gear is carried");
    assert!(staged > 0, "content new to the world is staged");

    let (ability_data, _) = t.system_data().expect("system data written");
    assert!(ability_data.is_some(), "the scores land");

    let mut conn = t.state.db_pool.get().expect("conn");
    let origin: crate::compendium::origin::ContentOrigin = world_actors::table
        .find(t.actor)
        .select(world_actors::origin)
        .first(&mut conn)
        .unwrap();
    assert_eq!(origin, crate::compendium::origin::ContentOrigin::Uploaded);

    let (key, pages): (String, i16) = sheet_import_versions::table
        .find(import.version_id.expect("an import has a version"))
        .select((
            sheet_import_versions::file_key,
            sheet_import_versions::file_pages,
        ))
        .first(&mut conn)
        .unwrap();
    assert!(pages >= 1);
    let cfg = RustFsConfig::resolve(&t.state).await;
    assert_eq!(read_object(&cfg, &key).await.unwrap(), FIGHTER_WIZARD);

    let events: Vec<Option<Value>> = world_events::table
        .filter(world_events::world_id.eq(t.world))
        .filter(world_events::event_code.eq(40))
        .select(world_events::token_event)
        .load(&mut conn)
        .unwrap();
    assert_eq!(
        events,
        vec![Some(json!({ "actorId": t.actor, "importId": import.id }))]
    );

    let history = actor_imports_impl(&t.state, player, false, t.actor)
        .await
        .unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].version_no, Some(1));
    assert!(history[0].file_available, "the owner may download");
}

#[tokio::test]
async fn the_gm_may_import_onto_any_actor_in_the_world() {
    let t = table();
    let hash = t.preview(t.gm, None).await.expect("a plan");
    t.apply(t.gm, FIGHTER_WIZARD, None, hash)
        .await
        .expect("applied");
    assert_eq!(t.counts().0, 1);
}

#[tokio::test]
async fn a_viewer_and_a_stranger_are_refused() {
    let t = table();
    let viewer = t.member("Player");
    t.grant(viewer, "Viewer");
    let stranger = {
        let mut conn = t.state.db_pool.get().expect("conn");
        insert_test_user(&mut conn)
    };
    let hash = t.preview(t.gm, None).await.expect("a plan");
    for user in [viewer, stranger] {
        assert_eq!(t.preview(user, None).await.unwrap_err(), "FORBIDDEN");
        assert_eq!(
            t.apply(user, FIGHTER_WIZARD, None, hash.clone())
                .await
                .unwrap_err(),
            "FORBIDDEN"
        );
    }
    assert_eq!(t.counts().0, 0);
    // The viewer still sees the (empty) history.
    assert!(
        actor_imports_impl(&t.state, viewer, false, t.actor)
            .await
            .unwrap()
            .is_empty()
    );
}

#[test]
fn the_flag_off_is_refused() {
    assert_eq!(
        require_enabled(false).unwrap_err().code(),
        "FEATURE_DISABLED"
    );
}

#[tokio::test]
async fn a_changed_plan_is_refused_and_writes_nothing() {
    let t = table();
    let player = t.claimant().await;
    let before = t.counts();
    let error = t
        .apply(player, FIGHTER_WIZARD, None, "0".repeat(64))
        .await
        .unwrap_err();
    assert_eq!(error, "PLAN_CHANGED");
    assert_eq!(t.counts(), before);
    assert_eq!(t.system_data(), None);
}

#[tokio::test]
async fn a_document_that_is_not_a_sheet_is_not_recognised() {
    let t = table();
    let player = t.claimant().await;
    let error = t
        .apply(player, NOT_A_SHEET, None, "0".repeat(64))
        .await
        .unwrap_err();
    assert_eq!(error, "SHEET_NOT_RECOGNISED");
}

#[tokio::test]
async fn a_failed_transaction_deletes_the_stored_file() {
    let t = table();
    let player = t.claimant().await;
    let hash = t.preview(player, None).await.expect("a plan");
    let first = t
        .apply(player, FIGHTER_WIZARD, None, hash)
        .await
        .expect("the first import");

    // A correction the validator refuses fails the second import inside its
    // transaction, after the file was stored.
    let bad = json!({ "abilities.str": "a great deal" });
    let hash = t.preview(player, Some(bad.clone())).await.expect("a plan");
    let error = t
        .apply(player, FIGHTER_WIZARD, Some(bad), hash)
        .await
        .unwrap_err();
    assert_eq!(error, "VALIDATION_FAILED");
    assert_eq!(t.counts().0, 1, "only the first import is recorded");

    let character = {
        use crate::schema::sheet_import_versions as versions;
        let mut conn = t.state.db_pool.get().expect("conn");
        versions::table
            .find(first.version_id.unwrap())
            .select(versions::character_id)
            .first::<Uuid>(&mut conn)
            .unwrap()
    };
    let cfg = RustFsConfig::resolve(&t.state).await;
    let second = crate::sheet_import::storage::object_key(player, character, 2);
    assert!(
        read_object(&cfg, &second).await.is_err(),
        "the file is gone"
    );
    let first_key = crate::sheet_import::storage::object_key(player, character, 1);
    assert!(read_object(&cfg, &first_key).await.is_ok());
}

/// T037, FR-037: a link to staged content is withheld from every read that
/// play is composed from, not greyed out — the roll buttons and the combat
/// menu read `actorAbilities` and `actorInventory`, an attack resolves its
/// weapon through `find_weapon`, and the compendium lists world content.
#[tokio::test]
async fn a_staged_link_is_absent_from_every_play_read() {
    use crate::schema::{world_actor_abilities as abilities, world_actor_inventory as inventory};
    let t = table();
    let player = t.claimant().await;
    let hash = t.preview(player, None).await.expect("a plan");
    t.apply(player, FIGHTER_WIZARD, None, hash)
        .await
        .expect("applied");

    let mut conn = t.state.db_pool.get().expect("conn");
    let staged_abilities: Vec<(Uuid, Uuid, String)> = abilities::table
        .filter(abilities::actor_id.eq(t.actor))
        .filter(abilities::staged_id.is_not_null())
        .select((
            abilities::id,
            abilities::staged_id.assume_not_null(),
            abilities::ability_name_snapshot,
        ))
        .load(&mut conn)
        .unwrap();
    let staged_items: Vec<(Uuid, Uuid, String)> = inventory::table
        .filter(inventory::actor_id.eq(t.actor))
        .filter(inventory::staged_id.is_not_null())
        .select((
            inventory::id,
            inventory::staged_id.assume_not_null(),
            inventory::item_name_snapshot,
        ))
        .load(&mut conn)
        .unwrap();
    assert!(!staged_abilities.is_empty(), "the fixture stages spells");
    assert!(!staged_items.is_empty(), "the fixture stages gear");

    for user in [player, t.gm] {
        let delivered = crate::graphql::mutations_actor_abilities::actor_abilities_impl(
            &t.state, user, false, t.actor,
        )
        .await
        .unwrap();
        for (link, _, _) in &staged_abilities {
            assert!(delivered.iter().all(|entry| entry.id != *link));
        }
        let carried = crate::graphql::queries::inventory::actor_inventory_impl(
            &t.state, user, false, t.actor,
        )
        .await
        .unwrap();
        for (link, _, _) in &staged_items {
            assert!(carried.iter().all(|entry| entry.id != *link));
        }

        let world_abilities = crate::graphql::queries::ability::world_abilities_impl(
            &t.state, user, false, t.world, None,
        )
        .await
        .unwrap();
        for (_, _, name) in &staged_abilities {
            assert!(
                world_abilities.iter().all(|a| a.name != *name),
                "{name} is listed"
            );
        }
        let world_items =
            crate::graphql::queries::item::world_items_impl(&t.state, user, false, t.world, None)
                .await
                .unwrap();
        for (_, _, name) in &staged_items {
            assert!(
                world_items.iter().all(|i| i.name != *name),
                "{name} is listed"
            );
        }
    }

    // Neither the player nor the GM can swing what was never adopted.
    use crate::combat::weapon::find_weapon;
    for runs_the_world in [false, true] {
        for (_, staged, _) in &staged_abilities {
            assert!(
                find_weapon(
                    &mut conn,
                    t.world,
                    Some(t.actor),
                    runs_the_world,
                    Some(*staged),
                    None
                )
                .is_err()
            );
        }
        for (_, staged, _) in &staged_items {
            assert!(
                find_weapon(
                    &mut conn,
                    t.world,
                    Some(t.actor),
                    runs_the_world,
                    None,
                    Some(*staged)
                )
                .is_err()
            );
        }
    }
}
