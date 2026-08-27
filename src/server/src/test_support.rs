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
    // Every test builds its own pool, and `cargo test` defaults to one thread
    // per core. r2d2's defaults (max_size 10, and `build()` eagerly filling to
    // max_size because min_idle defaults to None) therefore try to open
    // cores x 10 connections at once — 320 on a 32-core machine, against
    // Postgres's default max_connections of 100. The result was a suite that
    // passed under `--test-threads=4` and failed ~240 tests at full
    // parallelism with "sorry, too many clients already": an environment
    // error that reads exactly like a code failure.
    //
    // `min_idle(Some(0))` makes connections lazy so a pool costs nothing until
    // used, and a small max_size caps the worst case. 4 leaves room for the
    // few tests that genuinely hold more than one connection at a time (the
    // concurrent-join test in `mutations_invites.rs` needs two).
    let db_pool = Pool::builder()
        .max_size(4)
        .min_idle(Some(0))
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
pub fn insert_test_world_member(
    conn: &mut PgConnection,
    world_id: Uuid,
    user_id: Uuid,
    role: &str,
) {
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
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
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

/// Spec 027: inserts a minimal `world_actors` row. Actors are scene-scoped,
/// so callers supply a scene from `insert_test_scene`.
pub fn insert_test_actor(
    conn: &mut PgConnection,
    world_id: Uuid,
    scene_id: Uuid,
    created_by: Uuid,
) -> Uuid {
    use crate::schema::world_actors;
    let id = Uuid::now_v7();
    let now = chrono::Utc::now().naive_utc();
    diesel::insert_into(world_actors::table)
        .values((
            world_actors::id.eq(id),
            world_actors::world_id.eq(world_id),
            world_actors::scene_id.eq(scene_id),
            world_actors::actor_type.eq("npc"),
            world_actors::game_system_id.eq("dnd5e"),
            world_actors::label.eq("Test Actor"),
            world_actors::created_by.eq(created_by),
            world_actors::owned_by.eq(created_by),
            world_actors::is_public.eq(false),
            world_actors::is_npc.eq(true),
            world_actors::created_at.eq(now),
            world_actors::updated_at.eq(now),
        ))
        .execute(conn)
        .expect("failed to insert test actor");
    id
}

/// Spec 027: inserts a minimal `world_items` row.
pub fn insert_test_item(conn: &mut PgConnection, world_id: Uuid, created_by: Uuid) -> Uuid {
    use crate::schema::world_items;
    let id = Uuid::now_v7();
    let now = chrono::Utc::now().naive_utc();
    diesel::insert_into(world_items::table)
        .values((
            world_items::id.eq(id),
            world_items::world_id.eq(world_id),
            world_items::name.eq("Test Item"),
            world_items::created_by.eq(created_by),
            world_items::created_at.eq(now),
            world_items::updated_at.eq(now),
        ))
        .execute(conn)
        .expect("failed to insert test item");
    id
}

/// Spec 027: inserts a minimal `world_lore_entries` row. `slug` is unique per
/// world, so it is derived from the generated id rather than a fixed string —
/// otherwise a second call for the same world collides.
pub fn insert_test_lore_entry(conn: &mut PgConnection, world_id: Uuid, created_by: Uuid) -> Uuid {
    use crate::schema::world_lore_entries;
    let id = Uuid::now_v7();
    let now = chrono::Utc::now().naive_utc();
    diesel::insert_into(world_lore_entries::table)
        .values((
            world_lore_entries::id.eq(id),
            world_lore_entries::world_id.eq(world_id),
            world_lore_entries::title.eq("Test Lore Entry"),
            world_lore_entries::slug.eq(format!("test-lore-{}", id.simple())),
            world_lore_entries::content.eq(""),
            world_lore_entries::created_by.eq(created_by),
            world_lore_entries::created_at.eq(now),
            world_lore_entries::updated_at.eq(now),
        ))
        .execute(conn)
        .expect("failed to insert test lore entry");
    id
}

/// Spec 027: grants `user_id` an explicit permission level on one content row
/// of each of the four permissioned types.
///
/// Exists so a test can set up "this member has a grant on everything" in one
/// call — which is what the member-removal cleanup contract
/// (`specs/027-unified-access-links/contracts/permission-resolution.md`) has
/// to hold for. Note `world_lore_permissions` names its user column
/// `world_member_user_id` rather than `user_id`; that asymmetry is real and is
/// absorbed here rather than migrated.
pub fn grant_all_content_permissions(
    conn: &mut PgConnection,
    user_id: Uuid,
    actor_id: Uuid,
    item_id: Uuid,
    lore_entry_id: Uuid,
    ability_id: Uuid,
    level: &str,
) {
    use crate::schema::{
        world_ability_permissions, world_actor_permissions, world_item_permissions,
        world_lore_permissions,
    };
    let now = chrono::Utc::now().naive_utc();

    diesel::insert_into(world_actor_permissions::table)
        .values((
            world_actor_permissions::id.eq(Uuid::now_v7()),
            world_actor_permissions::actor_id.eq(actor_id),
            world_actor_permissions::user_id.eq(user_id),
            world_actor_permissions::level.eq(level),
            world_actor_permissions::created_at.eq(now),
            world_actor_permissions::updated_at.eq(now),
        ))
        .execute(conn)
        .expect("failed to grant actor permission");

    diesel::insert_into(world_item_permissions::table)
        .values((
            world_item_permissions::id.eq(Uuid::now_v7()),
            world_item_permissions::item_id.eq(item_id),
            world_item_permissions::user_id.eq(user_id),
            world_item_permissions::level.eq(level),
            world_item_permissions::created_at.eq(now),
            world_item_permissions::updated_at.eq(now),
        ))
        .execute(conn)
        .expect("failed to grant item permission");

    diesel::insert_into(world_lore_permissions::table)
        .values((
            world_lore_permissions::id.eq(Uuid::now_v7()),
            world_lore_permissions::lore_entry_id.eq(lore_entry_id),
            world_lore_permissions::world_member_user_id.eq(user_id),
            world_lore_permissions::level.eq(level),
            world_lore_permissions::created_at.eq(now),
            world_lore_permissions::updated_at.eq(now),
        ))
        .execute(conn)
        .expect("failed to grant lore permission");

    diesel::insert_into(world_ability_permissions::table)
        .values((
            world_ability_permissions::id.eq(Uuid::now_v7()),
            world_ability_permissions::ability_id.eq(ability_id),
            world_ability_permissions::user_id.eq(user_id),
            world_ability_permissions::level.eq(level),
            world_ability_permissions::created_at.eq(now),
            world_ability_permissions::updated_at.eq(now),
        ))
        .execute(conn)
        .expect("failed to grant ability permission");
}

/// Spec 027: counts a user's explicit grants across all four permissioned
/// content types within one world. Returns
/// `(actors, items, lore_entries, abilities)`.
///
/// The member-removal contract requires every one of these to be zero after
/// removal; today the ability count survives, which is the defect US2 fixes.
pub fn count_content_permissions(
    conn: &mut PgConnection,
    world_id: Uuid,
    user_id: Uuid,
) -> (i64, i64, i64, i64) {
    use crate::schema::{
        world_abilities, world_ability_permissions, world_actor_permissions, world_actors,
        world_item_permissions, world_items, world_lore_entries, world_lore_permissions,
    };

    let actors = world_actor_permissions::table
        .filter(world_actor_permissions::user_id.eq(user_id))
        .filter(
            world_actor_permissions::actor_id.eq_any(
                world_actors::table
                    .filter(world_actors::world_id.eq(world_id))
                    .select(world_actors::id),
            ),
        )
        .count()
        .get_result::<i64>(conn)
        .expect("failed to count actor permissions");

    let items = world_item_permissions::table
        .filter(world_item_permissions::user_id.eq(user_id))
        .filter(
            world_item_permissions::item_id.eq_any(
                world_items::table
                    .filter(world_items::world_id.eq(world_id))
                    .select(world_items::id),
            ),
        )
        .count()
        .get_result::<i64>(conn)
        .expect("failed to count item permissions");

    let lore = world_lore_permissions::table
        .filter(world_lore_permissions::world_member_user_id.eq(user_id))
        .filter(
            world_lore_permissions::lore_entry_id.eq_any(
                world_lore_entries::table
                    .filter(world_lore_entries::world_id.eq(world_id))
                    .select(world_lore_entries::id),
            ),
        )
        .count()
        .get_result::<i64>(conn)
        .expect("failed to count lore permissions");

    let abilities = world_ability_permissions::table
        .filter(world_ability_permissions::user_id.eq(user_id))
        .filter(
            world_ability_permissions::ability_id.eq_any(
                world_abilities::table
                    .filter(world_abilities::world_id.eq(world_id))
                    .select(world_abilities::id),
            ),
        )
        .count()
        .get_result::<i64>(conn)
        .expect("failed to count ability permissions");

    (actors, items, lore, abilities)
}
