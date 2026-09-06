use super::*;
use thunderforge_server::test_support::{
    insert_test_user, insert_test_world, insert_test_world_member, test_app_state,
};

fn insert_test_actor(
    conn: &mut PgConnection,
    world_id: Uuid,
    scene_id: Uuid,
    owner_id: Uuid,
) -> Uuid {
    use thunderforge_server::schema::world_actors;
    let now = Utc::now().naive_utc();
    let actor_id = Uuid::now_v7();
    diesel::insert_into(world_actors::table)
        .values((
            world_actors::id.eq(actor_id),
            world_actors::world_id.eq(world_id),
            world_actors::scene_id.eq(scene_id),
            world_actors::actor_type.eq("character"),
            world_actors::game_system_id.eq("genie"),
            world_actors::label.eq("Test Actor"),
            world_actors::created_by.eq(owner_id),
            world_actors::owned_by.eq(owner_id),
            world_actors::is_public.eq(false),
            world_actors::is_npc.eq(false),
            world_actors::created_at.eq(now),
            world_actors::updated_at.eq(now),
            world_actors::available_for_claim.eq(false),
        ))
        .execute(conn)
        .expect("failed to insert test actor");
    actor_id
}

fn insert_test_scene(conn: &mut PgConnection, world_id: Uuid, owner_id: Uuid) -> Uuid {
    use thunderforge_server::schema::scenes;
    let now = Utc::now().naive_utc();
    let scene_id = Uuid::now_v7();
    diesel::insert_into(scenes::table)
        .values((
            scenes::scene_id.eq(scene_id),
            scenes::world_id.eq(world_id),
            scenes::owner_id.eq(owner_id),
            scenes::name.eq("Test Scene"),
            scenes::created_at.eq(now),
            scenes::updated_at.eq(now),
        ))
        .execute(conn)
        .expect("failed to insert test scene");
    scene_id
}

async fn setup_active_session(state: &AppState) -> (Uuid, Uuid, Uuid, Uuid) {
    // Returns (world_id, owner_id/gm, player_id, session_id)
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    let player_id = insert_test_user(&mut conn);
    insert_test_world_member(&mut conn, world_id, player_id, "Player");
    drop(conn);

    let session = start_genie_session_impl(
        state,
        owner_id,
        false,
        StartGenieSessionInput {
            world_id,
            doom_clock_max: 4,
        },
    )
    .await
    .expect("GM should be able to start a session");

    (world_id, owner_id, player_id, session.id)
}

// The shop helpers live here rather than in `shop`, because the reward
// tests stock a shop too.
fn insert_test_item(conn: &mut PgConnection, world_id: Uuid, owner_id: Uuid, name: &str) -> Uuid {
    use thunderforge_server::schema::world_items;
    let now = Utc::now().naive_utc();
    let item_id = Uuid::now_v7();
    diesel::insert_into(world_items::table)
        .values((
            world_items::id.eq(item_id),
            world_items::world_id.eq(world_id),
            world_items::name.eq(name),
            world_items::created_by.eq(owner_id),
            world_items::created_at.eq(now),
            world_items::updated_at.eq(now),
        ))
        .execute(conn)
        .expect("failed to insert test item");
    item_id
}

fn stock_item(
    conn: &mut PgConnection,
    actor_id: Uuid,
    item_id: Uuid,
    quantity: i32,
    owner_id: Uuid,
) {
    use thunderforge_server::schema::world_actor_inventory;
    diesel::insert_into(world_actor_inventory::table)
        .values((
            world_actor_inventory::actor_id.eq(actor_id),
            world_actor_inventory::item_id.eq(item_id),
            world_actor_inventory::item_name_snapshot.eq("Test Item"),
            world_actor_inventory::quantity.eq(quantity),
            world_actor_inventory::created_by.eq(owner_id),
            world_actor_inventory::updated_by.eq(owner_id),
        ))
        .execute(conn)
        .expect("failed to stock item");
}

#[path = "mutations_session_tests.rs"]
mod session;

#[path = "mutations_shop_tests.rs"]
mod shop;

#[path = "mutations_reward_tests.rs"]
mod rewards;
