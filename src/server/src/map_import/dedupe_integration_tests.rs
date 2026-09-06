use super::*;
use crate::schema::canvas_image_assets;

/// The same map imported twice stores its bytes once.
///
/// Measured before this existed: 4,387 asset rows holding 61 distinct
/// images — 2,695 MB of storage for 116 MB of content, because importing a
/// map into a second world wrote the whole thing again.
///
/// Two *different* worlds deliberately. Restricting reuse to one world
/// would have reclaimed 3.8 MB of that 2,579 MB: the duplication is almost
/// entirely across worlds, which is exactly what a shared map looks like.
#[tokio::test]
#[ignore = "requires a reachable RustFS and Postgres"]
async fn the_same_map_imported_into_two_worlds_is_stored_once() {
    use crate::test_support::*;

    dotenvy::dotenv().ok();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();

    let owner_a = insert_test_user(&mut conn);
    let world_a = insert_test_world(&mut conn, owner_a);
    let scene_a = insert_test_scene(&mut conn, world_a, owner_a);

    let owner_b = insert_test_user(&mut conn);
    let world_b = insert_test_world(&mut conn, owner_b);
    let scene_b = insert_test_scene(&mut conn, world_b, owner_b);
    drop(conn);

    let raw = super::tests::read_fixture("chamber-of-echoing-grief.dd2vtt");
    import_uvtt_impl(&state, owner_a, false, scene_a, raw.clone())
        .await
        .expect("first import should succeed");
    import_uvtt_impl(&state, owner_b, false, scene_b, raw)
        .await
        .expect("second import should succeed");

    let mut conn = state.db_pool.get().unwrap();
    let paths: Vec<(uuid::Uuid, String)> = canvas_image_assets::table
        .filter(canvas_image_assets::world_id.eq_any([world_a, world_b]))
        .select((
            canvas_image_assets::world_id,
            canvas_image_assets::storage_path,
        ))
        .load(&mut conn)
        .expect("both assets should load");

    assert_eq!(paths.len(), 2, "each world still gets its own asset row");
    assert_eq!(
        paths[0].1, paths[1].1,
        "and both rows name one stored object"
    );
    assert_ne!(
        paths[0].0, paths[1].0,
        "while belonging to different worlds — which is what keeps the \
         permission check per-world"
    );
}
