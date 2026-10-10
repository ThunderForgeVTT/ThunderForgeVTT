use super::tests::read_fixture;
use super::*;

/// Hotfix (vtt-dev, 2026-10-09): an import rewrites the scene's art and size
/// and must say so in `updated_at`. It did not — the row kept its creation
/// time, so it read as never having changed after a map was put on it.
#[tokio::test]
async fn an_import_bumps_the_scenes_updated_at() {
    use crate::schema::scenes;
    use crate::test_support::*;

    crate::test_support::load_dotenv();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
    let long_ago = chrono::NaiveDate::from_ymd_opt(2020, 1, 1)
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap();
    diesel::update(scenes::table.filter(scenes::scene_id.eq(scene_id)))
        .set(scenes::updated_at.eq(long_ago))
        .execute(&mut conn)
        .expect("age the scene");
    drop(conn);

    import_uvtt_impl(
        &state,
        owner_id,
        false,
        scene_id,
        read_fixture("little-fish-academy.dd2vtt"),
        false,
    )
    .await
    .expect("little-fish-academy should import");

    let mut conn = state.db_pool.get().unwrap();
    let updated_at: chrono::NaiveDateTime = scenes::table
        .filter(scenes::scene_id.eq(scene_id))
        .select(scenes::updated_at)
        .first(&mut conn)
        .expect("scene should reload");
    assert!(
        updated_at > long_ago,
        "an import must bump the scene's updated_at, still {updated_at}"
    );
}
