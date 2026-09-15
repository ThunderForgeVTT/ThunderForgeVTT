//! Spec 002: shared test fixtures for integration tests that need a real
//! `AppState` (DB pool) — and, for storage tests, a real RustFS instance.
//!
//! # The test database
//!
//! Tests do **not** use the development database. They connect to their own,
//! `thunderforge_test` by default, which [`test_database_url`] creates and
//! migrates the first time a test binary asks for it:
//!
//! - `TEST_DATABASE_URL`, when set, names it outright;
//! - otherwise it is `DATABASE_URL` (from the environment or `.env`) with the
//!   database name replaced by `thunderforge_test`;
//! - otherwise `postgres://postgres:password@localhost:5432/thunderforge_test`.
//!
//! It refuses to run against the development database's name, or against an
//! e2e shard's. Sharing the development database is what let `cargo test`
//! and an e2e run fight over the same global settings rows, and what left
//! a quarter of a million test users in it.
//!
//! `make test-db-reset` drops it; the next test recreates it.
//!
//! Tests that need RustFS (`docker compose up -d rustfs`) are responsible for
//! skipping/failing clearly if it is not reachable; this module does not gate
//! on that itself.

use std::sync::{Once, OnceLock};

use diesel::r2d2::{ConnectionManager, Pool};
use diesel::{pg::PgConnection, prelude::*};
use tower_cookies::Key;
use uuid::Uuid;

use crate::config::{Config, Directories};
use crate::state::AppState;

/// The database tests use when nothing names another.
pub const TEST_DATABASE_NAME: &str = "thunderforge_test";

/// Loads `.env` into the process environment, **once** per test binary.
///
/// Every test that wants `.env` must come through here rather than calling
/// `dotenvy::dotenv()` itself. `dotenv()` sets every variable in the file that
/// is not set *at that moment* — so a test that had unset one (through
/// `settings::test_env::temp_env`) could find it put back halfway through by a
/// neighbour's `dotenv()` on another thread. That is exactly how
/// `settings::resolver`'s `a_row_beats_the_default_and_no_row_falls_back_to_it`
/// flaked: `.env` carries `SYNC_GITHUB_APP_SLUG`, and `github_app.sync.slug`
/// resolved from the environment in the middle of the test that had removed
/// it. After the first call this does nothing, and `test_env::lock()` calls it
/// before it hands out the lock, so the file is read before any test changes
/// the environment rather than during.
pub fn load_dotenv() {
    static LOADED: Once = Once::new();
    LOADED.call_once(|| {
        dotenvy::dotenv().ok();
    });
}

/// Why no test database could be had.
#[derive(Debug, Clone)]
pub enum TestDatabaseError {
    /// The URL names a database tests must never touch. Always a panic, even
    /// for the tests that otherwise skip when no database is reachable.
    Refused(String),
    /// Postgres could not be reached, or the database not created or migrated.
    Unavailable(String),
}

/// The database name in a Postgres URL (`postgres://u:p@host:port/name?x=y`).
pub fn database_name(url: &str) -> Option<&str> {
    let after_scheme = url.split_once("://").map_or(url, |(_, rest)| rest);
    let (_, path) = after_scheme.split_once('/')?;
    let name = path.split(['?', '#']).next().unwrap_or("");
    (!name.is_empty()).then_some(name)
}

/// `url` with its database name replaced (or added).
pub fn with_database_name(url: &str, name: &str) -> String {
    let (scheme, after_scheme) = url.split_once("://").unwrap_or(("postgres", url));
    let (authority, query) = match after_scheme.split_once('/') {
        Some((authority, path)) => (
            authority,
            path.find(['?', '#']).map_or("", |at| &path[at..]),
        ),
        None => (after_scheme, ""),
    };
    format!("{scheme}://{authority}/{name}{query}")
}

/// Which URL tests would use, from these inputs — split out so the rule can be
/// tested without touching the process environment.
pub fn choose_test_database_url(
    test_database_url: Option<&str>,
    database_url: Option<&str>,
) -> Result<String, TestDatabaseError> {
    let chosen = match (test_database_url, database_url) {
        (Some(explicit), _) if !explicit.trim().is_empty() => explicit.trim().to_string(),
        (_, Some(dev)) if !dev.trim().is_empty() => {
            with_database_name(dev.trim(), TEST_DATABASE_NAME)
        }
        _ => format!("postgres://postgres:password@localhost:5432/{TEST_DATABASE_NAME}"),
    };

    let name = database_name(&chosen).unwrap_or("");
    let dev_name = database_url.and_then(database_name);
    let refused = name.is_empty()
        || name == "thunderforge"
        || name.starts_with("thunderforge_e2e")
        || (Some(name) == dev_name && test_database_url.is_some());
    if refused {
        return Err(TestDatabaseError::Refused(format!(
            "refusing to run tests against the database `{name}`: tests get their own \
             (`{TEST_DATABASE_NAME}` by default; set TEST_DATABASE_URL to another \
             that is neither the development database nor an e2e shard's)"
        )));
    }
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(TestDatabaseError::Refused(format!(
            "the test database name `{name}` must be letters, digits and underscores"
        )));
    }
    Ok(chosen)
}

/// Create the database if it is missing and run any pending migrations, under
/// an advisory lock so two test binaries starting at once do not both try.
fn ensure_test_database(url: &str) -> Result<(), TestDatabaseError> {
    use diesel_migrations::{FileBasedMigrations, MigrationHarness};

    let unavailable = |what: &str, e: &dyn std::fmt::Display| {
        TestDatabaseError::Unavailable(format!("{what}: {e}"))
    };
    let name = database_name(url).unwrap_or(TEST_DATABASE_NAME).to_string();
    let maintenance_url = with_database_name(url, "postgres");
    let mut maintenance = PgConnection::establish(&maintenance_url)
        .map_err(|e| unavailable("could not reach Postgres to prepare the test database", &e))?;

    // Any fixed number: it only has to be the same in every test binary.
    const LOCK_KEY: i64 = 0x7466_7465_7374_6462; // "tftestdb"
    diesel::sql_query(format!("SELECT pg_advisory_lock({LOCK_KEY})"))
        .execute(&mut maintenance)
        .map_err(|e| unavailable("could not take the test database lock", &e))?;

    let result = (|| {
        #[derive(QueryableByName)]
        struct Exists {
            #[diesel(sql_type = diesel::sql_types::Bool)]
            exists: bool,
        }
        let exists = diesel::sql_query(
            "SELECT EXISTS (SELECT 1 FROM pg_database WHERE datname = $1) AS exists",
        )
        .bind::<diesel::sql_types::Text, _>(&name)
        .get_result::<Exists>(&mut maintenance)
        .map_err(|e| unavailable("could not look for the test database", &e))?
        .exists;
        if !exists {
            // The name was checked to be `[A-Za-z0-9_]+` when it was chosen.
            diesel::sql_query(format!("CREATE DATABASE \"{name}\""))
                .execute(&mut maintenance)
                .map_err(|e| unavailable("could not create the test database", &e))?;
        }

        let migrations =
            FileBasedMigrations::from_path(concat!(env!("CARGO_MANIFEST_DIR"), "/migrations"))
                .map_err(|e| unavailable("could not read src/server/migrations", &e))?;
        let mut conn = PgConnection::establish(url)
            .map_err(|e| unavailable("could not connect to the test database", &e))?;
        conn.run_pending_migrations(migrations)
            .map_err(|e| unavailable("could not migrate the test database", &e))?;
        Ok(())
    })();

    let _ = diesel::sql_query(format!("SELECT pg_advisory_unlock({LOCK_KEY})"))
        .execute(&mut maintenance);
    result
}

fn prepared_test_database() -> &'static Result<String, TestDatabaseError> {
    static PREPARED: OnceLock<Result<String, TestDatabaseError>> = OnceLock::new();
    PREPARED.get_or_init(|| {
        load_dotenv();
        let url = choose_test_database_url(
            std::env::var("TEST_DATABASE_URL").ok().as_deref(),
            std::env::var("DATABASE_URL").ok().as_deref(),
        )?;
        ensure_test_database(&url)?;
        Ok(url)
    })
}

/// The test database's URL, created and migrated if need be. Panics when there
/// is none — a database-backed test with no database has nothing to test.
pub fn test_database_url() -> String {
    match prepared_test_database() {
        Ok(url) => url.clone(),
        Err(TestDatabaseError::Refused(why) | TestDatabaseError::Unavailable(why)) => {
            panic!("{why}")
        }
    }
}

/// For the older tests that skip, rather than fail, when no database is
/// reachable: `None` then. A refused URL still panics — skipping would make
/// pointing the suite at the development database look like it worked.
pub fn try_test_database_url() -> Option<String> {
    match prepared_test_database() {
        Ok(url) => Some(url.clone()),
        Err(TestDatabaseError::Refused(why)) => panic!("{why}"),
        Err(TestDatabaseError::Unavailable(_)) => None,
    }
}

/// A connection to the test database, or `None` when none is reachable.
pub fn try_test_connection() -> Option<PgConnection> {
    PgConnection::establish(&try_test_database_url()?).ok()
}

pub fn test_app_state() -> AppState {
    // Load `.env` here, at the one place every database-backed test funnels
    // through, because otherwise whether a test passes depends on **which
    // other tests ran first**.
    //
    // `dotenvy::dotenv()` mutates the process environment, and a dozen test
    // modules call it individually. A test that does not — this function's
    // callers, until now — therefore finds `DATABASE_URL` set or unset purely
    // according to whether some unrelated test in the same binary got there
    // first. Run the whole suite and it usually passes; run one of these
    // tests alone, or after a change reorders the binary, and it fails with
    // `NotPresent`, which reads exactly like a misconfigured machine rather
    // than like the ordering bug it is. Observed both ways in one afternoon.
    //
    // Idempotent and does not override anything already exported, so a CI
    // machine setting `DATABASE_URL` directly keeps its value.
    load_dotenv();

    let manager = ConnectionManager::<PgConnection>::new(test_database_url());
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

    let (presence_sender, _) = tokio::sync::broadcast::channel(16);

    AppState {
        config: Config::from_env(),
        directories: Directories::from(std::env::temp_dir().to_str().unwrap().to_string()),
        world_events: std::sync::Arc::new(thunderforge_pg_sockets::WorldRouter::new()),
        presence_sender,
        presence: std::sync::Arc::new(thunderforge_presence::PresenceRegistry::new()),
        key: Key::generate(),
        db_pool,
        adjudicator: std::sync::Arc::new(thunderforge_crucible::local::LocalAdjudicator),
        // No override: a test that cares about mail sets one, and a test that
        // does not gets an instance with no mail configured — which is what
        // most instances are, and therefore the right default to be tested
        // against.
        mail: crate::mail::MailSeam::from_settings(),
        // No override, for the same reason: a test that cares about where
        // feedback goes sets one, and a test that does not gets an instance
        // with no destination configured — which is what most instances are.
        feedback: crate::feedback::FeedbackSeam::from_settings(),
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
    insert_test_scene_named(conn, world_id, owner_id, "Test Scene")
}

/// `insert_test_scene` with an explicit name. Scene names are unique per
/// world (`unique_scene_name_per_world`), so a test that needs two scenes in
/// one world — e.g. one created by the Owner and one by a GM, to check that
/// content authority does not follow the creator — must name them apart.
pub fn insert_test_scene_named(
    conn: &mut PgConnection,
    world_id: Uuid,
    owner_id: Uuid,
    name: &str,
) -> Uuid {
    use crate::schema::scenes;
    let id = Uuid::now_v7();
    diesel::insert_into(scenes::table)
        .values((
            scenes::scene_id.eq(id),
            scenes::world_id.eq(world_id),
            scenes::name.eq(name),
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

/// Spec 035: put the instance into a known admission policy.
///
/// Tests must set this explicitly rather than relying on the migration's seed,
/// because the seed is deliberately conditional (FR-013/FR-013a) — a shared
/// test database has users in it, so it seeds `open`, and a test that assumed
/// otherwise would pass for the wrong reason.
pub fn set_instance_access_policy(conn: &mut PgConnection, policy: &str) {
    use crate::schema::instance_access_settings as s;
    diesel::insert_into(s::table)
        .values((s::id.eq(1), s::access_policy.eq(policy)))
        .on_conflict(s::id)
        .do_update()
        .set(s::access_policy.eq(policy))
        .execute(conn)
        .expect("Failed to set the instance access policy");
}

/// Spec 035: an instance invitation, with the caller choosing the shape that
/// matters to their test.
pub fn insert_test_instance_invitation(
    conn: &mut PgConnection,
    created_by: Uuid,
    max_uses: i32,
    expires_at: Option<chrono::NaiveDateTime>,
    revoked: bool,
) -> (Uuid, String) {
    use crate::schema::instance_invitations as i;
    let id = Uuid::now_v7();
    let code = crate::graphql::share_codes::generate_link_code();
    diesel::insert_into(i::table)
        .values((
            i::id.eq(id),
            i::invite_code.eq(&code),
            i::max_uses.eq(max_uses),
            i::expires_at.eq(expires_at),
            i::revoked.eq(revoked),
            i::created_by.eq(created_by),
        ))
        .execute(conn)
        .expect("Failed to insert the test instance invitation");
    (id, code)
}

#[cfg(test)]
mod test_database_tests {
    use super::*;

    const DEV: &str = "postgres://postgres:password@localhost/thunderforge";

    #[test]
    fn with_no_override_the_development_url_is_renamed() {
        assert_eq!(
            choose_test_database_url(None, Some(DEV)).unwrap(),
            "postgres://postgres:password@localhost/thunderforge_test"
        );
        assert_eq!(
            choose_test_database_url(None, Some("postgres://u:p@db:5432/dev?sslmode=disable"))
                .unwrap(),
            "postgres://u:p@db:5432/thunderforge_test?sslmode=disable"
        );
        assert_eq!(
            choose_test_database_url(None, None).unwrap(),
            "postgres://postgres:password@localhost:5432/thunderforge_test"
        );
    }

    #[test]
    fn an_explicit_test_database_is_used_as_given() {
        let url = "postgres://postgres:password@localhost/tf_p9_scratch";
        assert_eq!(choose_test_database_url(Some(url), Some(DEV)).unwrap(), url);
    }

    #[test]
    fn the_development_and_e2e_databases_are_refused() {
        for url in [
            DEV,
            "postgres://postgres:password@localhost:5432/thunderforge",
            "postgres://postgres:password@localhost:5432/thunderforge_e2e_0",
            "postgres://postgres:password@localhost:5432/thunderforge_e2e_template",
        ] {
            assert!(
                matches!(
                    choose_test_database_url(Some(url), Some(DEV)),
                    Err(TestDatabaseError::Refused(_))
                ),
                "{url} must be refused"
            );
        }
        // Whatever the development database is called, naming it outright is
        // refused too.
        let dev = "postgres://postgres:password@localhost/my_dev";
        assert!(matches!(
            choose_test_database_url(Some(dev), Some(dev)),
            Err(TestDatabaseError::Refused(_))
        ));
    }

    #[test]
    fn a_name_that_would_need_quoting_is_refused() {
        assert!(matches!(
            choose_test_database_url(Some("postgres://h/bad\"name"), None),
            Err(TestDatabaseError::Refused(_))
        ));
    }
}
