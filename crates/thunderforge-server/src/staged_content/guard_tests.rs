//! Spec 048 T056: every play path that can name a piece the world has not
//! adopted refuses it with `CONTENT_NOT_ADOPTED` and the FR-036a sentence, and
//! writes nothing. Someone who could not see the piece gets the path's
//! ordinary answer, so the refusal reveals nothing.
//!
//! `rollCheck` is not here: it takes a check the system declared, never an
//! ability or item, so it has no way to name a piece (see the module doc).

use async_graphql::Value as GqlValue;
use diesel::prelude::*;
use serde_json::json;
use uuid::Uuid;

use super::{NOT_ADOPTED, NOT_ADOPTED_CODE};
use crate::combat::attack::FightRefusal;
use crate::combat::weapon::find_weapon;
use crate::graphql::mutations_ability_shares::create_ability_share_link_impl;
use crate::graphql::mutations_inventory::{
    AdjustInventoryQuantityInput, adjust_inventory_quantity_impl,
};
use crate::graphql::mutations_item_shares::create_item_share_link_impl;
use crate::graphql::mutations_staged_content::tests::world;
use crate::publishing::AttestationInput;
use crate::staged_content::StagedState;
use crate::staged_content::decide::{adopt, decline};

fn code(error: &async_graphql::Error) -> Option<String> {
    match error.extensions.as_ref()?.get("code")? {
        GqlValue::String(code) => Some(code.clone()),
        _ => None,
    }
}

fn assert_not_adopted(error: &async_graphql::Error) {
    assert_eq!(code(error).as_deref(), Some(NOT_ADOPTED_CODE));
    assert_eq!(error.message, NOT_ADOPTED);
}

/// `Weapon` is not `Debug`, so `expect_err` cannot be used on it.
fn refused(found: Result<crate::combat::weapon::Weapon, FightRefusal>, why: &str) -> FightRefusal {
    match found {
        Ok(_) => panic!("{why}"),
        Err(refusal) => refusal,
    }
}

fn terms() -> AttestationInput {
    AttestationInput {
        terms_version_id: "any".into(),
    }
}

#[test]
fn an_attack_with_a_staged_ability_or_item_is_refused_and_says_why() {
    let w = world();
    let actor = w.actor();
    let spell = w.stage(w.player, "spell", "Ember Lance", json!({ "level": 1 }));
    w.link_ability(actor, spell, "Ember Lance");
    let sword = w.stage(w.player, "item", "Moonblade", json!({}));
    w.link_item(actor, sword, "Moonblade");

    for (ability, item) in [(Some(spell), None), (None, Some(sword))] {
        let refusal = refused(
            find_weapon(&mut w.conn(), w.world, Some(actor), false, ability, item),
            "a staged piece is not a weapon",
        );
        let FightRefusal::NotAdopted(piece) = &refusal else {
            panic!("expected NotAdopted, got {}", refusal.message());
        };
        assert_eq!(piece.actor_id, Some(actor));
        assert_eq!(piece.world_id, w.world);
        assert_not_adopted(&refusal.into());
    }

    // The Game Master runs the world, so is told too.
    let gm = refused(
        find_weapon(&mut w.conn(), w.world, None, true, Some(spell), None),
        "still not adopted",
    );
    assert!(matches!(gm, FightRefusal::NotAdopted(_)));
}

#[test]
fn a_declined_piece_is_refused_and_an_adopted_one_is_the_worlds() {
    let w = world();
    let actor = w.actor();
    let declined = w.stage(w.player, "spell", "Ember Lance", json!({ "level": 1 }));
    w.link_ability(actor, declined, "Ember Lance");
    decline(&mut w.conn(), w.gm, declined).expect("declined");
    let refusal = refused(
        find_weapon(
            &mut w.conn(),
            w.world,
            Some(actor),
            false,
            Some(declined),
            None,
        ),
        "declined is not usable",
    );
    assert!(matches!(refusal, FightRefusal::NotAdopted(_)));

    let adopted = w.stage(w.player, "item", "Moonblade", json!({}));
    w.link_item(actor, adopted, "Moonblade");
    let piece = adopt(&mut w.conn(), w.gm, adopted).expect("adopted");
    assert_eq!(piece.state, StagedState::Adopted);
    // The staged id names nothing in play now; the world's id does.
    let by_staged = refused(
        find_weapon(
            &mut w.conn(),
            w.world,
            Some(actor),
            false,
            None,
            Some(adopted),
        ),
        "the staged id is spent",
    );
    assert!(matches!(by_staged, FightRefusal::NotFound(_)));
    assert!(
        find_weapon(
            &mut w.conn(),
            w.world,
            Some(actor),
            false,
            None,
            piece.adopted_item_id,
        )
        .is_ok(),
        "the world's item is a weapon"
    );
}

#[test]
fn someone_who_does_not_hold_the_piece_is_not_told_it_exists() {
    let w = world();
    let holder = w.actor();
    let other = w.actor();
    let spell = w.stage(w.player, "spell", "Ember Lance", json!({ "level": 1 }));
    w.link_ability(holder, spell, "Ember Lance");

    let refusal = refused(
        find_weapon(
            &mut w.conn(),
            w.world,
            Some(other),
            false,
            Some(spell),
            None,
        ),
        "not this creature's",
    );
    assert!(matches!(refusal, FightRefusal::NotFound(_)));
}

#[tokio::test]
async fn spending_a_staged_item_is_refused_and_the_quantity_stays() {
    let w = world();
    let actor = w.actor();
    let potion = w.stage(w.player, "item", "Potion of Embers", json!({}));
    let entry = w.link_item(actor, potion, "Potion of Embers");

    let error = adjust_inventory_quantity_impl(
        &w.state,
        w.gm,
        false,
        AdjustInventoryQuantityInput {
            inventory_entry_id: entry,
            quantity: 0,
        },
    )
    .await
    .expect_err("not adopted");
    assert_not_adopted(&error);

    use crate::schema::world_actor_inventory as inventory;
    let quantity: i32 = inventory::table
        .find(entry)
        .select(inventory::quantity)
        .first(&mut w.conn())
        .expect("the entry is still there");
    assert_eq!(quantity, 1);
}

#[tokio::test]
async fn sharing_a_staged_piece_is_refused_for_its_bringer_and_the_gm() {
    let w = world();
    let actor = w.actor();
    let spell = w.stage(w.player, "spell", "Ember Lance", json!({ "level": 1 }));
    w.link_ability(actor, spell, "Ember Lance");
    let sword = w.stage(w.player, "item", "Moonblade", json!({}));
    w.link_item(actor, sword, "Moonblade");

    for user in [w.player, w.gm, w.trusted] {
        let error = create_ability_share_link_impl(&w.state, user, false, spell, &terms())
            .await
            .expect_err("an ability not adopted is not shared");
        assert_not_adopted(&error);
        let error = create_item_share_link_impl(&w.state, user, false, sword, &terms())
            .await
            .expect_err("an item not adopted is not shared");
        assert_not_adopted(&error);
    }

    // Another player, and someone outside the table, get the ordinary answer.
    for user in [w.other, w.stranger] {
        let error = create_ability_share_link_impl(&w.state, user, false, spell, &terms())
            .await
            .expect_err("still refused");
        assert_ne!(code(&error).as_deref(), Some(NOT_ADOPTED_CODE));
    }
}

#[test]
fn an_entry_that_carries_a_world_item_is_not_a_staged_piece() {
    let w = world();
    let actor = w.actor();
    let item = crate::test_support::insert_test_item(&mut w.conn(), w.world, w.gm);
    use crate::schema::world_actor_inventory as inventory;
    let entry: Uuid = diesel::insert_into(inventory::table)
        .values((
            inventory::actor_id.eq(actor),
            inventory::item_id.eq(Some(item)),
            inventory::item_name_snapshot.eq("Rope"),
            inventory::quantity.eq(1),
        ))
        .returning(inventory::id)
        .get_result(&mut w.conn())
        .expect("carried");
    assert_eq!(
        super::carried_by_entry(&mut w.conn(), entry).expect("read"),
        None
    );
}
