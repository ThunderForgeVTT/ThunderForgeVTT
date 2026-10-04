//! Spec 051 T009: the gate refuses a paused world and nothing else, and is
//! blind to who is asking.

use diesel::prelude::*;
use uuid::Uuid;

use super::gate::{
    GateError, PlayPaused, WORLD_PLAY_PAUSED, refuse_if_paused, refuse_scene_if_paused,
};
use super::{TriggerDetail, lift_pause, pause_world};
use crate::auth::world_membership::{actor_in_world, is_dm_of_scene};
use crate::schema::users;
use crate::test_support::{
    insert_test_scene, insert_test_user, insert_test_world, insert_test_world_member,
    test_app_state,
};

const GROUNDS: &str = "Harassment reported at this table; stopping play while it is looked at.";

fn an_operator(conn: &mut PgConnection) -> Uuid {
    let id = insert_test_user(conn);
    diesel::update(users::table.filter(users::id.eq(id)))
        .set(users::is_admin.eq(true))
        .execute(conn)
        .unwrap();
    id
}

#[test]
fn it_refuses_an_active_pause_and_passes_a_lifted_one_or_none() {
    let _lock = super::test_lock();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
        let operator = an_operator(conn);
        let world = insert_test_world(conn, operator);
        let neighbour = insert_test_world(conn, operator);

        assert_eq!(refuse_if_paused(conn, world), Ok(()), "never paused");

        let outcome =
            pause_world(conn, operator, world, GROUNDS, TriggerDetail::operator()).expect("paused");

        assert_eq!(
            refuse_if_paused(conn, world),
            Err(GateError::Paused(PlayPaused {
                world_id: world,
                paused_at: outcome.pause.paused_at,
            }))
        );
        assert_eq!(
            refuse_if_paused(conn, neighbour),
            Ok(()),
            "another world is untouched"
        );

        lift_pause(
            conn,
            operator,
            outcome.pause.id,
            "Looked at; nothing further.",
        )
        .expect("lifted");
        assert_eq!(refuse_if_paused(conn, world), Ok(()), "lifted");
        Ok(())
    });
}

#[test]
fn the_scene_variant_resolves_the_scenes_world() {
    let _lock = super::test_lock();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
        let operator = an_operator(conn);
        let world = insert_test_world(conn, operator);
        let scene = insert_test_scene(conn, world, operator);
        let neighbour = insert_test_world(conn, operator);
        let neighbour_scene = insert_test_scene(conn, neighbour, operator);

        assert_eq!(refuse_scene_if_paused(conn, scene), Ok(()));

        pause_world(conn, operator, world, GROUNDS, TriggerDetail::operator()).unwrap();

        assert!(matches!(
            refuse_scene_if_paused(conn, scene),
            Err(GateError::Paused(PlayPaused { world_id, .. })) if world_id == world
        ));
        assert_eq!(refuse_scene_if_paused(conn, neighbour_scene), Ok(()));
        assert_eq!(
            refuse_scene_if_paused(conn, Uuid::now_v7()),
            Ok(()),
            "no such scene is the resolver's to say, not the gate's"
        );
        Ok(())
    });
}

/// ADR-100 decision 2. The site-admin short-circuit lets this operator
/// through every role check in the world — the assertions below show that it
/// does — and the gate refuses them anyway.
#[test]
fn it_refuses_a_site_admin_who_is_a_member() {
    let _lock = super::test_lock();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
        let operator = an_operator(conn);
        let owner = insert_test_user(conn);
        let world = insert_test_world(conn, owner);
        let scene = insert_test_scene(conn, world, owner);
        insert_test_world_member(conn, world, operator, "Player");

        pause_world(conn, operator, world, GROUNDS, TriggerDetail::operator()).unwrap();

        assert!(actor_in_world(conn, operator, true, world).is_site_admin);
        assert!(is_dm_of_scene(conn, operator, true, scene).unwrap());

        assert!(matches!(
            refuse_if_paused(conn, world),
            Err(GateError::Paused(_))
        ));
        assert!(matches!(
            refuse_scene_if_paused(conn, scene),
            Err(GateError::Paused(_))
        ));
        Ok(())
    });
}

/// FR-011: the refusal a member sees says *that and when*, and not why or who.
#[test]
fn the_error_carries_no_grounds() {
    let _lock = super::test_lock();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
        let operator = an_operator(conn);
        let world = insert_test_world(conn, operator);
        let outcome =
            pause_world(conn, operator, world, GROUNDS, TriggerDetail::operator()).unwrap();

        let error: async_graphql::Error = refuse_if_paused(conn, world).unwrap_err().into();
        assert_eq!(
            error.message,
            "Play in this world has been paused by an operator."
        );

        let extensions = serde_json::to_value(error.extensions.as_ref().expect("extensions"))
            .expect("extensions serialise");
        assert_eq!(extensions["code"], WORLD_PLAY_PAUSED);
        assert_eq!(extensions["worldId"], world.to_string());
        assert_eq!(
            extensions["pausedAt"],
            outcome.pause.paused_at.and_utc().to_rfc3339()
        );
        let keys: Vec<&String> = extensions.as_object().unwrap().keys().collect();
        assert_eq!(
            keys.len(),
            3,
            "nothing but code, worldId and pausedAt: {keys:?}"
        );

        let whole = format!("{} {extensions}", error.message);
        let operator_name = outcome.pause.paused_by_name;
        for secret in [GROUNDS, "Harassment", operator_name.as_str()] {
            assert!(
                !whole.contains(secret),
                "the refusal leaks {secret:?}: {whole}"
            );
        }
        Ok(())
    });
}

/// A refusal that crossed a diesel-typed closure keeps its code.
#[test]
fn a_refusal_carried_through_diesel_keeps_its_code() {
    let paused = PlayPaused {
        world_id: Uuid::now_v7(),
        paused_at: chrono::Utc::now().naive_utc(),
    };
    let through: diesel::result::Error = GateError::Paused(paused).into();
    assert_eq!(
        super::gate::carried(&through),
        Some(GateError::Paused(paused))
    );

    let error = super::gate::refusal_or(through, "Failed to delete wall");
    let extensions = serde_json::to_value(error.extensions.as_ref().expect("extensions"))
        .expect("extensions serialise");
    assert_eq!(extensions["code"], WORLD_PLAY_PAUSED);

    let plain = super::gate::refusal_or(diesel::result::Error::NotFound, "Failed to delete wall");
    assert_eq!(plain.message, "Failed to delete wall");
    assert!(plain.extensions.is_none());
}
