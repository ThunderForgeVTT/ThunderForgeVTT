//! Spec 084 T039: `rerollRoll` spends Heroic Inspiration on the maker's own
//! check, in one transaction, once — or refuses and writes nothing.

use std::sync::Arc;

use chrono::{Duration, Utc};
use serde_json::{Value, json};
use uuid::Uuid;

use super::*;
use crate::graphql::mutations_roll::{RollDiceInput, roll_dice_impl};
use crate::graphql::mutations_roll_check::roll_check_impl;
use crate::graphql::mutations_roll_check::tests::{
    StepRng, state_with_real_packs, world_with_actor,
};
use crate::models::RollRecord;
use crate::play_pause::{TriggerDetail, pause_world};
use crate::rolls::reroll::{NO_LONGER_ACTS, TOO_LATE};
use crate::rolls::visibility::{RollFacts, RollView, Viewer, view_of};
use crate::schema::{
    users, world_actor_permissions, world_actor_system_data, world_events, world_roll_records,
    world_system_settings,
};
use crate::test_support::{insert_test_user, insert_test_world_member};
use thunderforge_canvas_core::roll_facets::Advantage;

/// A 5e world, its GM, a player with Editor on the one actor, and the
/// actor holding Heroic Inspiration.
struct Table {
    state: Arc<AppState>,
    gm: Uuid,
    player: Uuid,
    world: Uuid,
    actor: Uuid,
}

fn a_table() -> Table {
    let state = state_with_real_packs();
    let (gm, world, actor) = world_with_actor(&state, "dnd5e");
    let mut conn = state.db_pool.get().unwrap();
    let player = insert_test_user(&mut conn);
    insert_test_world_member(&mut conn, world, player, "Player");
    let now = Utc::now().naive_utc();
    diesel::insert_into(world_actor_permissions::table)
        .values((
            world_actor_permissions::id.eq(Uuid::now_v7()),
            world_actor_permissions::actor_id.eq(actor),
            world_actor_permissions::user_id.eq(player),
            world_actor_permissions::level.eq("Editor"),
            world_actor_permissions::created_at.eq(now),
            world_actor_permissions::updated_at.eq(now),
        ))
        .execute(&mut conn)
        .unwrap();
    drop(conn);
    let table = Table {
        state: Arc::new(state),
        gm,
        player,
        world,
        actor,
    };
    table.inspire(true);
    table
}

impl Table {
    fn inspire(&self, inspired: bool) {
        let mut conn = self.state.db_pool.get().unwrap();
        diesel::update(
            world_actor_system_data::table.filter(world_actor_system_data::actor_id.eq(self.actor)),
        )
        .set(
            world_actor_system_data::trait_data
                .eq(Some(json!({ "level": 5, "inspiration": inspired }))),
        )
        .execute(&mut conn)
        .unwrap();
    }

    fn inspired(&self) -> Value {
        let mut conn = self.state.db_pool.get().unwrap();
        let traits = world_actor_system_data::table
            .filter(world_actor_system_data::actor_id.eq(self.actor))
            .select(world_actor_system_data::trait_data)
            .first::<Option<Value>>(&mut conn)
            .unwrap()
            .unwrap_or(Value::Null);
        traits["inspiration"].clone()
    }

    async fn check(&self, who: Uuid, advantage: Advantage) -> RollRecord {
        roll_check_impl(
            &self.state,
            who,
            false,
            self.world,
            self.actor,
            "stealth".to_string(),
            advantage,
            &mut StepRng(11),
        )
        .await
        .unwrap();
        self.rolls().pop().unwrap()
    }

    fn rolls(&self) -> Vec<RollRecord> {
        let mut conn = self.state.db_pool.get().unwrap();
        world_roll_records::table
            .filter(world_roll_records::world_id.eq(self.world))
            .order(world_roll_records::created_at.asc())
            .select(RollRecord::as_select())
            .load(&mut conn)
            .unwrap()
    }

    fn events(&self, code: i32) -> i64 {
        let mut conn = self.state.db_pool.get().unwrap();
        world_events::table
            .filter(world_events::world_id.eq(self.world))
            .filter(world_events::event_code.eq(code))
            .count()
            .get_result(&mut conn)
            .unwrap()
    }

    async fn reroll(&self, who: Uuid, roll: &RollRecord) -> GraphQLResult<WorldRoll> {
        self.reroll_at(who, roll, Utc::now()).await
    }

    async fn reroll_at(
        &self,
        who: Uuid,
        roll: &RollRecord,
        now: chrono::DateTime<Utc>,
    ) -> GraphQLResult<WorldRoll> {
        reroll_roll_impl(
            &self.state,
            who,
            false,
            RerollRequest {
                world_id: self.world,
                roll_id: roll.id,
                spend: "inspiration".to_string(),
            },
            &mut StepRng(977),
            now,
        )
        .await
    }

    /// Refused with `sentence`, with nothing spent and nothing recorded.
    async fn refused(&self, who: Uuid, roll: &RollRecord, sentence: &str) {
        let before = self.rolls().len();
        let error = self.reroll(who, roll).await.unwrap_err();
        assert_eq!(error.message, sentence);
        assert_eq!(self.rolls().len(), before, "no roll was recorded");
        assert_eq!(self.inspired(), json!(true), "nothing was spent");
    }
}

fn faces(roll: &RollRecord) -> Vec<i64> {
    roll.detail["dice"]
        .as_array()
        .unwrap()
        .iter()
        .map(|die| die["final_value"].as_i64().unwrap())
        .collect()
}

#[tokio::test]
async fn heroic_inspiration_rerolls_a_check_and_is_spent() {
    let table = a_table();
    let first = table.check(table.player, Advantage::Normal).await;
    let sheet_events = table.events(26);
    let roll_events = table.events(36);

    let shown = table.reroll(table.player, &first).await.unwrap();

    assert_eq!(table.inspired(), json!(false));
    let rolls = table.rolls();
    assert_eq!(rolls.len(), 2);
    let second = &rolls[1];
    assert_eq!(shown.id, second.id);
    assert_eq!(second.reroll_of, Some(first.id));
    assert_eq!(second.reroll_spent.as_deref(), Some("inspiration"));
    let mut facets = first.facet_ids();
    facets.push("inspiration".to_string());
    assert_eq!(second.facet_ids(), facets);
    assert_eq!(second.visibility, first.visibility);
    assert_eq!(second.label, first.label);
    assert_eq!(second.actor_id, first.actor_id);
    assert_eq!(second.check_id, first.check_id);
    assert_eq!(second.triggered_by, table.player);
    assert_eq!(faces(second).len(), faces(&first).len());
    assert_eq!(table.events(26), sheet_events + 1);
    assert_eq!(table.events(36), roll_events + 1);
    assert_eq!(
        shown.spent.map(|spent| spent.label),
        Some("Heroic Inspiration".to_string())
    );
}

#[tokio::test]
async fn with_advantage_only_the_lower_d20_is_rolled_again() {
    let table = a_table();
    let first = table.check(table.player, Advantage::Advantage).await;
    let before = faces(&first);
    assert_eq!(before.len(), 2, "two d20s were rolled");
    let lower = if before[1] < before[0] { 1 } else { 0 };

    table.reroll(table.player, &first).await.unwrap();

    let after = faces(&table.rolls()[1]);
    assert_eq!(after[1 - lower], before[1 - lower], "the higher die stays");
    assert!(table.rolls()[1].formula.contains("kh1"));
}

#[tokio::test]
async fn a_roll_is_rerolled_once_and_the_second_spend_is_kept() {
    let table = a_table();
    let first = table.check(table.player, Advantage::Normal).await;
    table.reroll(table.player, &first).await.unwrap();
    table.inspire(true);
    table
        .refused(table.player, &first, "This roll has already been rerolled.")
        .await;
    let second = table.rolls().pop().unwrap();
    table
        .refused(
            table.player,
            &second,
            "Heroic Inspiration has already been spent on this roll.",
        )
        .await;
}

#[tokio::test]
async fn only_the_maker_of_a_sheets_d20_test_may_reroll_it() {
    let table = a_table();
    let players_roll = table.check(table.player, Advantage::Normal).await;
    let other = {
        let mut conn = table.state.db_pool.get().unwrap();
        let other = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, table.world, other, "Player");
        other
    };
    table.refused(other, &players_roll, ONLY_MAKER).await;
    table.refused(table.gm, &players_roll, ONLY_MAKER).await;

    roll_dice_impl(
        &table.state,
        table.player,
        RollDiceInput {
            world_id: table.world,
            formula: "1d20".to_string(),
            bindings: None,
            visibility: None,
            label: None,
        },
        &mut StepRng(5),
    )
    .await
    .unwrap();
    let free = table.rolls().pop().unwrap();
    table.refused(table.player, &free, ONLY_MAKER).await;
}

#[tokio::test]
async fn a_damage_roll_is_not_rerolled() {
    let table = a_table();
    let check = table.check(table.player, Advantage::Normal).await;
    let damage = {
        let mut conn = table.state.db_pool.get().unwrap();
        let mut record = NewRollRecord::plain(
            table.world,
            table.player,
            "1d8".to_string(),
            check.detail.clone(),
            "total",
            4.0,
        );
        RollMeta {
            actor_id: Some(table.actor),
            roll_kind: Some(RollKind::Damage),
            ..RollMeta::default()
        }
        .write_to(&mut record);
        diesel::insert_into(world_roll_records::table)
            .values(&record)
            .returning(RollRecord::as_returning())
            .get_result::<RollRecord>(&mut conn)
            .unwrap()
    };
    table.refused(table.player, &damage, NOT_A_D20_TEST).await;
}

#[tokio::test]
async fn a_table_that_does_not_use_inspiration_cannot_spend_it() {
    let table = a_table();
    let first = table.check(table.player, Advantage::Normal).await;
    let mut conn = table.state.db_pool.get().unwrap();
    let now = Utc::now().naive_utc();
    diesel::insert_into(world_system_settings::table)
        .values((
            world_system_settings::world_id.eq(table.world),
            world_system_settings::system_id.eq("dnd5e"),
            world_system_settings::key.eq("inspiration"),
            world_system_settings::value.eq(json!(false)),
            world_system_settings::created_at.eq(now),
            world_system_settings::updated_at.eq(now),
        ))
        .execute(&mut conn)
        .unwrap();
    drop(conn);
    table
        .refused(
            table.player,
            &first,
            "This table does not use Heroic Inspiration.",
        )
        .await;
}

#[tokio::test]
async fn a_paused_world_rerolls_nothing() {
    let table = a_table();
    let first = table.check(table.player, Advantage::Normal).await;
    let mut conn = table.state.db_pool.get().unwrap();
    let operator = insert_test_user(&mut conn);
    diesel::update(users::table.filter(users::id.eq(operator)))
        .set(users::is_admin.eq(true))
        .execute(&mut conn)
        .unwrap();
    insert_test_world_member(&mut conn, table.world, operator, "Player");
    pause_world(
        &mut conn,
        operator,
        table.world,
        "Stopping play.",
        TriggerDetail::operator(),
    )
    .expect("paused");
    drop(conn);
    let before = table.rolls().len();
    assert!(table.reroll(table.player, &first).await.is_err());
    assert_eq!(table.rolls().len(), before);
    assert_eq!(table.inspired(), json!(true));
}

#[tokio::test]
async fn the_window_is_two_minutes() {
    let table = a_table();
    let first = table.check(table.player, Advantage::Normal).await;
    let late = first.created_at + Duration::seconds(121);
    let error = table
        .reroll_at(table.player, &first, late)
        .await
        .unwrap_err();
    assert_eq!(error.message, TOO_LATE);
    assert_eq!(table.inspired(), json!(true));
    let in_time = first.created_at + Duration::seconds(119);
    table
        .reroll_at(table.player, &first, in_time)
        .await
        .unwrap();
}

#[tokio::test]
async fn a_player_who_lost_the_sheet_cannot_spend_from_it() {
    let table = a_table();
    let first = table.check(table.player, Advantage::Normal).await;
    let mut conn = table.state.db_pool.get().unwrap();
    diesel::delete(
        world_actor_permissions::table.filter(world_actor_permissions::user_id.eq(table.player)),
    )
    .execute(&mut conn)
    .unwrap();
    drop(conn);
    table.refused(table.player, &first, NO_LONGER_ACTS).await;
}

#[tokio::test]
async fn a_gms_eyes_roll_rerolled_is_still_for_the_gms_eyes() {
    let table = a_table();
    let first = table.check(table.player, Advantage::Normal).await;
    let mut conn = table.state.db_pool.get().unwrap();
    diesel::update(world_roll_records::table.find(first.id))
        .set(world_roll_records::visibility.eq("gm_eyes"))
        .execute(&mut conn)
        .unwrap();
    drop(conn);

    table.reroll(table.player, &first).await.unwrap();

    let second = table.rolls().pop().unwrap();
    assert_eq!(second.visibility, "gm_eyes");
    let facts = RollFacts {
        roller: second.triggered_by,
        visibility: Visibility::parse(&second.visibility),
        revealed: false,
    };
    let another = Viewer {
        user_id: Uuid::now_v7(),
        is_gm: false,
        is_admin: false,
    };
    assert_eq!(view_of(facts, another), RollView::Masked);
}

/// SC-003: however many ask at once, one reroll lands and one spend is made.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_hundred_rerolls_at_once_make_one() {
    let table = Arc::new(a_table());
    let first = table.check(table.player, Advantage::Normal).await;
    let sheet_events = table.events(26);

    let calls: Vec<_> = (0..100)
        .map(|_| {
            let table = Arc::clone(&table);
            let first = first.clone();
            tokio::spawn(async move { table.reroll(table.player, &first).await.is_ok() })
        })
        .collect();
    let mut landed = 0;
    for call in calls {
        if call.await.unwrap() {
            landed += 1;
        }
    }

    assert_eq!(landed, 1);
    assert_eq!(table.rolls().len(), 2);
    assert_eq!(table.events(26), sheet_events + 1);
    assert_eq!(table.inspired(), json!(false));
}
