use super::*;
use crate::test_support::{
    insert_test_scene, insert_test_user, insert_test_world, insert_test_world_member,
    test_app_state,
};

/// A world with a Game Master and a player, and one scene.
struct Table {
    state: crate::state::AppState,
    gm: Uuid,
    player: Uuid,
    scene_id: Uuid,
}

fn seat_a_table() -> Table {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let gm = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, gm);
    let scene_id = insert_test_scene(&mut conn, world_id, gm);
    let player = insert_test_user(&mut conn);
    insert_test_world_member(&mut conn, world_id, player, "Player");
    drop(conn);
    Table {
        state,
        gm,
        player,
        scene_id,
    }
}

fn a_prop(scene_id: Uuid, subject: Uuid) -> GraphQLCreateInteractiveInput {
    GraphQLCreateInteractiveInput {
        scene_id,
        subject_kind: String::from("prop"),
        subject_ref: Some(subject),
        geometry: None,
        effect_id: None,
        effect_config: None,
        trigger: String::from("click"),
        activation: String::from("anyone"),
        fire_mode: None,
    }
}

#[tokio::test]
async fn a_player_cannot_author_edit_delete_or_reset_an_interactive() {
    // FR-005, Principle III. Every one of these is refused *here*, at the
    // data boundary, rather than by a client that declined to show the
    // authoring panel.
    let t = seat_a_table();
    let subject = Uuid::now_v7();

    let refused =
        create_interactive_impl(&t.state, t.player, false, a_prop(t.scene_id, subject)).await;
    assert!(refused.is_err(), "a player must not author an interactive");

    // The Game Master's identical call succeeds, so the refusal above is
    // about who asked and not about the request being malformed.
    let created = create_interactive_impl(&t.state, t.gm, false, a_prop(t.scene_id, subject))
        .await
        .expect("the Game Master authors it");
    let id = created.interactive_id;

    let edited = update_interactive_impl(
        &t.state,
        t.player,
        false,
        id,
        GraphQLUpdateInteractiveInput {
            geometry: None,
            effect_id: None,
            effect_config: None,
            trigger: None,
            activation: Some(String::from("gm_only")),
            fire_mode: None,
            clear_effect: None,
        },
    )
    .await;
    assert!(edited.is_err(), "a player must not edit an interactive");

    let reset = reset_interactive_impl(&t.state, t.player, false, id).await;
    assert!(reset.is_err(), "a player must not reset an interactive");

    let deleted = delete_interactive_impl(&t.state, t.player, false, id).await;
    assert!(deleted.is_err(), "a player must not delete an interactive");

    // And it is still there, because a refused delete must not half-happen.
    let mut conn = t.state.db_pool.get().unwrap();
    assert!(crate::interaction::load(&mut conn, id).is_ok());

    let _ = delete_interactive_impl(&t.state, t.gm, false, id).await;
}

#[tokio::test]
async fn scenery_activates_to_no_effect_rather_than_to_an_error() {
    // An interactive with no effect is a legitimate thing to place (US1
    // scenario 3). A player clicking it should be told nothing happened,
    // not shown a failure.
    let t = seat_a_table();
    let created =
        create_interactive_impl(&t.state, t.gm, false, a_prop(t.scene_id, Uuid::now_v7()))
            .await
            .expect("the Game Master places a table");

    let result = activate_interactive_impl(&t.state, t.player, false, created.interactive_id)
        .await
        .expect("clicking scenery is not an error");
    assert_eq!(result.outcome, "noEffect");
    assert!(result.reason.is_none());

    let _ = delete_interactive_impl(&t.state, t.gm, false, created.interactive_id).await;
}

#[tokio::test]
async fn a_stranger_cannot_activate_anything_in_a_world_they_are_not_in() {
    let t = seat_a_table();
    let created =
        create_interactive_impl(&t.state, t.gm, false, a_prop(t.scene_id, Uuid::now_v7()))
            .await
            .expect("authored");

    let stranger = {
        let mut conn = t.state.db_pool.get().unwrap();
        insert_test_user(&mut conn)
    };
    let refused =
        activate_interactive_impl(&t.state, stranger, false, created.interactive_id).await;
    assert!(
        refused.is_err(),
        "membership is checked before anything else"
    );

    let _ = delete_interactive_impl(&t.state, t.gm, false, created.interactive_id).await;
}

#[tokio::test]
async fn an_effect_no_contributor_declares_is_refused_at_authoring_time() {
    // The other half of FR-041: an interactive can only *become* stale, it
    // cannot be authored stale.
    let t = seat_a_table();
    let mut input = a_prop(t.scene_id, Uuid::now_v7());
    input.effect_id = Some(String::from("audio.play"));

    let refused = create_interactive_impl(&t.state, t.gm, false, input).await;
    assert!(
        refused.is_err(),
        "no audio subsystem exists, so nothing may be authored against it"
    );
}

/// A wall on the table's scene, closed and unlocked.
fn a_wall(t: &Table) -> Uuid {
    use crate::schema::walls;
    let mut conn = t.state.db_pool.get().unwrap();
    let wall_id = Uuid::now_v7();
    let now = Utc::now().naive_utc();
    diesel::insert_into(walls::table)
        .values((
            walls::wall_id.eq(wall_id),
            walls::scene_id.eq(t.scene_id),
            walls::x1.eq(0.0f64),
            walls::y1.eq(0.0f64),
            walls::x2.eq(100.0f64),
            walls::y2.eq(0.0f64),
            walls::blocks_vision.eq(true),
            walls::blocks_movement.eq(true),
            walls::door_state.eq("closed"),
            walls::created_by.eq(t.gm),
            walls::updated_by.eq(t.gm),
            walls::created_at.eq(now),
            walls::updated_at.eq(now),
        ))
        .execute(&mut conn)
        .expect("insert wall");
    wall_id
}

fn door_state_of(t: &Table, wall_id: Uuid) -> String {
    use crate::schema::walls;
    let mut conn = t.state.db_pool.get().unwrap();
    walls::table
        .filter(walls::wall_id.eq(wall_id))
        .select(walls::door_state)
        .first(&mut conn)
        .expect("wall exists")
}

/// The interactive `setDoorDesignation` creates for a door.
async fn designate(t: &Table, wall_id: Uuid) -> Uuid {
    use crate::schema::interactives;
    set_door_designation_impl(&t.state, t.gm, false, wall_id, true)
        .await
        .expect("the Game Master designates a door");
    let mut conn = t.state.db_pool.get().unwrap();
    interactives::table
        .filter(interactives::subject_ref.eq(wall_id))
        .select(interactives::interactive_id)
        .first(&mut conn)
        .expect("designating a door gives it an interactive")
}

#[tokio::test]
async fn a_player_cannot_open_a_locked_door_at_the_server() {
    // The rule most likely to be implemented by not drawing the button.
    // A screen test would pass against a server that happily performs the
    // change when asked directly, which is why this asks directly.
    let t = seat_a_table();
    let wall_id = a_wall(&t);
    let interactive_id = designate(&t, wall_id).await;

    // Unlocked: the player opens it, and the change is durable.
    let opened = activate_interactive_impl(&t.state, t.player, false, interactive_id)
        .await
        .expect("an unlocked door opens");
    assert_eq!(opened.outcome, "performed");
    assert_eq!(door_state_of(&t, wall_id), "open");

    set_door_flag_impl(&t.state, t.gm, false, wall_id, DoorFlag::Locked(true))
        .await
        .expect("the Game Master locks it");

    let refused = activate_interactive_impl(&t.state, t.player, false, interactive_id)
        .await
        .expect("a refusal is an outcome, not an error");
    assert_eq!(refused.outcome, "refused");
    assert_eq!(refused.reason.as_deref(), Some("locked"));
    // And nothing moved. A refusal that still performed the effect would
    // pass an outcome assertion and fail the table.
    assert_eq!(door_state_of(&t, wall_id), "open");

    let _ = delete_interactive_impl(&t.state, t.gm, false, interactive_id).await;
}

#[tokio::test]
async fn a_game_master_can_still_change_a_locked_door() {
    // FR-013. The lock is theirs; it is not a rule against them.
    let t = seat_a_table();
    let wall_id = a_wall(&t);
    let interactive_id = designate(&t, wall_id).await;

    set_door_flag_impl(&t.state, t.gm, false, wall_id, DoorFlag::Locked(true))
        .await
        .expect("locked");

    let performed = activate_interactive_impl(&t.state, t.gm, false, interactive_id)
        .await
        .expect("the Game Master opens their own locked door");
    assert_eq!(performed.outcome, "performed");
    assert_eq!(door_state_of(&t, wall_id), "open");

    let _ = delete_interactive_impl(&t.state, t.gm, false, interactive_id).await;
}

#[tokio::test]
async fn a_player_cannot_lock_designate_or_reveal_a_door() {
    let t = seat_a_table();
    let wall_id = a_wall(&t);

    assert!(
        set_door_designation_impl(&t.state, t.player, false, wall_id, true)
            .await
            .is_err(),
        "a player must not designate a door"
    );
    assert!(
        set_door_flag_impl(&t.state, t.player, false, wall_id, DoorFlag::Locked(true))
            .await
            .is_err(),
        "a player must not lock a door"
    );
    assert!(
        set_door_flag_impl(&t.state, t.player, false, wall_id, DoorFlag::Secret(true))
            .await
            .is_err(),
        "a player must not hide a door"
    );
}

#[tokio::test]
async fn toggling_a_door_twice_returns_it_to_where_it_started() {
    let t = seat_a_table();
    let wall_id = a_wall(&t);
    let interactive_id = designate(&t, wall_id).await;

    assert_eq!(door_state_of(&t, wall_id), "closed");
    let _ = activate_interactive_impl(&t.state, t.player, false, interactive_id).await;
    assert_eq!(door_state_of(&t, wall_id), "open");
    let _ = activate_interactive_impl(&t.state, t.player, false, interactive_id).await;
    assert_eq!(door_state_of(&t, wall_id), "closed");

    let _ = delete_interactive_impl(&t.state, t.gm, false, interactive_id).await;
}

#[tokio::test]
async fn undesignating_a_door_takes_its_interactive_with_it() {
    // A door on a wall that is no longer a door is not a thing, and an
    // interactive left behind would be a click that does nothing.
    use crate::schema::interactives;
    let t = seat_a_table();
    let wall_id = a_wall(&t);
    designate(&t, wall_id).await;

    set_door_designation_impl(&t.state, t.gm, false, wall_id, false)
        .await
        .expect("undesignated");

    let mut conn = t.state.db_pool.get().unwrap();
    let remaining: i64 = interactives::table
        .filter(interactives::subject_ref.eq(wall_id))
        .count()
        .get_result(&mut conn)
        .unwrap();
    assert_eq!(remaining, 0);
    assert_eq!(door_state_of(&t, wall_id), "none");
}

/// A door the player must ask about.
async fn a_gated_door(t: &Table) -> (Uuid, Uuid) {
    let wall_id = a_wall(t);
    let interactive_id = designate(t, wall_id).await;
    update_interactive_impl(
        &t.state,
        t.gm,
        false,
        interactive_id,
        GraphQLUpdateInteractiveInput {
            geometry: None,
            effect_id: None,
            effect_config: None,
            trigger: None,
            activation: Some(String::from("requires_approval")),
            fire_mode: None,
            clear_effect: None,
        },
    )
    .await
    .expect("the Game Master gates it");
    (wall_id, interactive_id)
}

#[tokio::test]
async fn a_request_never_expires_into_approval() {
    // FR-027, and the reason there is no timeout anywhere in this file.
    // Silence is not consent, and a queue that eventually says yes on the
    // Game Master's behalf is a queue that decides things they did not.
    let t = seat_a_table();
    let (wall_id, interactive_id) = a_gated_door(&t).await;

    let asked = activate_interactive_impl(&t.state, t.player, false, interactive_id)
        .await
        .expect("the player asks");
    assert_eq!(asked.outcome, "requested");
    assert!(asked.request_id.is_some());

    // Nothing happened, and nothing will until somebody decides.
    assert_eq!(door_state_of(&t, wall_id), "closed");

    let mut conn = t.state.db_pool.get().unwrap();
    let still_pending = crate::interaction::pending_for_scene(&mut conn, t.scene_id).unwrap();
    assert_eq!(still_pending.len(), 1);
    assert_eq!(still_pending[0].state, crate::interaction::REQUEST_PENDING);
    drop(conn);

    let _ = delete_interactive_impl(&t.state, t.gm, false, interactive_id).await;
}

#[tokio::test]
async fn approval_re_checks_permission_at_decision_time() {
    // A Game Master who locks the door after the request was raised has
    // contradicted themselves, and the lock wins — it is the more recent
    // statement of what they want. Trusting the request's own moment would
    // make approval a way to perform something currently forbidden.
    let t = seat_a_table();
    let (wall_id, interactive_id) = a_gated_door(&t).await;

    let asked = activate_interactive_impl(&t.state, t.player, false, interactive_id)
        .await
        .expect("the player asks");
    let request_id = asked.request_id.expect("raised");

    set_door_flag_impl(&t.state, t.gm, false, wall_id, DoorFlag::Locked(true))
        .await
        .expect("and then the GM locks it");

    let decided = decide_request_impl(&t.state, t.gm, false, request_id, true)
        .await
        .expect("the GM approves anyway");
    assert_eq!(decided.outcome, "refused");
    assert_eq!(decided.reason.as_deref(), Some("locked"));
    assert_eq!(
        door_state_of(&t, wall_id),
        "closed",
        "and the door did not move"
    );

    let _ = delete_interactive_impl(&t.state, t.gm, false, interactive_id).await;
}

#[tokio::test]
async fn approving_runs_the_effect_and_refusing_changes_nothing() {
    let t = seat_a_table();
    let (wall_id, interactive_id) = a_gated_door(&t).await;

    // Refused first, so "nothing changed" is checked against a door that
    // could have moved rather than one that never could.
    let first = activate_interactive_impl(&t.state, t.player, false, interactive_id)
        .await
        .expect("asked");
    decide_request_impl(&t.state, t.gm, false, first.request_id.unwrap(), false)
        .await
        .expect("refused");
    assert_eq!(door_state_of(&t, wall_id), "closed");

    let second = activate_interactive_impl(&t.state, t.player, false, interactive_id)
        .await
        .expect("asked again");
    let approved = decide_request_impl(&t.state, t.gm, false, second.request_id.unwrap(), true)
        .await
        .expect("approved");
    assert_eq!(approved.outcome, "performed");
    assert_eq!(door_state_of(&t, wall_id), "open");

    let _ = delete_interactive_impl(&t.state, t.gm, false, interactive_id).await;
}

#[tokio::test]
async fn a_player_cannot_decide_a_request_including_their_own() {
    let t = seat_a_table();
    let (_wall_id, interactive_id) = a_gated_door(&t).await;

    let asked = activate_interactive_impl(&t.state, t.player, false, interactive_id)
        .await
        .expect("asked");
    let request_id = asked.request_id.expect("raised");

    assert!(
        decide_request_impl(&t.state, t.player, false, request_id, true)
            .await
            .is_err(),
        "a player approving their own request is the whole thing this gate prevents"
    );

    let _ = delete_interactive_impl(&t.state, t.gm, false, interactive_id).await;
}

#[tokio::test]
async fn a_decided_request_cannot_be_decided_again() {
    // Two Game Masters clicking approve and refuse must not race into
    // whichever transaction committed last.
    let t = seat_a_table();
    let (_wall_id, interactive_id) = a_gated_door(&t).await;

    let asked = activate_interactive_impl(&t.state, t.player, false, interactive_id)
        .await
        .expect("asked");
    let request_id = asked.request_id.expect("raised");

    decide_request_impl(&t.state, t.gm, false, request_id, false)
        .await
        .expect("refused");
    assert!(
        decide_request_impl(&t.state, t.gm, false, request_id, true)
            .await
            .is_err(),
        "a decision already made is not reopened by asking again"
    );

    let _ = delete_interactive_impl(&t.state, t.gm, false, interactive_id).await;
}

#[tokio::test]
async fn a_game_masters_own_activation_does_not_queue() {
    // They are the person the queue exists to ask.
    let t = seat_a_table();
    let (wall_id, interactive_id) = a_gated_door(&t).await;

    let performed = activate_interactive_impl(&t.state, t.gm, false, interactive_id)
        .await
        .expect("the GM acts");
    assert_eq!(performed.outcome, "performed");
    assert!(performed.request_id.is_none());
    assert_eq!(door_state_of(&t, wall_id), "open");

    let _ = delete_interactive_impl(&t.state, t.gm, false, interactive_id).await;
}

#[tokio::test]
async fn a_prop_cannot_be_authored_with_an_entry_trigger() {
    let t = seat_a_table();
    let mut input = a_prop(t.scene_id, Uuid::now_v7());
    input.trigger = String::from("enter");

    let refused = create_interactive_impl(&t.state, t.gm, false, input).await;
    assert!(refused.is_err(), "a book cannot be crossed");
}
