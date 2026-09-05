use super::*;
use diesel::PgConnection;

/// Every kind the client may send is accepted and stored verbatim.
#[test]
fn every_known_token_kind_is_accepted() {
    for kind in TokenKind::ALL {
        let parsed = parse_token_kind(Some(kind.as_stored()))
            .unwrap_or_else(|e| panic!("{kind:?} should parse: {e:?}"));
        assert_eq!(parsed, kind);
    }
}

/// Omitting the field is the column default, not an error.
#[test]
fn an_absent_kind_is_the_default_rather_than_a_refusal() {
    assert_eq!(parse_token_kind(None).unwrap(), TokenKind::Character);
    assert_eq!(TokenKind::Character.as_stored(), "character");
}

/// An unknown kind is refused rather than stored.
///
/// The alternative — falling back to a default — would put a token on the
/// board wearing the wrong meaning, and the Game Master would have no way
/// to tell. The error names the valid set so the caller can fix it.
#[test]
fn an_unknown_kind_is_refused_and_the_error_says_what_is_valid() {
    let err = parse_token_kind(Some("dragon"))
        .expect_err("an unknown kind must not be silently accepted");
    let message = err.message;
    assert!(
        message.contains("dragon"),
        "should name the bad value: {message}"
    );
    for kind in TokenKind::ALL {
        assert!(
            message.contains(kind.as_stored()),
            "should list {}: {message}",
            kind.as_stored()
        );
    }
}

/// Casing is not forgiven, deliberately.
///
/// These are stored values, not user input — the client sends what the
/// schema says. Accepting "NPC" here would mean two spellings reaching
/// the column and the renderer having to know about both.
#[test]
fn kind_matching_is_exact() {
    for wrong in ["NPC", "Character", "OBJECT", " npc"] {
        assert!(
            parse_token_kind(Some(wrong)).is_err(),
            "{wrong:?} must be refused"
        );
    }
}

/// Establishes a connection to the dev database configured via
/// DATABASE_URL (same source main.rs uses). Skips (rather than fails)
/// when no dev database is reachable, since this is a real-DB
/// integration test, not a unit test.
fn try_connect() -> Option<PgConnection> {
    dotenvy::dotenv().ok();
    let url = std::env::var("DATABASE_URL").ok()?;
    PgConnection::establish(&url).ok()
}

/// The rule every token mutation (`create_token`/`update_token`/`delete_token`) now asks, in the one place they all ask
/// it: authority to author content on a scene is the caller's **world
/// role** — Owner or GM — not who happened to create the scene.
///
/// This replaces `token_mutations_are_scoped_to_scene_owner`, which asserted the old rule faithfully.
/// That rule was the bug: two people both holding GM authority in one
/// world, writing to one scene, had exactly half the writes refused,
/// because whichever of them had not created the scene was refused every
/// time. Both directions of that break are asserted below, along with the
/// two answers that must stay refusals — a GM's new authority must not
/// leak down to Players or out to non-members.
///
/// `move_own_token` is deliberately untouched by this and keeps its own,
/// stricter rule — see `move_own_token_filter_rejects_non_owner` below.
#[test]
fn token_authority_follows_the_world_role_not_the_scene_creator() {
    let Some(mut conn) = try_connect() else {
        eprintln!(
            "skipping token_authority_follows_the_world_role_not_the_scene_creator: no DATABASE_URL/dev DB reachable"
        );
        return;
    };

    conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
        use crate::auth::world_membership::is_dm_of_scene;
        use crate::test_support::{
            insert_test_scene_named, insert_test_user, insert_test_world, insert_test_world_member,
        };

        let owner_id = insert_test_user(conn);
        let world_id = insert_test_world(conn, owner_id);

        let gm_id = insert_test_user(conn);
        insert_test_world_member(conn, world_id, gm_id, "GM");
        let player_id = insert_test_user(conn);
        insert_test_world_member(conn, world_id, player_id, "Player");
        let stranger_id = insert_test_user(conn);

        // Two scenes in the same world, created by two different people.
        // Under the old rule each of them was an island.
        let owners_scene = insert_test_scene_named(conn, world_id, owner_id, "Owner's Scene");
        let gms_scene = insert_test_scene_named(conn, world_id, gm_id, "GM's Scene");

        assert!(
            is_dm_of_scene(conn, gm_id, false, owners_scene)?,
            "a member promoted to GM must be able to edit tokens on a scene the Owner created"
        );
        assert!(
            is_dm_of_scene(conn, owner_id, false, gms_scene)?,
            "the world's Owner must be able to edit tokens on a scene a GM created"
        );
        assert!(
            !is_dm_of_scene(conn, player_id, false, owners_scene)?,
            "a plain Player must not gain content authority from world membership"
        );
        assert!(
            !is_dm_of_scene(conn, stranger_id, false, owners_scene)?,
            "a non-member must not be able to edit tokens in this world at all"
        );

        Ok(())
    });
}

/// Spec 004 T026: a non-owning player's `move_own_token`-shaped filter
/// (owner_user_id = requester) must not match a token owned by someone
/// else, and the token's position must be unchanged afterward.
#[test]
fn move_own_token_filter_rejects_non_owner() {
    let Some(mut conn) = try_connect() else {
        eprintln!(
            "skipping move_own_token_filter_rejects_non_owner: no DATABASE_URL/dev DB reachable"
        );
        return;
    };

    conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
        use crate::schema::{scenes, tokens, users, worlds};

        let scene_owner_id = uuid::Uuid::now_v7();
        let controller_id = uuid::Uuid::now_v7();
        let intruder_id = uuid::Uuid::now_v7();
        let world_id = uuid::Uuid::now_v7();
        let scene_id = uuid::Uuid::now_v7();
        let token_id = uuid::Uuid::now_v7();
        let now = chrono::Utc::now().naive_utc();

        for (id, username) in [
            (scene_owner_id, "move-own-scene-owner"),
            (controller_id, "move-own-controller"),
            (intruder_id, "move-own-intruder"),
        ] {
            diesel::insert_into(users::table)
                .values((
                    users::id.eq(id),
                    users::username.eq(format!("{username}-{id}")),
                    users::password_hash.eq("test-hash"),
                    users::email.eq(format!("{username}-{id}@example.test")),
                    users::created_at.eq(now),
                    users::updated_at.eq(now),
                ))
                .execute(conn)?;
        }

        diesel::insert_into(worlds::table)
            .values((
                worlds::id.eq(world_id),
                worlds::name.eq("Move Own Token Test World"),
                worlds::created_by.eq(scene_owner_id),
                worlds::updated_by.eq(scene_owner_id),
                worlds::created_at.eq(now),
                worlds::updated_at.eq(now),
            ))
            .execute(conn)?;

        diesel::insert_into(scenes::table)
            .values((
                scenes::scene_id.eq(scene_id),
                scenes::world_id.eq(world_id),
                scenes::name.eq("Move Own Token Test Scene"),
                scenes::type_.eq("battlemap"),
                scenes::grid_size.eq(32),
                scenes::grid_type.eq("square"),
                scenes::width.eq(1000),
                scenes::height.eq(1000),
                scenes::owner_id.eq(scene_owner_id),
                scenes::created_at.eq(now),
                scenes::updated_at.eq(now),
            ))
            .execute(conn)?;

        diesel::insert_into(tokens::table)
            .values((
                tokens::token_id.eq(token_id),
                tokens::scene_id.eq(scene_id),
                tokens::x.eq(5.0),
                tokens::y.eq(5.0),
                tokens::rotation.eq(0.0),
                tokens::scale.eq(1.0),
                tokens::owner_user_id.eq(controller_id),
                tokens::created_at.eq(now),
                tokens::updated_at.eq(now),
            ))
            .execute(conn)?;

        // The intruder's move_own_token-shaped filter must match zero rows.
        let intruder_move_count = diesel::update(
            tokens::table
                .filter(tokens::token_id.eq(token_id))
                .filter(tokens::owner_user_id.eq(intruder_id)),
        )
        .set((tokens::x.eq(99.0), tokens::y.eq(99.0)))
        .execute(conn)?;
        assert_eq!(
            intruder_move_count, 0,
            "a non-controller's move filter must not match another player's token"
        );

        let (x, y): (f64, f64) = tokens::table
            .filter(tokens::token_id.eq(token_id))
            .select((tokens::x, tokens::y))
            .first(conn)?;
        assert_eq!(
            (x, y),
            (5.0, 5.0),
            "position must be unchanged after a rejected move"
        );

        // The real controller's filter must match exactly one row.
        let controller_move_count = diesel::update(
            tokens::table
                .filter(tokens::token_id.eq(token_id))
                .filter(tokens::owner_user_id.eq(controller_id)),
        )
        .set((tokens::x.eq(10.0), tokens::y.eq(10.0)))
        .execute(conn)?;
        assert_eq!(
            controller_move_count, 1,
            "the token's controller must be able to move it"
        );

        Ok(())
    });
}

/// Spec 004 T027: setting `is_primary = true` for a second token under
/// the same (scene_id, owner_user_id) must leave exactly one primary,
/// respecting the partial unique index `tokens_one_primary_per_owner_per_scene`.
#[test]
fn setting_second_primary_replaces_the_first() {
    let Some(mut conn) = try_connect() else {
        eprintln!(
            "skipping setting_second_primary_replaces_the_first: no DATABASE_URL/dev DB reachable"
        );
        return;
    };

    conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
        use crate::schema::{scenes, tokens, users, worlds};

        let scene_owner_id = uuid::Uuid::now_v7();
        let player_id = uuid::Uuid::now_v7();
        let world_id = uuid::Uuid::now_v7();
        let scene_id = uuid::Uuid::now_v7();
        let token_a_id = uuid::Uuid::now_v7();
        let token_b_id = uuid::Uuid::now_v7();
        let now = chrono::Utc::now().naive_utc();

        for (id, username) in [
            (scene_owner_id, "primary-test-scene-owner"),
            (player_id, "primary-test-player"),
        ] {
            diesel::insert_into(users::table)
                .values((
                    users::id.eq(id),
                    users::username.eq(format!("{username}-{id}")),
                    users::password_hash.eq("test-hash"),
                    users::email.eq(format!("{username}-{id}@example.test")),
                    users::created_at.eq(now),
                    users::updated_at.eq(now),
                ))
                .execute(conn)?;
        }

        diesel::insert_into(worlds::table)
            .values((
                worlds::id.eq(world_id),
                worlds::name.eq("Primary Token Test World"),
                worlds::created_by.eq(scene_owner_id),
                worlds::updated_by.eq(scene_owner_id),
                worlds::created_at.eq(now),
                worlds::updated_at.eq(now),
            ))
            .execute(conn)?;

        diesel::insert_into(scenes::table)
            .values((
                scenes::scene_id.eq(scene_id),
                scenes::world_id.eq(world_id),
                scenes::name.eq("Primary Token Test Scene"),
                scenes::type_.eq("battlemap"),
                scenes::grid_size.eq(32),
                scenes::grid_type.eq("square"),
                scenes::width.eq(1000),
                scenes::height.eq(1000),
                scenes::owner_id.eq(scene_owner_id),
                scenes::created_at.eq(now),
                scenes::updated_at.eq(now),
            ))
            .execute(conn)?;

        for token_id in [token_a_id, token_b_id] {
            diesel::insert_into(tokens::table)
                .values((
                    tokens::token_id.eq(token_id),
                    tokens::scene_id.eq(scene_id),
                    tokens::x.eq(0.0),
                    tokens::y.eq(0.0),
                    tokens::rotation.eq(0.0),
                    tokens::scale.eq(1.0),
                    tokens::owner_user_id.eq(player_id),
                    tokens::created_at.eq(now),
                    tokens::updated_at.eq(now),
                ))
                .execute(conn)?;
        }

        // Mark token A primary first.
        diesel::update(tokens::table.filter(tokens::token_id.eq(token_a_id)))
            .set(tokens::is_primary.eq(true))
            .execute(conn)?;

        // Now replicate update_token's "clear prior primary" step before
        // marking token B primary (the actual mutation does this inside
        // one DB transaction; here we exercise the same two statements).
        diesel::update(
            tokens::table
                .filter(tokens::scene_id.eq(scene_id))
                .filter(tokens::owner_user_id.eq(player_id))
                .filter(tokens::is_primary.eq(true))
                .filter(tokens::token_id.ne(token_b_id)),
        )
        .set(tokens::is_primary.eq(false))
        .execute(conn)?;

        diesel::update(tokens::table.filter(tokens::token_id.eq(token_b_id)))
            .set(tokens::is_primary.eq(true))
            .execute(conn)?;

        let primary_count: i64 = tokens::table
            .filter(tokens::scene_id.eq(scene_id))
            .filter(tokens::owner_user_id.eq(player_id))
            .filter(tokens::is_primary.eq(true))
            .count()
            .get_result(conn)?;
        assert_eq!(
            primary_count, 1,
            "exactly one primary token must remain for this owner"
        );

        let token_b_is_primary: bool = tokens::table
            .filter(tokens::token_id.eq(token_b_id))
            .select(tokens::is_primary)
            .first(conn)?;
        assert!(token_b_is_primary, "token B must be the surviving primary");

        Ok(())
    });
}

/// The three states of `TokenUpdate::photo_url`, against a real
/// database, because they are a property of Diesel's `AsChangeset`
/// rather than of any code here: skip, write, and write NULL.
///
/// The clearing case is the one that did not exist before — a plain
/// `Option<String>` cannot express it, so a GM could replace token art
/// but never remove it.
#[test]
fn token_photo_url_can_be_set_skipped_and_cleared() {
    let Some(mut conn) = try_connect() else {
        eprintln!(
            "skipping token_photo_url_can_be_set_skipped_and_cleared: no DATABASE_URL/dev DB reachable"
        );
        return;
    };

    conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
        use crate::schema::{scenes, tokens, users, worlds};

        let owner_id = uuid::Uuid::now_v7();
        let world_id = uuid::Uuid::now_v7();
        let scene_id = uuid::Uuid::now_v7();
        let token_id = uuid::Uuid::now_v7();
        let now = chrono::Utc::now().naive_utc();

        diesel::insert_into(users::table)
            .values((
                users::id.eq(owner_id),
                users::username.eq(format!("token-photo-owner-{owner_id}")),
                users::password_hash.eq("test-hash"),
                users::email.eq(format!("token-photo-{owner_id}@example.test")),
                users::created_at.eq(now),
                users::updated_at.eq(now),
            ))
            .execute(conn)?;

        diesel::insert_into(worlds::table)
            .values((
                worlds::id.eq(world_id),
                worlds::name.eq("Token Photo World"),
                worlds::created_by.eq(owner_id),
                worlds::updated_by.eq(owner_id),
                worlds::created_at.eq(now),
                worlds::updated_at.eq(now),
            ))
            .execute(conn)?;

        diesel::insert_into(scenes::table)
            .values((
                scenes::scene_id.eq(scene_id),
                scenes::world_id.eq(world_id),
                scenes::name.eq("Token Photo Scene"),
                scenes::type_.eq("battlemap"),
                scenes::grid_size.eq(32),
                scenes::grid_type.eq("square"),
                scenes::width.eq(1000),
                scenes::height.eq(1000),
                scenes::owner_id.eq(owner_id),
                scenes::created_at.eq(now),
                scenes::updated_at.eq(now),
            ))
            .execute(conn)?;

        diesel::insert_into(tokens::table)
            .values((
                tokens::token_id.eq(token_id),
                tokens::scene_id.eq(scene_id),
                tokens::x.eq(0.0),
                tokens::y.eq(0.0),
                tokens::rotation.eq(0.0),
                tokens::scale.eq(1.0),
                tokens::created_at.eq(now),
                tokens::updated_at.eq(now),
            ))
            .execute(conn)?;

        let photo_of = |conn: &mut PgConnection| -> Result<Option<String>, diesel::result::Error> {
            tokens::table
                .filter(tokens::token_id.eq(token_id))
                .select(tokens::photo_url)
                .first(conn)
        };

        // Always carries an `x`, both because that is the shape of a
        // real update (the client sends position with every change)
        // and because Diesel rejects a wholly empty changeset at
        // runtime with `EmptyChangeset`.
        let update = |conn: &mut PgConnection, x: f64, photo_url| {
            diesel::update(tokens::table.filter(tokens::token_id.eq(token_id)))
                .set(crate::models::TokenUpdate {
                    actor_id: None,
                    x: Some(x),
                    y: None,
                    rotation: None,
                    scale: None,
                    metadata: None,
                    owner_user_id: None,
                    is_primary: None,
                    photo_url,
                    health: None,
                    max_health: None,
                    token_type: None,
                })
                .execute(conn)
        };

        // Write.
        update(
            conn,
            1.0,
            Some(Some("/api/canvas-assets/abc.webp".to_string())),
        )?;
        assert_eq!(
            photo_of(conn)?,
            Some("/api/canvas-assets/abc.webp".to_string())
        );

        // Skip: a plain move must not disturb the art.
        update(conn, 2.0, None)?;
        assert_eq!(
            photo_of(conn)?,
            Some("/api/canvas-assets/abc.webp".to_string()),
            "an omitted photo_url must leave the column untouched"
        );

        // Clear.
        update(conn, 3.0, Some(None))?;
        assert_eq!(
            photo_of(conn)?,
            None,
            "an explicit null photo_url must clear the column"
        );

        Ok(())
    });
}
