//! Spec 051 T055: the record of paused play — *that and when* for a member,
//! the whole of it for an operator, and none of it lost with the world or the
//! operator's account (FR-050 to FR-053). Through the real schema, because
//! who can read what is a property of the schema.

use async_graphql::Request;
use serde_json::Value;
use uuid::Uuid;

use super::models::PauseTriggerKind;
use super::requests::raise;
use super::{TriggerDetail, lift_pause, pause_world};
use crate::auth_middleware::AuthenticatedUser;
use crate::test_support::{
    insert_test_user, insert_test_world, insert_test_world_member, test_app_state,
};

const GROUNDS: &str = "Record grounds only an operator reads.";
const DECLINE_NOTE: &str = "Declined: nothing live to protect.";

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

async fn ask(schema: &crate::graphql::AppSchema, who: AuthenticatedUser, query: String) -> Value {
    let response = schema.execute(Request::new(query).data(who)).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    response.data.into_json().unwrap()
}

fn takedown(action: Uuid, entity: Uuid) -> TriggerDetail {
    TriggerDetail {
        kind: PauseTriggerKind::Takedown,
        moderation_action_id: Some(action),
        entity_type: Some("scene".into()),
        entity_id: Some(entity),
        note: None,
    }
}

const PAUSE_FIELDS: &str = "id worldId worldName worldExists grounds requestId \
    pausedBy { id name } pausedAt liftedBy { id name } liftedAt liftGrounds \
    triggers { kind moderationActionId entityType entityId note recordedAt }";

const REQUEST_FIELDS: &str = "id worldId worldName worldExists raisedAt state \
    decidedBy { id name } decidedAt decisionNote \
    triggers { kind moderationActionId entityType entityId note recordedAt }";

fn world_play_state(world: Uuid) -> String {
    format!(
        r#"{{ worldPlayState(worldId: "{world}") {{ paused pausedAt history {{ pausedAt liftedAt }} }} }}"#
    )
}

fn pauses_of_world(world: Uuid) -> String {
    format!(r#"{{ playPauses(worldId: "{world}") {{ nextCursor nodes {{ {PAUSE_FIELDS} }} }} }}"#)
}

fn request_in<'a>(listed: &'a Value, request: &str) -> Option<&'a Value> {
    listed["playPauseRequests"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == request)
}

fn declined_requests() -> String {
    format!(
        "{{ playPauseRequests(state: DECLINED, first: 200) {{ nodes {{ {REQUEST_FIELDS} }} }} }}"
    )
}

/// An operator declines a request raised by a takedown on `world`; the id.
async fn declined_request(
    state: &crate::state::AppState,
    operator: Uuid,
    world: Uuid,
    action: Uuid,
) -> String {
    let request = {
        let mut conn = state.db_pool.get().unwrap();
        match raise(&mut conn, world, &takedown(action, Uuid::now_v7())).unwrap() {
            super::requests::RaiseOutcome::Requested { request_id } => request_id,
            other => panic!("expected a request, got {other:?}"),
        }
    };
    let decided = ask(
        &schema(state.clone()),
        caller(operator, true),
        format!(
            r#"mutation {{ decidePlayPauseRequest(requestId: "{request}", decision: DECLINE, note: "{DECLINE_NOTE}") {{ decidedHere }} }}"#
        ),
    )
    .await;
    assert_eq!(decided["decidePlayPauseRequest"]["decidedHere"], true);
    request.to_string()
}

/// FR-050: a member reads `paused`, `pausedAt` and spans, and nothing else;
/// someone not at the table reads nothing.
#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn a_member_reads_that_and_when_and_a_stranger_reads_nothing() {
    let _lock = super::test_lock();
    let state = test_app_state();
    let (operator, owner, player, stranger, world) = {
        let mut conn = state.db_pool.get().unwrap();
        let operator = insert_test_user(&mut conn);
        let owner = insert_test_user(&mut conn);
        let player = insert_test_user(&mut conn);
        let stranger = insert_test_user(&mut conn);
        let world = insert_test_world(&mut conn, owner);
        insert_test_world_member(&mut conn, world, player, "Player");
        pause_world(
            &mut conn,
            operator,
            world,
            GROUNDS,
            TriggerDetail::operator(),
        )
        .unwrap();
        (operator, owner, player, stranger, world)
    };
    let schema = schema(state.clone());

    for member in [owner, player] {
        let read = ask(&schema, caller(member, false), world_play_state(world)).await;
        let read = &read["worldPlayState"];
        let mut keys: Vec<&String> = read.as_object().unwrap().keys().collect();
        keys.sort();
        assert_eq!(keys, ["history", "paused", "pausedAt"]);
        assert_eq!(read["paused"], true);
        let span = read["history"][0].as_object().unwrap();
        let mut span_keys: Vec<&String> = span.keys().collect();
        span_keys.sort();
        assert_eq!(span_keys, ["liftedAt", "pausedAt"]);
        assert_eq!(read["history"][0]["pausedAt"], read["pausedAt"]);
    }

    // A field a member might hope for is not on the type at all.
    let hoped = schema
        .execute(
            Request::new(format!(
                r#"{{ worldPlayState(worldId: "{world}") {{ paused grounds }} }}"#
            ))
            .data(caller(owner, false)),
        )
        .await;
    assert!(
        !hoped.errors.is_empty(),
        "`grounds` is not a field members have"
    );

    for outsider in [caller(stranger, false), caller(operator, true)] {
        let refused = schema
            .execute(Request::new(world_play_state(world)).data(outsider))
            .await;
        assert!(
            !refused.errors.is_empty(),
            "answered someone not at the table"
        );
        assert!(refused.data.into_json().unwrap().is_null());
    }
}

/// FR-052: a declined request leaves no trace a member can read.
#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn a_declined_request_never_reaches_world_play_state() {
    let _lock = super::test_lock();
    let state = test_app_state();
    let (operator, owner, world) = {
        let mut conn = state.db_pool.get().unwrap();
        let operator = insert_test_user(&mut conn);
        let owner = insert_test_user(&mut conn);
        (operator, owner, insert_test_world(&mut conn, owner))
    };
    let schema = schema(state.clone());
    let before = ask(&schema, caller(owner, false), world_play_state(world)).await;
    assert_eq!(before["worldPlayState"]["paused"], false);

    let request = declined_request(&state, operator, world, Uuid::now_v7()).await;

    let after = ask(&schema, caller(owner, false), world_play_state(world)).await;
    assert_eq!(
        after, before,
        "a member reads exactly what they read before"
    );
    assert!(
        after["worldPlayState"]["history"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    let record = ask(&schema, caller(operator, true), declined_requests()).await;
    assert!(
        request_in(&record, &request).is_some(),
        "the operator record keeps it"
    );
}

/// FR-051, FR-052: the operator record says who, when, the grounds, the
/// triggers and the lift — for a pause approved from a request, whose
/// triggers stay on the request, as well as for one paused directly.
#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn the_operator_record_says_who_when_why_and_the_lift() {
    let _lock = super::test_lock();
    let state = test_app_state();
    let (operator, lifter, world) = {
        let mut conn = state.db_pool.get().unwrap();
        let operator = insert_test_user(&mut conn);
        let lifter = insert_test_user(&mut conn);
        let owner = insert_test_user(&mut conn);
        (operator, lifter, insert_test_world(&mut conn, owner))
    };
    let schema = schema(state.clone());
    let declined_action = Uuid::now_v7();
    let declined = declined_request(&state, operator, world, declined_action).await;

    // Approved from a request: the pause's grounds are the note.
    let (action, entity) = (Uuid::now_v7(), Uuid::now_v7());
    let approved = {
        let mut conn = state.db_pool.get().unwrap();
        match raise(&mut conn, world, &takedown(action, entity)).unwrap() {
            super::requests::RaiseOutcome::Requested { request_id } => request_id,
            other => panic!("expected a request, got {other:?}"),
        }
    };
    let decided = ask(
        &schema,
        caller(operator, true),
        format!(
            r#"mutation {{ decidePlayPauseRequest(requestId: "{approved}", decision: APPROVE, note: "{GROUNDS}") {{ pause {{ id }} }} }}"#
        ),
    )
    .await;
    let pause_id = decided["decidePlayPauseRequest"]["pause"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    {
        let mut conn = state.db_pool.get().unwrap();
        lift_pause(
            &mut conn,
            lifter,
            pause_id.parse().unwrap(),
            "Lifted, on review.",
        )
        .unwrap();
    }

    let pauses = ask(&schema, caller(operator, true), pauses_of_world(world)).await;
    let pause = &pauses["playPauses"]["nodes"][0];
    assert_eq!(pause["id"], pause_id);
    assert_eq!(pause["grounds"], GROUNDS);
    assert_eq!(pause["requestId"], approved.to_string());
    assert_eq!(pause["pausedBy"]["id"], operator.to_string());
    assert!(
        pause["pausedBy"]["name"]
            .as_str()
            .unwrap()
            .starts_with("test_user_")
    );
    assert!(pause["pausedAt"].is_string());
    assert_eq!(pause["liftedBy"]["id"], lifter.to_string());
    assert!(pause["liftedAt"].is_string());
    assert_eq!(pause["liftGrounds"], "Lifted, on review.");
    let triggers = pause["triggers"].as_array().unwrap();
    assert_eq!(
        triggers.len(),
        1,
        "the request's trigger, reached by requestId"
    );
    assert_eq!(triggers[0]["kind"], "TAKEDOWN");
    assert_eq!(triggers[0]["moderationActionId"], action.to_string());
    assert_eq!(triggers[0]["entityType"], "scene");
    assert_eq!(triggers[0]["entityId"], entity.to_string());
    assert!(triggers[0]["recordedAt"].is_string());

    let approved_record = ask(
        &schema,
        caller(operator, true),
        format!("{{ playPauseRequests(state: APPROVED, first: 200) {{ nodes {{ {REQUEST_FIELDS} }} }} }}"),
    )
    .await;
    let request = request_in(&approved_record, &approved.to_string()).expect("approved is listed");
    assert_eq!(request["decisionNote"], GROUNDS);
    assert_eq!(
        request["triggers"][0]["moderationActionId"],
        action.to_string()
    );

    let record = ask(&schema, caller(operator, true), declined_requests()).await;
    let request = request_in(&record, &declined).expect("the declined request is listed");
    assert_eq!(request["state"], "DECLINED");
    assert_eq!(request["worldId"], world.to_string());
    assert_eq!(request["decidedBy"]["id"], operator.to_string());
    assert!(request["decidedAt"].is_string());
    assert!(request["raisedAt"].is_string());
    assert_eq!(request["decisionNote"], DECLINE_NOTE);
    assert_eq!(request["triggers"][0]["kind"], "TAKEDOWN");
    assert_eq!(
        request["triggers"][0]["moderationActionId"],
        declined_action.to_string()
    );
}

/// FR-053: a world deleted while paused leaves its pause and its declined
/// request readable, named as they were, and marked as gone.
#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn a_world_deleted_while_paused_leaves_its_record() {
    let _lock = super::test_lock();
    let state = test_app_state();
    let (operator, owner, world) = {
        let mut conn = state.db_pool.get().unwrap();
        let operator = insert_test_user(&mut conn);
        let owner = insert_test_user(&mut conn);
        (operator, owner, insert_test_world(&mut conn, owner))
    };
    let schema = schema(state.clone());
    let declined = declined_request(&state, operator, world, Uuid::now_v7()).await;
    {
        let mut conn = state.db_pool.get().unwrap();
        pause_world(
            &mut conn,
            operator,
            world,
            GROUNDS,
            TriggerDetail::operator(),
        )
        .unwrap();
    }
    let world_name =
        ask(&schema, caller(operator, true), pauses_of_world(world)).await["playPauses"]["nodes"]
            [0]["worldName"]
            .clone();

    let deleted = ask(
        &schema,
        caller(owner, false),
        format!(r#"mutation {{ deleteWorld(id: "{world}") {{ status }} }}"#),
    )
    .await;
    assert_eq!(deleted["deleteWorld"]["status"], "deleted");

    let pauses = ask(&schema, caller(operator, true), pauses_of_world(world)).await;
    let pause = &pauses["playPauses"]["nodes"][0];
    assert_eq!(pause["worldExists"], false);
    assert_eq!(pause["worldName"], world_name);
    assert_eq!(pause["grounds"], GROUNDS);
    assert!(pause["liftedAt"].is_null(), "still in force, with no world");

    let record = ask(&schema, caller(operator, true), declined_requests()).await;
    let request = request_in(&record, &declined).expect("the declined request outlives its world");
    assert_eq!(request["worldExists"], false);
    assert_eq!(request["worldName"], world_name);
    assert_eq!(request["decisionNote"], DECLINE_NOTE);
}

/// FR-051, data-model *names are snapshots*: deleting the pausing operator's
/// account leaves the name the record was written under.
#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn deleting_the_pausing_operators_account_keeps_their_name_on_the_record() {
    let _lock = super::test_lock();
    let state = test_app_state();
    let (operator, reader, world, name) = {
        let mut conn = state.db_pool.get().unwrap();
        let operator = insert_test_user(&mut conn);
        let reader = insert_test_user(&mut conn);
        let owner = insert_test_user(&mut conn);
        let world = insert_test_world(&mut conn, owner);
        let pause = pause_world(
            &mut conn,
            operator,
            world,
            GROUNDS,
            TriggerDetail::operator(),
        )
        .unwrap()
        .pause;
        lift_pause(&mut conn, operator, pause.id, "Lifted before leaving.").unwrap();
        crate::users::delete_user_data_on(&mut conn, operator)
            .expect("an operator's account can be deleted with pauses on record");
        (operator, reader, world, pause.paused_by_name)
    };

    let pauses = ask(&schema(state), caller(reader, true), pauses_of_world(world)).await;
    let pause = &pauses["playPauses"]["nodes"][0];
    assert_eq!(pause["pausedBy"]["id"], operator.to_string());
    assert_eq!(pause["pausedBy"]["name"], name);
    assert_eq!(pause["liftedBy"]["name"], name);
}

/// The web *Record* section: lifted pauses and decided requests, newest
/// first, a page at a time, with pages that meet and a last page that says
/// so. `state: null` asks for requests in every state.
#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn the_record_pages_newest_first() {
    let _lock = super::test_lock();
    let state = test_app_state();
    let (operator, world) = {
        let mut conn = state.db_pool.get().unwrap();
        let operator = insert_test_user(&mut conn);
        let owner = insert_test_user(&mut conn);
        (operator, insert_test_world(&mut conn, owner))
    };
    let schema = schema(state.clone());

    let mut pauses = Vec::new();
    let mut requests = Vec::new();
    for _ in 0..3 {
        requests.push(declined_request(&state, operator, world, Uuid::now_v7()).await);
        let mut conn = state.db_pool.get().unwrap();
        let pause = pause_world(
            &mut conn,
            operator,
            world,
            GROUNDS,
            TriggerDetail::operator(),
        )
        .unwrap()
        .pause;
        lift_pause(&mut conn, operator, pause.id, "Lifted.").unwrap();
        pauses.push(pause.id.to_string());
    }
    pauses.reverse();
    requests.reverse();

    let mut seen = Vec::new();
    let mut after = String::new();
    loop {
        let page = ask(
            &schema,
            caller(operator, true),
            format!(
                r#"{{ playPauses(active: false, worldId: "{world}", first: 2{after}) {{ nextCursor nodes {{ id }} }} }}"#
            ),
        )
        .await;
        for node in page["playPauses"]["nodes"].as_array().unwrap() {
            seen.push(node["id"].as_str().unwrap().to_string());
        }
        match page["playPauses"]["nextCursor"].as_str() {
            Some(cursor) => after = format!(r#", after: "{cursor}""#),
            None => break,
        }
    }
    assert_eq!(seen, pauses, "lifted pauses, newest first, across pages");

    // Requests are listed across worlds; follow pages until all of this
    // world's are seen, and check they arrive in order.
    for filter in ["state: DECLINED", "state: null"] {
        let mut seen = Vec::new();
        let mut after = String::new();
        for _ in 0..1000 {
            let page = ask(
                &schema,
                caller(operator, true),
                format!(
                    "{{ playPauseRequests({filter}, first: 50{after}) {{ nextCursor nodes {{ id worldId state }} }} }}"
                ),
            )
            .await;
            for node in page["playPauseRequests"]["nodes"].as_array().unwrap() {
                if node["worldId"] == world.to_string() {
                    assert_eq!(node["state"], "DECLINED");
                    seen.push(node["id"].as_str().unwrap().to_string());
                }
            }
            match page["playPauseRequests"]["nextCursor"].as_str() {
                Some(cursor) if seen.len() < requests.len() => {
                    after = format!(r#", after: "{cursor}""#)
                }
                _ => break,
            }
        }
        assert_eq!(seen, requests, "{filter}: decided requests, newest first");
    }
}
