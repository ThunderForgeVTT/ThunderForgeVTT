//! Spec 051 T051: `liftWorldPlayPause`, through the schema.
//!
//! Held under `play_pause::test_lock` for the whole test: `migration_tests`
//! cycles `down.sql`, and a lift racing a dropped table reads as a flake.

use async_graphql::Request;
use diesel::prelude::*;
use serde_json::Value;
use uuid::Uuid;

use crate::auth_middleware::AuthenticatedUser;
use crate::graphql::mutations_moderation::{
    SubmitTakedownNoticeInput, submit_takedown_notice_impl,
};
use crate::graphql::types::ModerationEntityType;
use crate::moderation::effective_status_sync;
use crate::play_pause::{TriggerDetail, pause_world};
use crate::schema::world_members;
use crate::test_support::{
    insert_test_scene, insert_test_user, insert_test_world, insert_test_world_member,
    test_app_state,
};

fn schema(state: crate::state::AppState) -> crate::graphql::AppSchema {
    async_graphql::Schema::build(
        crate::graphql::QueryRoot::default(),
        crate::graphql::MutationRoot::default(),
        crate::graphql::SubscriptionRoot,
    )
    .data(state)
    .finish()
}

fn caller(user_id: Uuid, is_admin: bool) -> AuthenticatedUser {
    AuthenticatedUser {
        user_id,
        session_id: Uuid::now_v7(),
        expires_at: chrono::Utc::now().naive_utc() + chrono::Duration::hours(1),
        is_admin,
        role: if is_admin { "Admin" } else { "User" }.to_string(),
        disabled: false,
    }
}

fn lift(pause: Uuid, grounds: &str) -> String {
    format!(
        r#"mutation {{ liftWorldPlayPause(pauseId: "{pause}", grounds: "{grounds}") {{
            id worldId liftedBy {{ id name }} liftedAt liftGrounds
        }} }}"#
    )
}

fn error_of(response: &async_graphql::Response) -> Value {
    serde_json::to_value(response).unwrap()["errors"][0].clone()
}

/// A world with an Owner, a GM and a Player, an operator, and a pause in
/// force.
struct Paused {
    state: crate::state::AppState,
    operator: Uuid,
    owner: Uuid,
    gm: Uuid,
    player: Uuid,
    world: Uuid,
    pause: Uuid,
}

fn a_paused_world() -> Paused {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let operator = insert_test_user(&mut conn);
    let owner = insert_test_user(&mut conn);
    let gm = insert_test_user(&mut conn);
    let player = insert_test_user(&mut conn);
    let world = insert_test_world(&mut conn, owner);
    insert_test_world_member(&mut conn, world, gm, "GM");
    insert_test_world_member(&mut conn, world, player, "Player");
    let pause = pause_world(
        &mut conn,
        operator,
        world,
        "Abuse at this table.",
        TriggerDetail::operator(),
    )
    .unwrap()
    .pause
    .id;
    drop(conn);
    Paused {
        state,
        operator,
        owner,
        gm,
        player,
        world,
        pause,
    }
}

/// FR-040, FR-041 and the contract's refusal table: blank grounds and an
/// unknown pause are refused with their codes; an operator lifts; a second
/// lift is told who lifted it and when.
#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn an_operator_lifts_and_a_second_lift_is_told_who_and_when() {
    let _lock = crate::play_pause::test_lock();
    let p = a_paused_world();
    let second = {
        let mut conn = p.state.db_pool.get().unwrap();
        insert_test_user(&mut conn)
    };
    let schema = schema(p.state.clone());

    let blank = schema
        .execute(Request::new(lift(p.pause, "  ")).data(caller(p.operator, true)))
        .await;
    assert_eq!(error_of(&blank)["extensions"]["code"], "GROUNDS_REQUIRED");

    let missing = schema
        .execute(Request::new(lift(Uuid::now_v7(), "Grounds.")).data(caller(p.operator, true)))
        .await;
    assert_eq!(error_of(&missing)["extensions"]["code"], "PAUSE_NOT_FOUND");

    let lifted = schema
        .execute(Request::new(lift(p.pause, "Reviewed; fine.")).data(caller(p.operator, true)))
        .await;
    assert!(lifted.errors.is_empty(), "{:?}", lifted.errors);
    let lifted = lifted.data.into_json().unwrap()["liftWorldPlayPause"].clone();
    assert_eq!(lifted["id"], p.pause.to_string());
    assert_eq!(lifted["worldId"], p.world.to_string());
    assert_eq!(lifted["liftedBy"]["id"], p.operator.to_string());
    assert_eq!(lifted["liftGrounds"], "Reviewed; fine.");
    assert!(lifted["liftedAt"].is_string());

    let late = schema
        .execute(Request::new(lift(p.pause, "Also fine.")).data(caller(second, true)))
        .await;
    let late = error_of(&late);
    assert_eq!(late["extensions"]["code"], "PAUSE_ALREADY_LIFTED");
    assert_eq!(late["extensions"]["liftedBy"]["id"], p.operator.to_string());
    assert_eq!(
        late["extensions"]["liftedBy"]["name"],
        lifted["liftedBy"]["name"]
    );
    let told =
        chrono::DateTime::parse_from_rfc3339(late["extensions"]["liftedAt"].as_str().unwrap())
            .unwrap();
    let returned =
        chrono::DateTime::parse_from_rfc3339(lifted["liftedAt"].as_str().unwrap()).unwrap();
    assert_eq!(told, returned, "the first lift's time, not the second's");
}

/// FR-040: a world's Owner and its GM are not operators, and neither lifts.
#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn a_worlds_owner_and_gm_cannot_lift_its_pause() {
    let _lock = crate::play_pause::test_lock();
    let p = a_paused_world();
    let schema = schema(p.state.clone());

    for who in [p.owner, p.gm] {
        let refused = schema
            .execute(Request::new(lift(p.pause, "Ours to resume.")).data(caller(who, false)))
            .await;
        assert!(!refused.errors.is_empty(), "a non-operator lifted");
        assert!(refused.data.into_json().unwrap().is_null());
    }

    let state = schema
        .execute(
            Request::new(format!(
                r#"{{ worldPlayState(worldId: "{}") {{ paused }} }}"#,
                p.world
            ))
            .data(caller(p.owner, false)),
        )
        .await;
    assert!(state.errors.is_empty(), "{:?}", state.errors);
    assert_eq!(
        state.data.into_json().unwrap()["worldPlayState"]["paused"],
        true
    );
}

/// FR-041, FR-042: after a lift, gated calls answer again and the world reads
/// as not paused, with the span's `liftedAt` set. A scene taken down while the
/// world was paused stays withheld, and membership is untouched.
#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn a_lift_restores_play_and_nothing_else() {
    let _lock = crate::play_pause::test_lock();
    let p = a_paused_world();
    let scene = {
        let mut conn = p.state.db_pool.get().unwrap();
        insert_test_scene(&mut conn, p.world, p.owner)
    };
    let schema = schema(p.state.clone());
    let heartbeat = || {
        Request::new(format!(
            r#"mutation {{ heartbeat(worldId: "{}") }}"#,
            p.world
        ))
        .data(caller(p.player, false))
    };
    let play_state = || {
        Request::new(format!(
            r#"{{ worldPlayState(worldId: "{}") {{ paused pausedAt history {{ pausedAt liftedAt }} }} }}"#,
            p.world
        ))
        .data(caller(p.player, false))
    };

    let refused = schema.execute(heartbeat()).await;
    assert_eq!(
        error_of(&refused)["extensions"]["code"],
        "WORLD_PLAY_PAUSED",
        "the gate holds before the lift"
    );

    submit_takedown_notice_impl(
        &p.state,
        SubmitTakedownNoticeInput {
            entity_type: ModerationEntityType::Scene,
            entity_id: scene,
            claimant_name: "Lift Test Claimant".into(),
            claimant_contact: "claimant@example.test".into(),
            copyrighted_work_description: "A published battle map".into(),
            infringing_material_location: "A scene in a paused world".into(),
            good_faith_statement: true,
            accuracy_statement: true,
            signature: "Lift Test Claimant".into(),
        },
    )
    .await
    .expect("a notice can be filed against a scene in a paused world");

    let members = |state: &crate::state::AppState| -> Vec<(Uuid, String)> {
        let mut conn = state.db_pool.get().unwrap();
        world_members::table
            .filter(world_members::world_id.eq(p.world))
            .order(world_members::user_id)
            .select((world_members::user_id, world_members::role))
            .load(&mut conn)
            .unwrap()
    };
    let members_before = members(&p.state);
    let withheld_before = {
        let mut conn = p.state.db_pool.get().unwrap();
        effective_status_sync(&mut conn, "scene", scene).unwrap()
    };
    assert!(withheld_before.is_some(), "the scene is taken down");

    let lifted = schema
        .execute(Request::new(lift(p.pause, "Reviewed.")).data(caller(p.operator, true)))
        .await;
    assert!(lifted.errors.is_empty(), "{:?}", lifted.errors);
    let lifted_at = lifted.data.into_json().unwrap()["liftWorldPlayPause"]["liftedAt"].clone();

    let beat = schema.execute(heartbeat()).await;
    assert!(
        beat.errors.is_empty(),
        "play starts again: {:?}",
        beat.errors
    );

    let read = schema.execute(play_state()).await;
    assert!(read.errors.is_empty(), "{:?}", read.errors);
    let read = read.data.into_json().unwrap()["worldPlayState"].clone();
    assert_eq!(read["paused"], false);
    assert!(read["pausedAt"].is_null());
    assert_eq!(read["history"].as_array().unwrap().len(), 1);
    assert_eq!(read["history"][0]["liftedAt"], lifted_at);

    let mut conn = p.state.db_pool.get().unwrap();
    assert_eq!(
        effective_status_sync(&mut conn, "scene", scene).unwrap(),
        withheld_before,
        "the takedown is not lifted with the pause"
    );
    drop(conn);
    assert_eq!(members(&p.state), members_before, "membership untouched");
}
