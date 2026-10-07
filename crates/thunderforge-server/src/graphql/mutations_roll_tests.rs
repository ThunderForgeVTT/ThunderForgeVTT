//! `rollDice` and `revealRoll` against a real Postgres.

use super::*;
use crate::test_support::{insert_test_user, insert_test_world, test_app_state};

/// Deterministic RNG for tests — `rand_core` 0.10 dropped its old
/// `mock::StepRng`, so this is a minimal always-increasing generator
/// (never actually treated as authoritative; only `rollDice`'s real
/// resolver method uses `rand::rng()`, research.md §3).
struct StepRng(u64);

impl StepRng {
    fn new(start: u64, _step: u64) -> Self {
        StepRng(start)
    }
}

impl rand::TryRng for StepRng {
    type Error = std::convert::Infallible;

    fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
        self.0 = self.0.wrapping_add(1);
        Ok(self.0 as u32)
    }
    fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
        Ok(self.try_next_u32()? as u64)
    }
    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Self::Error> {
        for b in dest.iter_mut() {
            *b = self.try_next_u32()? as u8;
        }
        Ok(())
    }
}

#[tokio::test]
async fn non_member_is_rejected_before_any_roll_happens() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    let outsider_id = insert_test_user(&mut conn);
    drop(conn);

    let mut rng = StepRng::new(0, 1);
    let result = roll_dice_impl(
        &state,
        outsider_id,
        RollDiceInput {
            world_id,
            formula: "1d20".to_string(),
            bindings: None,
            visibility: None,
            label: None,
        },
        &mut rng,
    )
    .await;

    assert!(result.is_err());

    let mut conn = state.db_pool.get().unwrap();
    let count: i64 = world_roll_records::table
        .filter(world_roll_records::world_id.eq(world_id))
        .count()
        .get_result(&mut conn)
        .unwrap();
    assert_eq!(
        count, 0,
        "no roll should have been recorded for a rejected caller"
    );
}

#[tokio::test]
async fn member_can_roll_and_a_record_is_persisted() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    drop(conn);

    let mut rng = StepRng::new(0, 1);
    let resolution = roll_dice_impl(
        &state,
        owner_id,
        RollDiceInput {
            world_id,
            formula: "1d20".to_string(),
            bindings: None,
            visibility: None,
            label: None,
        },
        &mut rng,
    )
    .await
    .expect("a world member should be able to roll");

    assert_eq!(resolution.dice.len(), 1);

    let mut conn = state.db_pool.get().unwrap();
    let count: i64 = world_roll_records::table
        .filter(world_roll_records::world_id.eq(world_id))
        .count()
        .get_result(&mut conn)
        .unwrap();
    assert_eq!(count, 1);
}

fn one_d20(world_id: Uuid) -> RollDiceInput {
    RollDiceInput {
        world_id,
        formula: "1d20".to_string(),
        bindings: None,
        visibility: None,
        label: None,
    }
}

/// Spec 067 FR-032: what the settle step says is stored with the roll and
/// answered with it, and what else it returns comes back to the caller.
#[tokio::test]
async fn a_settled_roll_stores_its_outcome_with_the_record() {
    use thunderforge_canvas_core::system_contribution::Verdict;

    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    drop(conn);

    let (resolution, seen) = roll_and_settle(
        &state,
        owner_id,
        one_d20(world_id),
        &mut StepRng::new(0, 1),
        |_, resolution| {
            Ok((
                Some(RollOutcome {
                    verdict: Verdict::CriticalSuccess,
                    label: "Nailed it".to_string(),
                }),
                roll_value(resolution),
            ))
        },
    )
    .await
    .expect("the roll settles");

    let outcome = resolution.outcome.expect("the outcome is answered");
    assert_eq!(
        outcome.verdict,
        crate::graphql::types::RollVerdict::CriticalSuccess
    );
    assert_eq!(outcome.label, "Nailed it");
    assert_eq!(seen, resolution.result_value, "settle saw the real roll");

    let mut conn = state.db_pool.get().unwrap();
    let stored: Option<serde_json::Value> = world_roll_records::table
        .filter(world_roll_records::world_id.eq(world_id))
        .select(world_roll_records::outcome)
        .first(&mut conn)
        .unwrap();
    assert_eq!(
        stored,
        Some(serde_json::json!({ "verdict": "critical_success", "label": "Nailed it" }))
    );
}

/// A plain `rollDice` is judged by nobody, and says so with a null rather
/// than a made-up verdict.
#[tokio::test]
async fn a_plain_roll_stores_no_outcome() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    drop(conn);

    let resolution = roll_dice_impl(&state, owner_id, one_d20(world_id), &mut StepRng::new(0, 1))
        .await
        .expect("a member rolls");
    assert!(resolution.outcome.is_none());

    let mut conn = state.db_pool.get().unwrap();
    let stored: Option<serde_json::Value> = world_roll_records::table
        .filter(world_roll_records::world_id.eq(world_id))
        .select(world_roll_records::outcome)
        .first(&mut conn)
        .unwrap();
    assert_eq!(stored, None);
}

/// The record and what follows from it land together: a settle step that
/// refuses leaves no roll behind, and takes its own writes with it.
#[tokio::test]
async fn a_refused_settle_leaves_no_record_and_undoes_its_own_writes() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    drop(conn);

    let refused = roll_and_settle(
        &state,
        owner_id,
        one_d20(world_id),
        &mut StepRng::new(0, 1),
        move |conn, _| -> Result<(Option<RollOutcome>, ()), String> {
            diesel::update(crate::schema::worlds::table.find(world_id))
                .set(crate::schema::worlds::name.eq("Written and then refused"))
                .execute(conn)
                .map_err(|e| e.to_string())?;
            Err("Not today".to_string())
        },
    )
    .await
    .expect_err("the settle step refused");
    assert_eq!(refused.message, "Not today");

    let mut conn = state.db_pool.get().unwrap();
    let count: i64 = world_roll_records::table
        .filter(world_roll_records::world_id.eq(world_id))
        .count()
        .get_result(&mut conn)
        .unwrap();
    assert_eq!(count, 0);
    let name: String = crate::schema::worlds::table
        .find(world_id)
        .select(crate::schema::worlds::name)
        .first(&mut conn)
        .unwrap();
    assert_ne!(name, "Written and then refused");
}

#[tokio::test]
async fn malformed_formula_produces_zero_rows() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    drop(conn);

    let mut rng = StepRng::new(0, 1);
    let result = roll_dice_impl(
        &state,
        owner_id,
        RollDiceInput {
            world_id,
            formula: "1d20 +".to_string(),
            bindings: None,
            visibility: None,
            label: None,
        },
        &mut rng,
    )
    .await;
    assert!(result.is_err());

    let mut conn = state.db_pool.get().unwrap();
    let count: i64 = world_roll_records::table
        .filter(world_roll_records::world_id.eq(world_id))
        .count()
        .get_result(&mut conn)
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn two_rolls_from_different_users_are_independent_records() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    let other_id = insert_test_user(&mut conn);
    crate::test_support::insert_test_world_member(&mut conn, world_id, other_id, "Player");
    drop(conn);

    let mut rng_a = StepRng::new(0, 1);
    roll_dice_impl(
        &state,
        owner_id,
        RollDiceInput {
            world_id,
            formula: "1d20".to_string(),
            bindings: None,
            visibility: None,
            label: None,
        },
        &mut rng_a,
    )
    .await
    .unwrap();

    let mut rng_b = StepRng::new(0, 1);
    roll_dice_impl(
        &state,
        other_id,
        RollDiceInput {
            world_id,
            formula: "1d20".to_string(),
            bindings: None,
            visibility: None,
            label: None,
        },
        &mut rng_b,
    )
    .await
    .unwrap();

    let mut conn = state.db_pool.get().unwrap();
    let records: Vec<crate::models::RollRecord> = world_roll_records::table
        .filter(world_roll_records::world_id.eq(world_id))
        .load(&mut conn)
        .unwrap();
    assert_eq!(records.len(), 2);
    assert_ne!(records[0].triggered_by, records[1].triggered_by);
}

/// US3 (T021): `RollDiceInput.bindings` correctly maps into
/// `resolve()`'s `PlaceholderBindings`, and a missing placeholder
/// surfaces as a specific, distinguishable error message rather than
/// a generic failure.
#[tokio::test]
async fn placeholder_bindings_flow_through_and_missing_ones_are_specific_errors() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    drop(conn);

    let mut rng = StepRng::new(0, 1);
    let low = roll_dice_impl(
        &state,
        owner_id,
        RollDiceInput {
            world_id,
            formula: "1d20 + STAT".to_string(),
            bindings: Some(vec![PlaceholderBindingInput {
                name: "STAT".to_string(),
                value: 3.0,
            }]),
            visibility: None,
            label: None,
        },
        &mut rng,
    )
    .await
    .unwrap();

    let mut rng = StepRng::new(0, 1);
    let high = roll_dice_impl(
        &state,
        owner_id,
        RollDiceInput {
            world_id,
            formula: "1d20 + STAT".to_string(),
            bindings: Some(vec![PlaceholderBindingInput {
                name: "STAT".to_string(),
                value: 8.0,
            }]),
            visibility: None,
            label: None,
        },
        &mut rng,
    )
    .await
    .unwrap();

    assert_eq!(high.result_value - low.result_value, 5.0);

    let mut rng = StepRng::new(0, 1);
    let err = roll_dice_impl(
        &state,
        owner_id,
        RollDiceInput {
            world_id,
            formula: "1d20 + STAT".to_string(),
            bindings: None,
            visibility: None,
            label: None,
        },
        &mut rng,
    )
    .await
    .unwrap_err();
    assert!(
        err.message.contains("STAT"),
        "error should name the missing placeholder: {}",
        err.message
    );
}

/// US3 (T022, SC-005): a spec 013 Item Effect-style formula (attack
/// roll with a stat + flat modifiers placeholder) resolves through
/// `rollDice` with no schema changes needed on either side.
#[tokio::test]
async fn spec_013_item_effect_formula_resolves_unchanged() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    drop(conn);

    // A "Longsword" damage effect's stored formula (spec 013).
    let mut rng = StepRng::new(0, 1);
    let damage = roll_dice_impl(
        &state,
        owner_id,
        RollDiceInput {
            world_id,
            formula: "2d8".to_string(),
            bindings: None,
            visibility: None,
            label: None,
        },
        &mut rng,
    )
    .await
    .unwrap();
    assert_eq!(damage.dice.len(), 2);

    // An attack-roll effect's stored formula.
    let mut rng = StepRng::new(0, 1);
    let attack = roll_dice_impl(
        &state,
        owner_id,
        RollDiceInput {
            world_id,
            formula: "1d20 + STAT + MODIFIERS".to_string(),
            bindings: Some(vec![
                PlaceholderBindingInput {
                    name: "STAT".to_string(),
                    value: 3.0,
                },
                PlaceholderBindingInput {
                    name: "MODIFIERS".to_string(),
                    value: 2.0,
                },
            ]),
            visibility: None,
            label: None,
        },
        &mut rng,
    )
    .await
    .unwrap();
    assert_eq!(attack.dice.len(), 1);
}

// ------------------------------------------------------------------
// Spec 081 T009 and T033: a roll is announced, and the GM can show it.
// ------------------------------------------------------------------

use crate::schema::world_events;
use crate::test_support::insert_test_world_member;

/// The roll events this world recorded, as `(code, payload)`.
fn roll_events(state: &AppState, world_id: Uuid) -> Vec<(i32, serde_json::Value)> {
    let mut conn = state.db_pool.get().unwrap();
    world_events::table
        .filter(world_events::world_id.eq(world_id))
        .filter(world_events::event_code.eq_any([EVENT_CODE_ROLL_MADE, EVENT_CODE_ROLL_REVEALED]))
        .order(world_events::id.asc())
        .select((world_events::event_code, world_events::token_event))
        .load::<(i32, Option<serde_json::Value>)>(&mut conn)
        .unwrap()
        .into_iter()
        .map(|(code, payload)| (code, payload.unwrap_or_default()))
        .collect()
}

fn newest_roll(state: &AppState, world_id: Uuid) -> RollRecord {
    let mut conn = state.db_pool.get().unwrap();
    world_roll_records::table
        .filter(world_roll_records::world_id.eq(world_id))
        .order(world_roll_records::created_at.desc())
        .select(RollRecord::as_select())
        .first(&mut conn)
        .unwrap()
}

fn roll_count(state: &AppState, world_id: Uuid) -> i64 {
    let mut conn = state.db_pool.get().unwrap();
    world_roll_records::table
        .filter(world_roll_records::world_id.eq(world_id))
        .count()
        .get_result(&mut conn)
        .unwrap()
}

/// A GM and a player at one table.
fn gm_and_player(state: &AppState) -> (Uuid, Uuid, Uuid) {
    let mut conn = state.db_pool.get().unwrap();
    let gm = insert_test_user(&mut conn);
    let player = insert_test_user(&mut conn);
    let world = insert_test_world(&mut conn, gm);
    insert_test_world_member(&mut conn, world, player, "Player");
    (world, gm, player)
}

async fn roll_as(
    state: &AppState,
    who: Uuid,
    world_id: Uuid,
    visibility: Option<RollVisibility>,
    label: Option<&str>,
) -> GraphQLResult<GraphQLRollResolution> {
    let mut rng = StepRng::new(0, 1);
    roll_dice_impl(
        state,
        who,
        RollDiceInput {
            world_id,
            formula: "1d20".to_string(),
            bindings: None,
            visibility,
            label: label.map(str::to_string),
        },
        &mut rng,
    )
    .await
}

#[tokio::test]
async fn a_roll_is_announced_once_with_its_id_and_visibility_only() {
    let state = test_app_state();
    let (world, _gm, player) = gm_and_player(&state);

    roll_as(&state, player, world, None, Some("  Stealth  "))
        .await
        .unwrap();

    let row = newest_roll(&state, world);
    assert_eq!(row.visibility, "everyone", "no visibility means everyone");
    assert_eq!(
        row.label.as_deref(),
        Some("Stealth"),
        "the label is trimmed"
    );

    let events = roll_events(&state, world);
    assert_eq!(events.len(), 1);
    let (code, payload) = &events[0];
    assert_eq!(*code, EVENT_CODE_ROLL_MADE);
    assert_eq!(
        payload,
        &serde_json::json!({ "rollId": row.id, "visibility": "everyone" }),
        "the event names the roll and nothing of its dice (FR-002)"
    );
}

#[tokio::test]
async fn a_refused_roll_records_no_row_and_no_event() {
    let state = test_app_state();
    let (world, _gm, player) = gm_and_player(&state);

    // A player may not hide a roll from the GM.
    assert!(
        roll_as(&state, player, world, Some(RollVisibility::GmOnly), None)
            .await
            .is_err()
    );
    // A label past the limit.
    let long = "x".repeat(MAX_ROLL_LABEL + 1);
    assert!(
        roll_as(&state, player, world, None, Some(&long))
            .await
            .is_err()
    );
    // A formula that does not parse.
    let mut rng = StepRng::new(0, 1);
    assert!(
        roll_dice_impl(
            &state,
            player,
            RollDiceInput {
                world_id: world,
                formula: "1d".to_string(),
                bindings: None,
                visibility: None,
                label: None,
            },
            &mut rng,
        )
        .await
        .is_err()
    );

    assert_eq!(roll_count(&state, world), 0);
    assert!(roll_events(&state, world).is_empty());
}

#[tokio::test]
async fn the_gm_reveals_a_hidden_roll_once_and_it_is_unchanged() {
    let state = test_app_state();
    let (world, gm, player) = gm_and_player(&state);

    roll_as(&state, player, world, Some(RollVisibility::GmEyes), None)
        .await
        .unwrap();
    let before = newest_roll(&state, world);

    // A player may not show it.
    assert!(
        reveal_roll_impl(&state, player, false, world, before.id)
            .await
            .is_err()
    );

    let shown = reveal_roll_impl(&state, gm, false, world, before.id)
        .await
        .unwrap();
    assert!(shown.revealed_at.is_some());
    assert!(shown.revealed_by_name.is_some());

    let after = newest_roll(&state, world);
    assert_eq!(after.revealed_by, Some(gm));
    assert_eq!(after.detail, before.detail, "the dice never change");
    assert_eq!(after.result_value, before.result_value);
    assert_eq!(after.created_at, before.created_at);
    assert_eq!(
        after.visibility, "gm_eyes",
        "and it keeps how it was rolled"
    );

    // Shown twice is shown once.
    reveal_roll_impl(&state, gm, false, world, before.id)
        .await
        .unwrap();
    let events = roll_events(&state, world);
    assert_eq!(
        events,
        vec![
            (
                EVENT_CODE_ROLL_MADE,
                serde_json::json!({ "rollId": before.id, "visibility": "gm_eyes" })
            ),
            (
                EVENT_CODE_ROLL_REVEALED,
                serde_json::json!({ "rollId": before.id, "visibility": "gm_eyes" })
            ),
        ]
    );
}

#[tokio::test]
async fn an_admin_reveals_and_an_open_roll_has_nothing_to_reveal() {
    let state = test_app_state();
    let (world, gm, player) = gm_and_player(&state);
    let admin = {
        let mut conn = state.db_pool.get().unwrap();
        insert_test_user(&mut conn)
    };

    roll_as(&state, gm, world, Some(RollVisibility::GmOnly), None)
        .await
        .unwrap();
    let hidden = newest_roll(&state, world);
    reveal_roll_impl(&state, admin, true, world, hidden.id)
        .await
        .unwrap();
    assert_eq!(newest_roll(&state, world).revealed_by, Some(admin));

    roll_as(&state, player, world, None, None).await.unwrap();
    let open = newest_roll(&state, world);
    reveal_roll_impl(&state, gm, false, world, open.id)
        .await
        .unwrap();
    assert!(newest_roll(&state, world).revealed_at.is_none());

    let revealed = roll_events(&state, world)
        .into_iter()
        .filter(|(code, _)| *code == EVENT_CODE_ROLL_REVEALED)
        .count();
    assert_eq!(revealed, 1, "only the hidden roll was revealed");
}
