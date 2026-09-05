use super::*;
use crate::test_support::*;
use uuid::Uuid;

/// Insert an asset row directly, so these tests exercise the lookup rather
/// than the whole upload path.
fn insert_asset(
    conn: &mut PgConnection,
    world_id: Uuid,
    owner: Uuid,
    storage_path: &str,
    content_hash: Option<&str>,
) -> Uuid {
    use crate::schema::canvas_image_assets as assets;

    let asset_id = Uuid::now_v7();
    let now = chrono::Utc::now().naive_utc();
    diesel::insert_into(assets::table)
        .values((
            assets::asset_id.eq(asset_id),
            assets::world_id.eq(world_id),
            assets::scene_id.eq(None::<Uuid>),
            assets::owner_user_id.eq(owner),
            assets::storage_path.eq(storage_path),
            assets::original_format.eq("png"),
            assets::width_px.eq(64),
            assets::height_px.eq(64),
            assets::byte_size.eq(1024_i64),
            assets::kind.eq(crate::db_types::CanvasImageAssetKindEnum::Background),
            assets::created_by.eq(owner),
            assets::updated_by.eq(owner),
            assets::created_at.eq(now),
            assets::updated_at.eq(now),
            assets::content_hash.eq(content_hash),
        ))
        .execute(conn)
        .expect("asset row should insert");
    asset_id
}

fn unique_hash() -> String {
    // A real-shaped digest, unique per test run so a shared development
    // database cannot make one test's rows answer another's lookup.
    format!("{:064x}", Uuid::now_v7().as_u128())
}

#[test]
fn bytes_already_stored_are_found_by_their_hash() {
    dotenvy::dotenv().ok();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let world = insert_test_world(&mut conn, owner);

    let hash = unique_hash();
    insert_asset(
        &mut conn,
        world,
        owner,
        "worlds/a/original.webp",
        Some(&hash),
    );

    assert_eq!(
        object_holding(&mut conn, &hash).as_deref(),
        Some("worlds/a/original.webp")
    );
}

/// The case the whole feature is for, and the one a world-scoped lookup would
/// miss: 3,815 of the duplicated rows measured on 2026-09-03 share their bytes
/// with a row in a *different* world.
#[test]
fn the_same_image_in_a_different_world_is_still_found() {
    dotenvy::dotenv().ok();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_a = insert_test_user(&mut conn);
    let world_a = insert_test_world(&mut conn, owner_a);
    let owner_b = insert_test_user(&mut conn);
    let world_b = insert_test_world(&mut conn, owner_b);

    let hash = unique_hash();
    insert_asset(
        &mut conn,
        world_a,
        owner_a,
        "worlds/a/map.webp",
        Some(&hash),
    );

    // A different world, a different owner, the same bytes.
    let found = object_holding(&mut conn, &hash);
    assert_eq!(found.as_deref(), Some("worlds/a/map.webp"));

    // And world B's own row can now name that object while remaining its own
    // row — which is what keeps the permission check per-world.
    let b_asset = insert_asset(&mut conn, world_b, owner_b, &found.unwrap(), Some(&hash));
    use crate::schema::canvas_image_assets as assets;
    let (b_world, b_path) = assets::table
        .filter(assets::asset_id.eq(b_asset))
        .select((assets::world_id, assets::storage_path))
        .first::<(Uuid, String)>(&mut conn)
        .expect("world B's asset should load");
    assert_eq!(
        b_world, world_b,
        "the row belongs to the world that made it"
    );
    assert_eq!(b_path, "worlds/a/map.webp", "and shares only the bytes");
}

#[test]
fn different_bytes_are_never_confused_for_each_other() {
    dotenvy::dotenv().ok();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let world = insert_test_world(&mut conn, owner);

    let stored = unique_hash();
    insert_asset(&mut conn, world, owner, "worlds/a/one.webp", Some(&stored));

    assert_eq!(
        object_holding(&mut conn, &unique_hash()),
        None,
        "a hash nothing holds must not resolve to somebody else's object"
    );
}

/// Rows written before the hash column existed answer nothing.
///
/// They are not *wrong* — `storage/backfill.rs` is filling them in behind live
/// traffic — but an un-hashed row cannot be shown to hold any particular
/// bytes, and reusing its object on a guess would serve one world another
/// world's image.
#[test]
fn an_unhashed_row_is_never_reused() {
    dotenvy::dotenv().ok();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let world = insert_test_world(&mut conn, owner);

    insert_asset(&mut conn, world, owner, "worlds/a/legacy.webp", None);

    // Not even for the empty string, which is what a careless NULL-to-text
    // conversion would produce.
    assert_eq!(object_holding(&mut conn, ""), None);
}

/// Deterministic, so repeated uploads of one image converge on one object
/// rather than scattering across whichever row the planner returned first.
#[test]
fn the_oldest_copy_is_the_one_reused() {
    dotenvy::dotenv().ok();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let world = insert_test_world(&mut conn, owner);
    let hash = unique_hash();

    use crate::schema::canvas_image_assets as assets;
    let first = insert_asset(&mut conn, world, owner, "worlds/a/first.webp", Some(&hash));
    insert_asset(&mut conn, world, owner, "worlds/a/second.webp", Some(&hash));

    // Ages made explicit rather than relying on insertion speed: two rows
    // written in the same millisecond would make this test flap.
    diesel::update(assets::table.filter(assets::asset_id.eq(first)))
        .set(assets::created_at.eq(chrono::Utc::now().naive_utc() - chrono::Duration::days(1)))
        .execute(&mut conn)
        .expect("ageing the first row should succeed");

    assert_eq!(
        object_holding(&mut conn, &hash).as_deref(),
        Some("worlds/a/first.webp")
    );
}
