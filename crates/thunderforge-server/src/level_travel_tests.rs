//! Walking between floors, against a real database: stairs that take a
//! token, stairs that refuse, stairs that ask first, and the two things a
//! pair of stairs must never do — bounce a token back, or leave its torch
//! behind.

use super::*;
use crate::graphql::mutations_interactives::{activate_interactive_as, decide_request_impl};
use crate::scene_levels::tests::{Table, seat_a_table};

const STEP: f64 = 100.0;

/// A square region, `STEP` on a side, with its corner at `at`.
fn region(t: &Table, level: Uuid, at: (f64, f64), activation: &str) -> Uuid {
    place(
        t,
        level,
        "region",
        None,
        Some(serde_json::json!({
            "shape": "rect", "x": at.0, "y": at.1, "width": STEP, "height": STEP,
        })),
        "enter",
        activation,
    )
}

fn place(
    t: &Table,
    level: Uuid,
    kind: &str,
    subject: Option<Uuid>,
    geometry: Option<serde_json::Value>,
    trigger: &str,
    activation: &str,
) -> Uuid {
    use crate::schema::interactives;
    diesel::insert_into(interactives::table)
        .values((
            interactives::scene_id.eq(t.scene_id),
            interactives::level_id.eq(level),
            interactives::subject_kind.eq(kind),
            interactives::subject_ref.eq(subject),
            interactives::geometry.eq(geometry),
            interactives::trigger.eq(trigger),
            interactives::activation.eq(activation),
            interactives::fire_mode.eq("repeatable"),
            interactives::created_by.eq(t.gm),
            interactives::updated_by.eq(t.gm),
        ))
        .returning(interactives::interactive_id)
        .get_result(&mut t.conn())
        .expect("an interactive is placed")
}

/// Point `from` at `to` as its way between levels.
fn lead(t: &Table, from: Uuid, to: Uuid) {
    use crate::schema::interactives;
    diesel::update(interactives::table.filter(interactives::interactive_id.eq(from)))
        .set((
            interactives::effect_id.eq(TRAVEL),
            interactives::effect_config.eq(serde_json::json!({ "partner": to.to_string() })),
        ))
        .execute(&mut t.conn())
        .unwrap();
}

/// Stairs: a region on each of two levels, each leading to the other.
fn stairs(t: &Table, below: Uuid, above: Uuid, activation: &str) -> (Uuid, Uuid) {
    let foot = region(t, below, (500.0, 500.0), activation);
    let head = region(t, above, (900.0, 100.0), activation);
    lead(t, foot, head);
    lead(t, head, foot);
    (foot, head)
}

/// `moveOwnToken`, through the schema, as the player would send it.
async fn walk(t: &Table, user: Uuid, token: Uuid, to: (f64, f64)) -> serde_json::Value {
    let response = t
        .ask(
            user,
            format!(
                "mutation {{ moveOwnToken(tokenId: \"{token}\", x: {}, y: {}) \
                 {{ tokenId levelId x y }} }}",
                to.0, to.1
            ),
        )
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    response.data.into_json().unwrap()["moveOwnToken"].clone()
}

fn where_is(t: &Table, token: Uuid) -> (Uuid, f64, f64) {
    use crate::schema::tokens;
    tokens::table
        .filter(tokens::token_id.eq(token))
        .select((tokens::level_id, tokens::x, tokens::y))
        .first(&mut t.conn())
        .unwrap()
}

fn events_of(t: &Table, code: i32) -> Vec<serde_json::Value> {
    use crate::schema::world_events;
    world_events::table
        .filter(world_events::world_id.eq(t.world_id))
        .filter(world_events::event_code.eq(code))
        .order(world_events::id.asc())
        .select(world_events::token_event)
        .load::<Option<serde_json::Value>>(&mut t.conn())
        .unwrap()
        .into_iter()
        .flatten()
        .collect()
}

#[test]
fn arrivals_fan_out_instead_of_stacking() {
    let centre = (50.0, 50.0);
    assert_eq!(free_spot(centre, 10.0, &[]), centre);
    let second = free_spot(centre, 10.0, &[centre]);
    assert_ne!(second, centre);
    let third = free_spot(centre, 10.0, &[centre, second]);
    assert!(third != centre && third != second);
    // Deterministic: the same crowd fans out the same way.
    assert_eq!(free_spot(centre, 10.0, &[centre]), second);
}

#[tokio::test]
async fn walking_onto_the_stairs_takes_the_token_and_its_torch_and_does_not_bounce() {
    use crate::schema::light_sources;
    let t = seat_a_table();
    let loft = t.level("Loft");
    stairs(&t, t.ground, loft, "anyone");
    let ann = t.token(t.ann, None, (100.0, 100.0));
    let torch: Uuid = diesel::insert_into(light_sources::table)
        .values((
            light_sources::scene_id.eq(t.scene_id),
            light_sources::x.eq(100.0),
            light_sources::y.eq(100.0),
            light_sources::radius.eq(40.0),
            light_sources::bright_radius.eq(20.0),
            light_sources::intensity.eq(1.0),
            light_sources::attached_token_id.eq(ann),
            light_sources::created_by.eq(t.gm),
            light_sources::updated_by.eq(t.gm),
        ))
        .returning(light_sources::light_id)
        .get_result(&mut t.conn())
        .unwrap();

    // A step that does not reach the stairs is only a step.
    let stepped = walk(&t, t.ann, ann, (300.0, 300.0)).await;
    assert_eq!(stepped["levelId"], t.ground.to_string());

    // Onto the foot of the stairs: the answer is already the loft, at the
    // middle of the head of the stairs.
    let arrived = walk(&t, t.ann, ann, (550.0, 550.0)).await;
    assert_eq!(arrived["levelId"], loft.to_string());
    assert_eq!(
        (arrived["x"].as_f64(), arrived["y"].as_f64()),
        (Some(950.0), Some(150.0))
    );
    assert_eq!(where_is(&t, ann), (loft, 950.0, 150.0));

    // The torch is where she is.
    let lit: (Uuid, f64, f64) = light_sources::table
        .filter(light_sources::light_id.eq(torch))
        .select((light_sources::level_id, light_sources::x, light_sources::y))
        .first(&mut t.conn())
        .unwrap();
    assert_eq!(lit, (loft, 950.0, 150.0));

    // She arrived *inside* the head of the stairs, and was not sent back:
    // arriving is not entering, and neither is a step taken within it.
    let shuffled = walk(&t, t.ann, ann, (960.0, 160.0)).await;
    assert_eq!(shuffled["levelId"], loft.to_string());
    assert_eq!(where_is(&t, ann), (loft, 960.0, 160.0));

    // Out and back in is a deliberate trip down.
    walk(&t, t.ann, ann, (700.0, 150.0)).await;
    let down = walk(&t, t.ann, ann, (950.0, 150.0)).await;
    assert_eq!(down["levelId"], t.ground.to_string());
    assert_eq!(where_is(&t, ann), (t.ground, 550.0, 550.0));

    // Two trips, two travel events, and neither says which floor.
    let travelled = events_of(&t, crate::world_events::EVENT_CODE_TOKEN_TRAVELLED);
    assert_eq!(travelled.len(), 2);
    assert_eq!(
        travelled[0],
        serde_json::json!({ "token_id": ann, "scene_id": t.scene_id })
    );

    // Ben, who stayed downstairs, saw none of the loft while she was there —
    // and now that she is back, he sees her again.
    let ben = t.token(t.ben, None, (0.0, 0.0));
    let seen = t.tokens_seen(t.ben, None).await;
    assert!(seen.contains(&ann.to_string()) && seen.contains(&ben.to_string()));
}

#[tokio::test]
async fn game_master_only_stairs_let_a_player_walk_over_them() {
    let t = seat_a_table();
    let loft = t.level("Loft");
    stairs(&t, t.ground, loft, "gm_only");
    let ann = t.token(t.ann, None, (100.0, 100.0));

    let stood = walk(&t, t.ann, ann, (550.0, 550.0)).await;
    assert_eq!(stood["levelId"], t.ground.to_string());
    assert_eq!(
        where_is(&t, ann),
        (t.ground, 550.0, 550.0),
        "the move itself stands"
    );
    assert!(events_of(&t, crate::world_events::EVENT_CODE_TOKEN_TRAVELLED).is_empty());
}

#[tokio::test]
async fn stairs_that_need_approval_ask_and_take_the_token_when_approved() {
    let t = seat_a_table();
    let loft = t.level("Loft");
    stairs(&t, t.ground, loft, "requires_approval");
    let ann = t.token(t.ann, None, (100.0, 100.0));

    // She walks on and stays; a request is waiting, and it knows who asked.
    let stood = walk(&t, t.ann, ann, (550.0, 550.0)).await;
    assert_eq!(stood["levelId"], t.ground.to_string());
    let (request_id, traveller): (Uuid, Option<Uuid>) = {
        use crate::schema::interaction_requests as r;
        r::table
            .filter(r::scene_id.eq(t.scene_id))
            .select((r::request_id, r::token_id))
            .first(&mut t.conn())
            .expect("a request was raised")
    };
    assert_eq!(traveller, Some(ann));

    let decided = decide_request_impl(&t.state, t.gm, false, request_id, true)
        .await
        .expect("the Game Master approves");
    assert_eq!(decided.outcome, "performed");
    assert_eq!(where_is(&t, ann), (loft, 950.0, 150.0));
}

#[tokio::test]
async fn a_locked_door_refuses_and_an_open_one_is_clicked_through_by_a_named_token() {
    use crate::schema::walls;
    let t = seat_a_table();
    let cellar = t.level("Cellar");
    let ann = t.token(t.ann, None, (100.0, 100.0));

    // A trapdoor on the ground floor, leading to a region in the cellar.
    let trapdoor: Uuid = diesel::insert_into(walls::table)
        .values((
            walls::scene_id.eq(t.scene_id),
            walls::x1.eq(200.0),
            walls::y1.eq(200.0),
            walls::x2.eq(300.0),
            walls::y2.eq(200.0),
            walls::door_state.eq("closed"),
            walls::locked.eq(true),
            walls::created_by.eq(t.gm),
            walls::updated_by.eq(t.gm),
        ))
        .returning(walls::wall_id)
        .get_result(&mut t.conn())
        .unwrap();
    let hatch = place(
        &t,
        t.ground,
        "door",
        Some(trapdoor),
        None,
        "click",
        "anyone",
    );
    let landing = region(&t, cellar, (0.0, 0.0), "anyone");
    lead(&t, hatch, landing);

    let locked = activate_interactive_as(&t.state, t.ann, false, hatch, Some(ann))
        .await
        .expect("a locked door is an answer, not an error");
    assert_eq!(
        (locked.outcome.as_str(), locked.reason.as_deref()),
        ("refused", Some("locked"))
    );
    assert_eq!(where_is(&t, ann).0, t.ground);

    diesel::update(walls::table.filter(walls::wall_id.eq(trapdoor)))
        .set(walls::locked.eq(false))
        .execute(&mut t.conn())
        .unwrap();

    // A click has to say who is going, and it has to be the caller's token.
    assert!(
        activate_interactive_as(&t.state, t.ann, false, hatch, None)
            .await
            .is_err(),
        "nobody named"
    );
    let bens = t.token(t.ben, None, (0.0, 0.0));
    assert!(
        activate_interactive_as(&t.state, t.ann, false, hatch, Some(bens))
            .await
            .is_err(),
        "somebody else's token"
    );
    assert_eq!(where_is(&t, bens).0, t.ground);

    let through = activate_interactive_as(&t.state, t.ann, false, hatch, Some(ann))
        .await
        .expect("down the hatch");
    assert_eq!(through.outcome, "performed");
    assert_eq!(where_is(&t, ann), (cellar, 50.0, 50.0));

    // From the cellar she cannot use the hatch on the floor above.
    assert!(
        activate_interactive_as(&t.state, t.ann, false, hatch, Some(ann))
            .await
            .is_err(),
        "a ladder is not climbed from another floor"
    );
}

#[tokio::test]
async fn a_way_that_leads_nowhere_takes_nobody() {
    let t = seat_a_table();
    let loft = t.level("Loft");
    let ann = t.token(t.ann, None, (100.0, 100.0));

    // Its partner is in another scene.
    let other = {
        let mut conn = t.conn();
        crate::test_support::insert_test_scene_named(&mut conn, t.world_id, t.gm, "Elsewhere")
    };
    let foot = region(&t, t.ground, (500.0, 500.0), "anyone");
    let abroad = {
        use crate::schema::interactives;
        diesel::insert_into(interactives::table)
            .values((
                interactives::scene_id.eq(other),
                interactives::subject_kind.eq("region"),
                interactives::geometry.eq(serde_json::json!({
                    "shape": "rect", "x": 0.0, "y": 0.0, "width": STEP, "height": STEP,
                })),
                interactives::trigger.eq("enter"),
                interactives::activation.eq("anyone"),
                interactives::fire_mode.eq("repeatable"),
                interactives::created_by.eq(t.gm),
                interactives::updated_by.eq(t.gm),
            ))
            .returning(interactives::interactive_id)
            .get_result::<Uuid>(&mut t.conn())
            .unwrap()
    };
    lead(&t, foot, abroad);

    let stood = walk(&t, t.ann, ann, (550.0, 550.0)).await;
    assert_eq!(stood["levelId"], t.ground.to_string());
    assert_eq!(where_is(&t, ann), (t.ground, 550.0, 550.0));

    let via = crate::interaction::load(&mut t.conn(), foot).unwrap().row;
    assert_eq!(
        travel(&mut t.conn(), ann, &via, t.gm).unwrap_err(),
        TravelError::NoDestination
    );

    // Its partner is gone entirely.
    lead(&t, foot, Uuid::now_v7());
    let via = crate::interaction::load(&mut t.conn(), foot).unwrap().row;
    assert_eq!(
        travel(&mut t.conn(), ann, &via, t.gm).unwrap_err(),
        TravelError::NoDestination
    );

    // A partner on the same level is allowed: that is a teleporter.
    let pad = region(&t, t.ground, (800.0, 800.0), "anyone");
    lead(&t, foot, pad);
    walk(&t, t.ann, ann, (100.0, 100.0)).await;
    let jumped = walk(&t, t.ann, ann, (550.0, 550.0)).await;
    assert_eq!(jumped["levelId"], t.ground.to_string());
    assert_eq!(where_is(&t, ann), (t.ground, 850.0, 850.0));
    let _ = loft;
}

#[tokio::test]
async fn a_game_masters_drag_never_travels() {
    let t = seat_a_table();
    let loft = t.level("Loft");
    stairs(&t, t.ground, loft, "anyone");
    let ann = t.token(t.ann, None, (100.0, 100.0));

    // `updateToken` is the Game Master arranging the board. Dropping a token
    // on the stairs puts it on the stairs.
    let response = t
        .ask(
            t.gm,
            format!(
                "mutation {{ updateToken(tokenId: \"{ann}\", input: {{ x: 550, y: 550 }}) \
                 {{ levelId }} }}"
            ),
        )
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(where_is(&t, ann), (t.ground, 550.0, 550.0));
}
