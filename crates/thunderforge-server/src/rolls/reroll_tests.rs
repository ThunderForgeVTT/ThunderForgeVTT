//! Spec 084 (T037): the host's reroll rules, without a database.

use super::*;
use crate::rolls::facets::facets_for;
use serde_json::json;

fn roll(maker: Uuid, kind: &str) -> RollRecord {
    RollRecord {
        id: Uuid::now_v7(),
        world_id: Uuid::now_v7(),
        triggered_by: maker,
        formula: "1d20 + 3".to_string(),
        bindings: None,
        detail: json!({}),
        result_kind: "total".to_string(),
        result_value: 9.0,
        created_at: Utc::now(),
        outcome: None,
        visibility: "everyone".to_string(),
        label: Some("Dexterity".to_string()),
        revealed_at: None,
        revealed_by: None,
        actor_id: Some(Uuid::now_v7()),
        roll_kind: Some(kind.to_string()),
        check_id: Some("dexterity".to_string()),
        facets: Vec::new(),
        reroll_of: None,
        reroll_spent: None,
    }
}

fn standing<'a>(row: &'a RollRecord, spent: &'a [String]) -> Standing<'a> {
    Standing {
        row,
        rerolled_by: None,
        may_act: true,
        spent,
        hit: false,
    }
}

fn sheet(inspiration: bool) -> Sheet {
    Sheet {
        actor_name: "Pip".to_string(),
        slots: json!({ "trait_data": { "inspiration": inspiration } }),
        settings: json!({ "inspiration": true }),
    }
}

fn soon(row: &RollRecord) -> DateTime<Utc> {
    row.created_at + Duration::seconds(30)
}

#[test]
fn a_check_is_open_for_two_minutes_after_it_is_made() {
    let maker = Uuid::now_v7();
    let row = roll(maker, "check");
    assert_eq!(
        reroll_until(&row, None),
        Some(row.created_at + Duration::minutes(2))
    );
    assert_eq!(reroll_until(&row, Some(Uuid::now_v7())), None);
    assert_eq!(reroll_until(&roll(maker, "damage"), None), None);
    let typed = RollRecord {
        actor_id: None,
        ..roll(maker, "check")
    };
    assert_eq!(reroll_until(&typed, None), None);
}

#[test]
fn the_host_refuses_in_the_contracts_order() {
    let maker = Uuid::now_v7();
    let row = roll(maker, "check");
    let none: [String; 0] = [];
    let label = "Heroic Inspiration";
    let ask = |standing: &Standing<'_>, caller: Uuid, now: DateTime<Utc>| {
        may_reroll(caller, standing, "inspiration", label, now)
    };

    assert_eq!(
        ask(&standing(&row, &none), Uuid::now_v7(), soon(&row)),
        Err(ONLY_MAKER.to_string())
    );
    let barred = Standing {
        may_act: false,
        ..standing(&row, &none)
    };
    assert_eq!(
        ask(&barred, maker, soon(&row)),
        Err(NO_LONGER_ACTS.to_string())
    );
    let damage = roll(maker, "damage");
    assert_eq!(
        ask(&standing(&damage, &none), maker, soon(&damage)),
        Err(NOT_A_D20_TEST.to_string())
    );
    let replaced = Standing {
        rerolled_by: Some(Uuid::now_v7()),
        ..standing(&row, &none)
    };
    assert_eq!(
        ask(&replaced, maker, soon(&row)),
        Err(ALREADY_REROLLED.to_string())
    );
    let late = row.created_at + Duration::minutes(2) + Duration::seconds(1);
    assert_eq!(
        ask(&standing(&row, &none), maker, late),
        Err(TOO_LATE.to_string())
    );
    let spent = ["inspiration".to_string()];
    assert_eq!(
        ask(&standing(&row, &spent), maker, soon(&row)),
        Err("Heroic Inspiration has already been spent on this roll.".to_string())
    );
    let to_hit = roll(maker, "to_hit");
    let hit = Standing {
        hit: true,
        ..standing(&to_hit, &none)
    };
    assert_eq!(ask(&hit, maker, soon(&to_hit)), Err(A_HIT.to_string()));
    assert_eq!(
        ask(&standing(&row, &none), maker, soon(&row)),
        Ok(RollKind::Check)
    );
}

#[test]
fn only_the_maker_is_offered_what_their_sheet_can_pay() {
    let maker = Uuid::now_v7();
    let row = roll(maker, "check");
    let none: [String; 0] = [];
    let dnd5e = facets_for("dnd5e");
    let offered = offers_for(
        maker,
        &standing(&row, &none),
        dnd5e,
        &sheet(true),
        soon(&row),
    );
    assert_eq!(
        offered,
        vec![("inspiration".to_string(), "Heroic Inspiration".to_string())]
    );
    let stranger = offers_for(
        Uuid::now_v7(),
        &standing(&row, &none),
        dnd5e,
        &sheet(true),
        soon(&row),
    );
    assert!(stranger.is_empty());
    let empty = offers_for(
        maker,
        &standing(&row, &none),
        dnd5e,
        &sheet(false),
        soon(&row),
    );
    assert!(empty.is_empty());
    let other_system = offers_for(
        maker,
        &standing(&row, &none),
        None,
        &sheet(true),
        soon(&row),
    );
    assert!(other_system.is_empty());
}

#[test]
fn a_spend_is_shown_by_its_systems_name() {
    assert_eq!(spend_label("dnd5e", "inspiration"), "Heroic Inspiration");
    assert_eq!(spend_label("dnd5e", "nonesuch"), "nonesuch");
}

#[test]
fn a_chain_reports_every_spend_in_it() {
    let maker = Uuid::now_v7();
    let first = roll(maker, "check");
    let second = RollRecord {
        reroll_of: Some(first.id),
        reroll_spent: Some("inspiration".to_string()),
        ..roll(maker, "check")
    };
    assert_eq!(spent_in(&[second, first]), vec!["inspiration".to_string()]);
}

#[test]
fn the_window_closes_two_minutes_after_the_roll() {
    let maker = Uuid::now_v7();
    let row = roll(maker, "check");
    let none: [String; 0] = [];
    let at = |seconds: i64| row.created_at + Duration::seconds(seconds);
    let ask = |now| may_reroll(maker, &standing(&row, &none), "inspiration", "x", now);
    assert_eq!(ask(at(119)), Ok(RollKind::Check));
    assert_eq!(ask(at(121)), Err(TOO_LATE.to_string()));
}

#[test]
fn nothing_is_offered_on_a_replaced_roll_or_after_the_window() {
    let maker = Uuid::now_v7();
    let row = roll(maker, "check");
    let none: [String; 0] = [];
    let dnd5e = facets_for("dnd5e");
    let replaced = Standing {
        rerolled_by: Some(Uuid::now_v7()),
        ..standing(&row, &none)
    };
    assert!(offers_for(maker, &replaced, dnd5e, &sheet(true), soon(&row)).is_empty());
    let late = row.created_at + Duration::seconds(121);
    assert!(offers_for(maker, &standing(&row, &none), dnd5e, &sheet(true), late).is_empty());
}

#[test]
fn a_chain_is_walked_back_to_the_first_roll() {
    use crate::models::NewRollRecord;
    use crate::test_support::{insert_test_user, insert_test_world, test_app_state};

    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let user = insert_test_user(&mut conn);
    let world = insert_test_world(&mut conn, user);
    let mut insert = |reroll_of: Option<Uuid>, spent: Option<&str>| {
        diesel::insert_into(world_roll_records::table)
            .values(&NewRollRecord {
                reroll_of,
                reroll_spent: spent.map(str::to_string),
                ..NewRollRecord::plain(world, user, "1d20".to_string(), json!({}), "total", 4.0)
            })
            .returning(RollRecord::as_returning())
            .get_result::<RollRecord>(&mut conn)
            .unwrap()
    };
    let first = insert(None, None);
    let second = insert(Some(first.id), Some("inspiration"));
    let third = insert(Some(second.id), Some("luck_point"));

    let chain = chain_of(&mut conn, &third).unwrap();
    let ids: Vec<Uuid> = chain.iter().map(|roll| roll.id).collect();
    assert_eq!(ids, vec![third.id, second.id, first.id]);
    assert_eq!(spent_in(&chain), vec!["luck_point", "inspiration"]);
    assert_eq!(rerolled_by(&mut conn, first.id).unwrap(), Some(second.id));
    assert_eq!(rerolled_by(&mut conn, third.id).unwrap(), None);
}
