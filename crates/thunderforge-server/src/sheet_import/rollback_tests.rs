//! Spec 048 T074: which values a rollback keeps from the table.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::json;

use super::{ActorState, keep_play_state, restore_data};

fn state(data: serde_json::Value) -> ActorState {
    ActorState {
        label: "Wren".into(),
        system_data: serde_json::from_value::<BTreeMap<_, _>>(data).unwrap(),
        abilities: Vec::new(),
        inventory: Vec::new(),
    }
}

#[test]
fn play_state_keeps_now_and_the_sheet_goes_back() {
    let then = state(json!({
        "resource_data": { "max_hp": 30, "current_hp": 30, "hit_dice_pools": [{ "size": 10, "used": 0 }] },
        "spell_data": { "spell_slots_used": {} },
    }));
    let now = state(json!({
        "resource_data": {
            "max_hp": 38, "current_hp": 7,
            "hit_dice_pools": [{ "size": 10, "used": 2 }, { "size": 6, "used": 1 }]
        },
        "spell_data": { "spell_slots_used": { "level_1": 2 } },
        "trait_data": { "inspiration": true },
    }));
    let play: BTreeSet<String> = [
        "resource_data.current_hp",
        "resource_data.hit_dice_pools[].used",
        "spell_data.spell_slots_used",
        "trait_data.inspiration",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    let restored = restore_data(&now, &then, &play);
    assert_eq!(
        restored["resource_data"],
        Some(
            json!({ "max_hp": 30, "current_hp": 7, "hit_dice_pools": [{ "size": 10, "used": 2 }] })
        )
    );
    assert_eq!(
        restored["spell_data"],
        Some(json!({ "spell_slots_used": { "level_1": 2 } }))
    );
    assert_eq!(
        restored["trait_data"], None,
        "no sheet then, nothing to keep it in"
    );
    assert_eq!(restored["ability_data"], None);
}

#[test]
fn a_value_absent_now_is_absent_after() {
    let mut then = json!({ "current_hp": 30, "max_hp": 30 });
    keep_play_state(&mut then, &json!({ "max_hp": 38 }), &["current_hp"]);
    assert_eq!(then, json!({ "max_hp": 30 }));
    let mut nested = json!({ "pact_slots": { "max": 2, "used": 0 } });
    keep_play_state(
        &mut nested,
        &json!({ "pact_slots": { "max": 3, "used": 2 } }),
        &["pact_slots", "used"],
    );
    assert_eq!(nested, json!({ "pact_slots": { "max": 2, "used": 2 } }));
}
