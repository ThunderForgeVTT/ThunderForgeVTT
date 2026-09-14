//! Spec 051 T020 and T057: the pause record as an operator reads it, and
//! *that and when* as a member does — driven through the real schema, because
//! what a member cannot ask for is a property of the schema, not of a function.

use async_graphql::Request;
use serde_json::Value;
use uuid::Uuid;

use crate::auth_middleware::AuthenticatedUser;
use crate::play_pause::{TriggerDetail, lift_pause, pause_world};
use crate::test_support::{
    insert_test_user, insert_test_world, insert_test_world_member, test_app_state,
};

const GROUNDS: &str = "Grounds only an operator may read.";

fn schema(state: crate::state::AppState) -> crate::graphql::AppSchema {
    async_graphql::Schema::build(
        crate::graphql::QueryRoot::default(),
        crate::graphql::MutationRoot::default(),
        crate::graphql::SubscriptionRoot,
    )
    .data(state)
    .finish()
}

fn as_user(user_id: Uuid) -> AuthenticatedUser {
    AuthenticatedUser {
        user_id,
        session_id: Uuid::now_v7(),
        expires_at: chrono::Utc::now().naive_utc() + chrono::Duration::hours(1),
        is_admin: false,
        role: "User".to_string(),
        disabled: false,
    }
}

fn as_admin(user_id: Uuid) -> AuthenticatedUser {
    AuthenticatedUser {
        is_admin: true,
        role: "Admin".to_string(),
        ..as_user(user_id)
    }
}

async fn run(
    schema: &crate::graphql::AppSchema,
    who: AuthenticatedUser,
    query: String,
) -> async_graphql::Response {
    schema.execute(Request::new(query).data(who)).await
}

fn data(response: async_graphql::Response) -> Value {
    assert!(
        response.errors.is_empty(),
        "unexpected errors: {:?}",
        response.errors
    );
    response.data.into_json().unwrap()
}

/// A world with an Owner and a Player, an operator, and a pause that was
/// lifted followed by one in force.
struct Paused {
    state: crate::state::AppState,
    operator: Uuid,
    owner: Uuid,
    player: Uuid,
    world: Uuid,
}

fn a_paused_world() -> Paused {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let operator = insert_test_user(&mut conn);
    let owner = insert_test_user(&mut conn);
    let player = insert_test_user(&mut conn);
    let world = insert_test_world(&mut conn, owner);
    insert_test_world_member(&mut conn, world, player, "Player");

    let earlier = pause_world(
        &mut conn,
        operator,
        world,
        GROUNDS,
        TriggerDetail::operator(),
    )
    .unwrap()
    .pause;
    lift_pause(
        &mut conn,
        operator,
        earlier.id,
        "Lift grounds, also private.",
    )
    .unwrap();
    pause_world(
        &mut conn,
        operator,
        world,
        GROUNDS,
        TriggerDetail::operator(),
    )
    .unwrap();
    drop(conn);

    Paused {
        state,
        operator,
        owner,
        player,
        world,
    }
}

/// FR-024, FR-050: a member of a paused world is told that and when — while
/// it is paused, which is when the notice asks.
#[tokio::test]
async fn world_play_state_answers_a_member_while_paused() {
    let p = a_paused_world();
    let schema = schema(p.state.clone());

    for member in [p.owner, p.player] {
        let state = data(
            run(
                &schema,
                as_user(member),
                format!(
                    r#"{{ worldPlayState(worldId: "{}") {{ paused pausedAt history {{ pausedAt liftedAt }} }} }}"#,
                    p.world
                ),
            )
            .await,
        );
        let state = &state["worldPlayState"];
        assert_eq!(state["paused"], true);
        assert!(state["pausedAt"].is_string());

        let history = state["history"].as_array().unwrap();
        assert_eq!(history.len(), 2, "both spans, lifted and in force");
        assert!(history[0]["liftedAt"].is_null(), "newest first");
        assert_eq!(history[0]["pausedAt"], state["pausedAt"]);
        assert!(history[1]["liftedAt"].is_string());
    }
}

/// Not a member, not an answer — and a site admin who is not a member is not
/// one either.
#[tokio::test]
async fn world_play_state_refuses_a_non_member_and_an_admin_who_is_not_one() {
    let p = a_paused_world();
    let schema = schema(p.state.clone());
    let stranger = {
        let mut conn = p.state.db_pool.get().unwrap();
        insert_test_user(&mut conn)
    };

    for who in [as_user(stranger), as_admin(p.operator)] {
        let response = run(
            &schema,
            who,
            format!(
                r#"{{ worldPlayState(worldId: "{}") {{ paused }} }}"#,
                p.world
            ),
        )
        .await;
        assert!(
            !response.errors.is_empty(),
            "worldPlayState answered someone who is not at the table"
        );
        assert!(response.data.into_json().unwrap().is_null());
    }
}

/// FR-011, SC-004: the member-facing types have no field for a reason, a name
/// or a trigger, so no query can ask for one; and the answer to the widest
/// query a member can write carries none of the private text.
#[tokio::test]
async fn world_play_state_exposes_no_grounds() {
    let p = a_paused_world();
    let schema = schema(p.state.clone());

    let fields_of = |type_name: &str| {
        let sdl = schema.sdl();
        let start = sdl
            .find(&format!("type {type_name} {{"))
            .unwrap_or_else(|| panic!("type {type_name} in the schema"));
        let body = &sdl[start..];
        let body = &body[..body.find("\n}").unwrap()];
        let mut fields: Vec<String> = body
            .lines()
            .skip(1)
            .map(str::trim)
            .filter(|line| line.contains(':') && !line.starts_with('"'))
            .map(|line| line.split(':').next().unwrap().to_string())
            .collect();
        fields.sort();
        fields
    };
    assert_eq!(
        fields_of("WorldPlayState"),
        ["history", "paused", "pausedAt"]
    );
    assert_eq!(fields_of("PlayPauseSpan"), ["liftedAt", "pausedAt"]);

    let answer = data(
        run(
            &schema,
            as_user(p.player),
            format!(
                r#"{{ worldPlayState(worldId: "{}") {{ paused pausedAt history {{ pausedAt liftedAt }} }} }}"#,
                p.world
            ),
        )
        .await,
    )
    .to_string();
    for private in [GROUNDS, "Lift grounds", "test_user_"] {
        assert!(
            !answer.contains(private),
            "a member's answer carried `{private}`"
        );
    }
}

/// The operator and member fields, in the SDL the web client is written
/// against (contracts/graphql.md). Printed on failure, so a mismatch reads as
/// the schema it is.
#[test]
fn the_play_pause_surface_matches_the_contract() {
    let sdl = schema(test_app_state()).sdl();
    for expected in [
        "worldPlayState(worldId: UUID!): WorldPlayState!",
        "playPauses(active: Boolean, worldId: UUID, first: Int, after: String): PlayPauseConnection!",
        "playPauseCandidates(search: String!, first: Int = 20): [PauseCandidateWorld!]!",
        "pauseWorldPlay(worldId: UUID!, grounds: String!): PauseOutcome!",
        "enum PauseTriggerKind {",
    ] {
        assert!(sdl.contains(expected), "`{expected}` missing from:\n{sdl}");
    }
}

/// `playPauses`: grounds, who, triggers, the lift — and pages that meet.
#[tokio::test]
async fn play_pauses_lists_the_record_for_an_operator() {
    let p = a_paused_world();
    let schema = schema(p.state.clone());
    let query = |extra: &str| {
        format!(
            r#"{{ playPauses(worldId: "{}"{extra}) {{ nextCursor nodes {{
                id worldId worldName worldExists grounds requestId playedNow
                pausedBy {{ id name }} pausedAt liftedBy {{ id name }} liftedAt liftGrounds
                triggers {{ kind moderationActionId entityType entityId note recordedAt }}
            }} }} }}"#,
            p.world
        )
    };

    let all = data(run(&schema, as_admin(p.operator), query("")).await);
    let nodes = all["playPauses"]["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 2);
    let active = &nodes[0];
    assert!(active["liftedAt"].is_null(), "newest first");
    assert_eq!(active["grounds"], GROUNDS);
    assert_eq!(active["worldExists"], true);
    assert_eq!(active["pausedBy"]["id"], p.operator.to_string());
    assert!(
        active["pausedBy"]["name"]
            .as_str()
            .unwrap()
            .starts_with("test_user_")
    );
    assert_eq!(active["triggers"][0]["kind"], "OPERATOR");
    assert_eq!(nodes[1]["liftGrounds"], "Lift grounds, also private.");
    assert_eq!(nodes[1]["liftedBy"]["id"], p.operator.to_string());

    let active_only = data(run(&schema, as_admin(p.operator), query(", active: true")).await);
    assert_eq!(
        active_only["playPauses"]["nodes"].as_array().unwrap().len(),
        1
    );

    let first = data(run(&schema, as_admin(p.operator), query(", first: 1")).await);
    let cursor = first["playPauses"]["nextCursor"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(first["playPauses"]["nodes"][0]["id"], active["id"]);
    let second = data(
        run(
            &schema,
            as_admin(p.operator),
            query(&format!(r#", first: 1, after: "{cursor}""#)),
        )
        .await,
    );
    assert_eq!(second["playPauses"]["nodes"][0]["id"], nodes[1]["id"]);
    assert!(
        second["playPauses"]["nextCursor"].is_null(),
        "the last page says so"
    );
}

/// `playPauseCandidates`: by name or by id, with its owner and whether it is
/// paused.
#[tokio::test]
async fn play_pause_candidates_finds_a_world_by_name_or_id() {
    let p = a_paused_world();
    let schema = schema(p.state.clone());
    let name = format!("Test World {}", p.world.simple());

    for search in [name.clone(), p.world.to_string()] {
        let found = data(
            run(
                &schema,
                as_admin(p.operator),
                format!(
                    r#"{{ playPauseCandidates(search: "{search}") {{ id name ownerName playedNow paused }} }}"#
                ),
            )
            .await,
        );
        let found = found["playPauseCandidates"].as_array().unwrap();
        assert_eq!(found.len(), 1, "searching `{search}`");
        assert_eq!(found[0]["id"], p.world.to_string());
        assert_eq!(found[0]["name"], name);
        assert!(
            found[0]["ownerName"]
                .as_str()
                .unwrap()
                .starts_with("test_user_")
        );
        assert_eq!(found[0]["paused"], true);
        assert_eq!(found[0]["playedNow"], false);
    }

    let wildcard = data(
        run(
            &schema,
            as_admin(p.operator),
            r#"{ playPauseCandidates(search: "%_no world is named this_%") { id } }"#.to_string(),
        )
        .await,
    );
    assert!(
        wildcard["playPauseCandidates"]
            .as_array()
            .unwrap()
            .is_empty(),
        "`%` and `_` are letters, not wildcards"
    );
}
