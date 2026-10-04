//! The skill roll against a real Postgres.
//!
//! The verdict's truth table is pure and is tested beside `adjudicate`. What
//! needs a database is that the server reads the level and the target rather
//! than being told them, that a failure's experience and its record land
//! together, and who is refused before a die exists.
//!
//! No test here depends on what the dice show: an opposition of nought cannot
//! be failed and one of a hundred cannot be beaten.

use diesel::prelude::*;
use rand::SeedableRng;
use thunderforge_canvas_core::system_contribution::contribution_for;
use thunderforge_server::graphql::types::RollVerdict;
use thunderforge_server::schema::{world_actor_system_data, world_roll_records, worlds};
use thunderforge_server::state::AppState;
use thunderforge_server::test_support::{
    insert_test_actor, insert_test_scene, insert_test_user, insert_test_world,
    insert_test_world_member, test_app_state,
};
use uuid::Uuid;

use super::graphql::{roll_skill_impl, RollForShoesRollSkillInput};
use crate::settings::{upsert, UpsertSettings};
use crate::table::{set, SetDifficulty};
use crate::SYSTEM_ID;

const UNBEATABLE: f64 = 100.0;
const UNFAILABLE: f64 = 0.0;

fn seeded() -> rand::rngs::StdRng {
    rand::rngs::StdRng::seed_from_u64(67)
}

struct Table {
    state: AppState,
    world_id: Uuid,
    gm_id: Uuid,
    player_id: Uuid,
    actor_id: Uuid,
}

/// A Roll for Shoes world, its Game Master, a player who has joined, and one
/// of the Game Master's characters with nothing stored on its sheet.
fn a_table() -> Table {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("a test database connection");
    let gm_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, gm_id);
    diesel::update(worlds::table.find(world_id))
        .set(worlds::game_system_id.eq(SYSTEM_ID))
        .execute(&mut conn)
        .expect("the world plays Roll for Shoes");
    let player_id = insert_test_user(&mut conn);
    insert_test_world_member(&mut conn, world_id, player_id, "Player");
    let scene_id = insert_test_scene(&mut conn, world_id, gm_id);
    let actor_id = insert_test_actor(&mut conn, world_id, scene_id, gm_id);
    drop(conn);
    Table {
        state,
        world_id,
        gm_id,
        player_id,
        actor_id,
    }
}

impl Table {
    fn conn(
        &self,
    ) -> diesel::r2d2::PooledConnection<diesel::r2d2::ConnectionManager<PgConnection>> {
        self.state
            .db_pool
            .get()
            .expect("a test database connection")
    }

    fn roll(&self, skill_id: &str, opposition: Option<f64>) -> RollForShoesRollSkillInput {
        RollForShoesRollSkillInput {
            world_id: self.world_id,
            actor_id: self.actor_id,
            skill_id: skill_id.to_string(),
            opposition,
        }
    }

    fn store(&self, trait_data: serde_json::Value, xp: i64) {
        diesel::insert_into(world_actor_system_data::table)
            .values((
                world_actor_system_data::actor_id.eq(self.actor_id),
                world_actor_system_data::game_system_id.eq(SYSTEM_ID),
                world_actor_system_data::trait_data.eq(trait_data),
                world_actor_system_data::resource_data.eq(serde_json::json!({ "xp": xp })),
                world_actor_system_data::created_by.eq(self.gm_id),
                world_actor_system_data::updated_by.eq(self.gm_id),
            ))
            .execute(&mut self.conn())
            .expect("the sheet stores");
    }

    fn settings(&self, tie_succeeds: bool, statuses_enabled: bool) {
        upsert(
            &mut self.conn(),
            UpsertSettings {
                world_id: self.world_id,
                difficulty_mode: "free".to_string(),
                tie_succeeds,
                statuses_enabled,
                skill_slots_enabled: false,
                starting_skills: serde_json::json!([]),
                updated_by: None,
            },
        )
        .expect("the settings store");
    }

    /// Every roll recorded in this world, as the outcome stored with it.
    fn recorded_outcomes(&self) -> Vec<Option<serde_json::Value>> {
        world_roll_records::table
            .filter(world_roll_records::world_id.eq(self.world_id))
            .order(world_roll_records::created_at.asc())
            .select(world_roll_records::outcome)
            .load(&mut self.conn())
            .expect("the records read")
    }

    fn stored_xp(&self) -> Option<i64> {
        world_actor_system_data::table
            .filter(world_actor_system_data::actor_id.eq(self.actor_id))
            .select(world_actor_system_data::resource_data)
            .first::<Option<serde_json::Value>>(&mut self.conn())
            .optional()
            .expect("the sheet reads")
            .flatten()
            .and_then(|data| data.get("xp").and_then(serde_json::Value::as_i64))
    }
}

#[test]
fn the_pack_registers_its_adjudicator() {
    let pack = contribution_for(SYSTEM_ID).expect("the pack is registered");
    assert!(pack.adjudicate.is_some());
}

#[tokio::test]
async fn a_failure_is_paid_its_experience_and_recorded_with_the_roll() {
    let table = a_table();

    let rolled = roll_skill_impl(
        &table.state,
        table.gm_id,
        false,
        table.roll("starting-skill", Some(UNBEATABLE)),
        &mut seeded(),
    )
    .await
    .expect("the roll is made");

    let outcome = rolled.roll.outcome.expect("an opposed roll is judged");
    assert_eq!(outcome.verdict, RollVerdict::Failure);
    assert_eq!(outcome.label, "Failure");
    assert_eq!(rolled.opposition, Some(UNBEATABLE));
    assert_eq!(rolled.xp_awarded, 1);
    assert_eq!(rolled.xp, 1);
    // A character nobody had saved: the experience made their row.
    assert_eq!(table.stored_xp(), Some(1));
    assert_eq!(
        table.recorded_outcomes(),
        vec![Some(
            serde_json::json!({ "verdict": "failure", "label": "Failure" })
        )]
    );

    // A second failure adds to what is stored rather than replacing it.
    let again = roll_skill_impl(
        &table.state,
        table.gm_id,
        false,
        table.roll("starting-skill", Some(UNBEATABLE)),
        &mut seeded(),
    )
    .await
    .expect("the roll is made");
    assert_eq!(again.xp, 2);
    assert_eq!(table.stored_xp(), Some(2));
}

#[tokio::test]
async fn a_success_pays_nothing_and_leaves_the_rest_of_the_sheet_alone() {
    let table = a_table();
    table.store(
        serde_json::json!({
            "skills": [{ "id": "climb", "name": "Climb", "level": 3, "parentId": null }]
        }),
        4,
    );

    let rolled = roll_skill_impl(
        &table.state,
        table.gm_id,
        false,
        table.roll("climb", Some(UNFAILABLE)),
        &mut seeded(),
    )
    .await
    .expect("the roll is made");

    assert_eq!(
        rolled.roll.outcome.expect("judged").verdict,
        RollVerdict::Success
    );
    assert_eq!(rolled.xp_awarded, 0);
    assert_eq!(rolled.xp, 4);
    assert_eq!(table.stored_xp(), Some(4));
    // The pool is the level the server read: three dice, and the caller had
    // no way to ask for more.
    assert_eq!(rolled.roll.dice.len(), 3);
    assert_eq!(rolled.roll.formula, "3d6");
}

#[tokio::test]
async fn with_nothing_to_beat_the_roll_is_recorded_unjudged() {
    let table = a_table();

    let rolled = roll_skill_impl(
        &table.state,
        table.gm_id,
        false,
        table.roll("starting-skill", None),
        &mut seeded(),
    )
    .await
    .expect("the roll is made");

    assert!(rolled.roll.outcome.is_none());
    assert_eq!(rolled.opposition, None);
    assert_eq!(rolled.xp_awarded, 0);
    assert_eq!(table.recorded_outcomes(), vec![None]);
    assert_eq!(table.stored_xp(), None, "nothing was written to the sheet");
}

#[tokio::test]
async fn the_game_masters_difficulty_wins_over_what_the_player_sends() {
    let table = a_table();
    set(
        &mut table.conn(),
        SetDifficulty {
            world_id: table.world_id,
            target: 100,
            band: None,
            gm_dice: None,
            set_by: Some(table.gm_id),
        },
    )
    .expect("the difficulty stores");

    let rolled = roll_skill_impl(
        &table.state,
        table.gm_id,
        false,
        table.roll("starting-skill", Some(UNFAILABLE)),
        &mut seeded(),
    )
    .await
    .expect("the roll is made");

    assert_eq!(rolled.opposition, Some(100.0));
    assert_eq!(
        rolled.roll.outcome.expect("judged").verdict,
        RollVerdict::Failure
    );
}

#[tokio::test]
async fn statuses_count_only_in_a_world_that_has_turned_them_on() {
    let table = a_table();
    table.store(
        serde_json::json!({
            "statuses": [{ "id": "s", "name": "Doomed", "modifier": -50 }]
        }),
        0,
    );

    let ignored = roll_skill_impl(
        &table.state,
        table.gm_id,
        false,
        table.roll("starting-skill", Some(UNFAILABLE)),
        &mut seeded(),
    )
    .await
    .expect("the roll is made");
    assert_eq!(ignored.modifier, 0);
    assert_eq!(
        ignored.roll.outcome.expect("judged").verdict,
        RollVerdict::Success
    );

    table.settings(false, true);
    let counted = roll_skill_impl(
        &table.state,
        table.gm_id,
        false,
        table.roll("starting-skill", Some(UNFAILABLE)),
        &mut seeded(),
    )
    .await
    .expect("the roll is made");
    assert_eq!(counted.modifier, -50);
    assert_eq!(counted.total, counted.roll.result_value - 50.0);
    assert_eq!(
        counted.roll.outcome.expect("judged").verdict,
        RollVerdict::Failure
    );
    assert_eq!(counted.xp, 1);
}

#[tokio::test]
async fn a_skill_the_character_does_not_have_is_refused_before_any_die() {
    let table = a_table();

    let refused = roll_skill_impl(
        &table.state,
        table.gm_id,
        false,
        table.roll("made-up", Some(UNBEATABLE)),
        &mut seeded(),
    )
    .await
    .expect_err("no such skill");

    assert_eq!(refused.message, "This character has no such skill");
    assert!(table.recorded_outcomes().is_empty(), "nothing was rolled");
    assert_eq!(table.stored_xp(), None);
}

#[tokio::test]
async fn a_player_cannot_roll_a_character_they_may_not_edit() {
    let table = a_table();

    roll_skill_impl(
        &table.state,
        table.player_id,
        false,
        table.roll("starting-skill", Some(UNBEATABLE)),
        &mut seeded(),
    )
    .await
    .expect_err("the character is not theirs");

    assert!(table.recorded_outcomes().is_empty(), "nothing was rolled");
    assert_eq!(table.stored_xp(), None, "and nothing was paid");
}

#[tokio::test]
async fn a_world_playing_something_else_is_refused() {
    let table = a_table();
    diesel::update(worlds::table.find(table.world_id))
        .set(worlds::game_system_id.eq("dnd5e"))
        .execute(&mut table.conn())
        .expect("the world changes system");

    let refused = roll_skill_impl(
        &table.state,
        table.gm_id,
        false,
        table.roll("starting-skill", Some(UNBEATABLE)),
        &mut seeded(),
    )
    .await
    .expect_err("not this game");

    assert_eq!(refused.message, "This world is not playing Roll for Shoes");
    assert!(table.recorded_outcomes().is_empty());
}
