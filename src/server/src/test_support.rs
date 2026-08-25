//! Spec 002: shared test fixtures for integration tests that need a real
//! `AppState` (DB pool) — and, for storage tests, a real RustFS instance.
//!
//! Requires `DATABASE_URL` pointing at a live Postgres with migrations
//! applied (the project's normal local-dev Postgres is fine — see
//! `compose.yml`), and, for tests that exercise `storage::rustfs`, a
//! reachable RustFS (`docker compose up -d rustfs`). Tests that need
//! RustFS are responsible for skipping/failing clearly if it is not
//! reachable; this module does not gate on that itself.

use diesel::r2d2::{ConnectionManager, Pool};
use diesel::{pg::PgConnection, prelude::*};
use tower_cookies::Key;
use uuid::Uuid;

use crate::config::{Config, Directories};
use crate::state::AppState;

pub fn test_app_state() -> AppState {
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set to run spec-002 integration tests");
    let manager = ConnectionManager::<PgConnection>::new(database_url);
    let db_pool = Pool::builder()
        .build(manager)
        .expect("failed to build test DB pool");

    let (world_event_sender, _) = tokio::sync::broadcast::channel(16);
    let (presence_sender, _) = tokio::sync::broadcast::channel(16);

    AppState {
        config: Config::from_env(),
        directories: Directories::from(std::env::temp_dir().to_str().unwrap().to_string()),
        world_event_sender,
        presence_sender,
        key: Key::generate(),
        db_pool,
        system_hooks: std::sync::Arc::new(tokio::sync::RwLock::new(
            crate::system_hooks::SystemHookRegistry::new(),
        )),
        adjudicator: std::sync::Arc::new(thunderforge_crucible::local::LocalAdjudicator),
    }
}

/// Inserts a throwaway user with a random username/email, returns its id.
pub fn insert_test_user(conn: &mut PgConnection) -> Uuid {
    use crate::schema::users;
    let id = Uuid::now_v7();
    let suffix = id.simple().to_string();
    diesel::insert_into(users::table)
        .values((
            users::id.eq(id),
            users::username.eq(format!("test_user_{suffix}")),
            users::password_hash.eq("not-a-real-hash"),
            users::email.eq(format!("test_{suffix}@example.invalid")),
        ))
        .execute(conn)
        .expect("failed to insert test user");
    id
}

/// Inserts a world owned (via `created_by`) by `owner_id`. Deliberately
/// does NOT insert a `world_members` row — `create_world` doesn't either
/// (see `auth::world_membership::require_world_member`'s doc comment) —
/// so this fixture matches real production behavior.
pub fn insert_test_world(conn: &mut PgConnection, owner_id: Uuid) -> Uuid {
    use crate::schema::worlds;
    let id = Uuid::now_v7();
    let now = chrono::Utc::now().naive_utc();
    diesel::insert_into(worlds::table)
        .values((
            worlds::id.eq(id),
            worlds::name.eq(format!("Test World {}", id.simple())),
            worlds::created_at.eq(now),
            worlds::updated_at.eq(now),
            worlds::created_by.eq(owner_id),
            worlds::updated_by.eq(owner_id),
        ))
        .execute(conn)
        .expect("failed to insert test world");
    id
}

pub fn insert_test_scene(conn: &mut PgConnection, world_id: Uuid, owner_id: Uuid) -> Uuid {
    use crate::schema::scenes;
    let id = Uuid::now_v7();
    diesel::insert_into(scenes::table)
        .values((
            scenes::scene_id.eq(id),
            scenes::world_id.eq(world_id),
            scenes::name.eq("Test Scene"),
            scenes::type_.eq("battlemap"),
            scenes::grid_size.eq(5),
            scenes::grid_type.eq("square"),
            scenes::width.eq(100),
            scenes::height.eq(100),
            scenes::owner_id.eq(owner_id),
        ))
        .execute(conn)
        .expect("failed to insert test scene");
    id
}

/// Inserts an accepted `world_members` row (role `"Player"` unless
/// overridden), simulating a completed invite-accept flow.
pub fn insert_test_world_member(conn: &mut PgConnection, world_id: Uuid, user_id: Uuid, role: &str) {
    use crate::schema::world_members;
    let now = chrono::Utc::now().naive_utc();
    diesel::insert_into(world_members::table)
        .values((
            world_members::id.eq(Uuid::now_v7()),
            world_members::world_id.eq(world_id),
            world_members::user_id.eq(user_id),
            world_members::role.eq(role),
            world_members::joined_at.eq(now),
            world_members::created_at.eq(now),
            world_members::updated_at.eq(now),
        ))
        .execute(conn)
        .expect("failed to insert test world member");
}

pub fn remove_test_world_member(conn: &mut PgConnection, world_id: Uuid, user_id: Uuid) {
    use crate::schema::world_members;
    diesel::delete(
        world_members::table
            .filter(world_members::world_id.eq(world_id))
            .filter(world_members::user_id.eq(user_id)),
    )
    .execute(conn)
    .expect("failed to remove test world member");
}

/// A tiny valid PNG (1x1 red pixel), for tests that need real
/// non-WebP image bytes to upload/transcode.
pub fn tiny_png_bytes() -> Vec<u8> {
    let img = image::RgbImage::from_pixel(1, 1, image::Rgb([255, 0, 0]));
    let mut bytes = Vec::new();
    image::DynamicImage::ImageRgb8(img)
        .write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png)
        .unwrap();
    bytes
}

/// Spec 025: inserts a minimal, visible (non-GM-only) `world_abilities` row.
/// Callers that need a hidden ability flip `gm_only` themselves, so the
/// default here matches the DB default and the common case.
pub fn insert_test_ability(conn: &mut PgConnection, world_id: Uuid, created_by: Uuid) -> Uuid {
    use crate::schema::world_abilities;
    diesel::insert_into(world_abilities::table)
        .values((
            world_abilities::world_id.eq(world_id),
            world_abilities::name.eq("Test Ability"),
            world_abilities::classification.eq("spell"),
            world_abilities::created_by.eq(created_by),
            world_abilities::updated_by.eq(created_by),
        ))
        .returning(world_abilities::id)
        .get_result::<Uuid>(conn)
        .expect("failed to insert test ability")
}
