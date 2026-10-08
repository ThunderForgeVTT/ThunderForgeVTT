//! Spec 081 T006: `worldRoll` and `worldRolls` answer each viewer by the
//! visibility rule — whole, masked, or as though the roll did not exist.

use diesel::prelude::*;
use rand::SeedableRng;
use uuid::Uuid;

use super::roll::{world_roll_impl, world_rolls_impl};
use crate::graphql::mutations_roll::{RollDiceInput, roll_dice_impl};
use crate::graphql::types::{RollVisibility, WorldRollEntry};
use crate::schema::world_roll_records;
use crate::state::AppState;
use crate::test_support::{
    insert_test_user, insert_test_world, insert_test_world_member, test_app_state,
};

/// A GM, two players and an operator who is not at the table.
struct Table {
    state: AppState,
    world: Uuid,
    gm: Uuid,
    roller: Uuid,
    other: Uuid,
    admin: Uuid,
}

fn a_table() -> Table {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let gm = insert_test_user(&mut conn);
    let roller = insert_test_user(&mut conn);
    let other = insert_test_user(&mut conn);
    let admin = insert_test_user(&mut conn);
    let world = insert_test_world(&mut conn, gm);
    insert_test_world_member(&mut conn, world, roller, "Player");
    insert_test_world_member(&mut conn, world, other, "Player");
    drop(conn);
    Table {
        state,
        world,
        gm,
        roller,
        other,
        admin,
    }
}

/// Roll `1d20` as `who` and hand back the new roll's id.
async fn roll(t: &Table, who: Uuid, visibility: RollVisibility) -> Uuid {
    let mut rng = rand::rngs::StdRng::seed_from_u64(7);
    roll_dice_impl(
        &t.state,
        who,
        RollDiceInput {
            world_id: t.world,
            formula: "1d20".to_string(),
            bindings: None,
            visibility: Some(visibility),
            label: Some("Perception".to_string()),
        },
        &mut rng,
    )
    .await
    .expect("the roll is allowed");
    let mut conn = t.state.db_pool.get().unwrap();
    world_roll_records::table
        .filter(world_roll_records::world_id.eq(t.world))
        .filter(world_roll_records::triggered_by.eq(who))
        .order(world_roll_records::created_at.desc())
        .select(world_roll_records::id)
        .first(&mut conn)
        .unwrap()
}

#[derive(Debug, PartialEq)]
enum Seen {
    Whole,
    Masked,
    Nothing,
}

fn seen(entry: Option<WorldRollEntry>) -> Seen {
    match entry {
        Some(WorldRollEntry::WorldRoll(_)) => Seen::Whole,
        Some(WorldRollEntry::MaskedRoll(_)) => Seen::Masked,
        None => Seen::Nothing,
    }
}

async fn as_seen_by(t: &Table, who: Uuid, is_admin: bool, roll_id: Uuid) -> Seen {
    seen(
        world_roll_impl(&t.state, who, is_admin, t.world, roll_id)
            .await
            .expect("a member may ask"),
    )
}

#[tokio::test]
async fn each_viewer_sees_each_visibility_as_the_rule_says() {
    let t = a_table();
    let open = roll(&t, t.roller, RollVisibility::Everyone).await;
    let eyes = roll(&t, t.roller, RollVisibility::GmEyes).await;
    let hidden = roll(&t, t.gm, RollVisibility::GmOnly).await;

    use Seen::*;
    // (roll, roller, GM, admin, other player)
    let expected = [
        (open, Whole, Whole, Whole, Whole),
        (eyes, Whole, Whole, Whole, Masked),
        // Rolled by the GM: the player who rolled the others sees nothing.
        (hidden, Nothing, Whole, Whole, Nothing),
    ];
    for (id, by_roller, by_gm, by_admin, by_other) in expected {
        assert_eq!(as_seen_by(&t, t.roller, false, id).await, by_roller);
        assert_eq!(as_seen_by(&t, t.gm, false, id).await, by_gm);
        assert_eq!(as_seen_by(&t, t.admin, true, id).await, by_admin);
        assert_eq!(as_seen_by(&t, t.other, false, id).await, by_other);
    }
}

#[tokio::test]
async fn a_masked_roll_carries_no_dice() {
    let t = a_table();
    let eyes = roll(&t, t.roller, RollVisibility::GmEyes).await;
    let entry = world_roll_impl(&t.state, t.other, false, t.world, eyes)
        .await
        .unwrap()
        .expect("masked, not hidden");
    let WorldRollEntry::MaskedRoll(masked) = entry else {
        panic!("another player must get the masked shape");
    };
    // `MaskedRoll` has no field for the dice, the formula or the label, so
    // what is asserted is what it does carry.
    assert_eq!(masked.id, eyes);
    assert!(!masked.roller_name.is_empty());
    assert_eq!(masked.visibility, RollVisibility::GmEyes);
}

#[tokio::test]
async fn a_non_member_gets_nothing() {
    let t = a_table();
    let open = roll(&t, t.roller, RollVisibility::Everyone).await;
    let stranger = {
        let mut conn = t.state.db_pool.get().unwrap();
        insert_test_user(&mut conn)
    };
    assert!(
        world_roll_impl(&t.state, stranger, false, t.world, open)
            .await
            .is_err()
    );
    assert!(
        world_rolls_impl(&t.state, stranger, false, t.world, None, None)
            .await
            .is_err()
    );
}

fn ids(entries: &[WorldRollEntry]) -> Vec<Uuid> {
    entries
        .iter()
        .map(|e| match e {
            WorldRollEntry::WorldRoll(r) => r.id,
            WorldRollEntry::MaskedRoll(r) => r.id,
        })
        .collect()
}

#[tokio::test]
async fn the_feed_pages_newest_first_and_a_hidden_roll_leaves_no_hole() {
    let t = a_table();
    let first = roll(&t, t.roller, RollVisibility::Everyone).await;
    let second = roll(&t, t.roller, RollVisibility::GmEyes).await;
    let hidden = roll(&t, t.gm, RollVisibility::GmOnly).await;
    let fourth = roll(&t, t.roller, RollVisibility::Everyone).await;

    let gm = world_rolls_impl(&t.state, t.gm, false, t.world, None, None)
        .await
        .unwrap();
    assert_eq!(ids(&gm), vec![fourth, hidden, second, first]);

    // Two at a time: the hidden roll is left out in the query, so the first
    // page is still full.
    let page = world_rolls_impl(&t.state, t.other, false, t.world, None, Some(2))
        .await
        .unwrap();
    assert_eq!(ids(&page), vec![fourth, second]);
    assert!(matches!(page[1], WorldRollEntry::MaskedRoll(_)));

    let before = match &page[1] {
        WorldRollEntry::MaskedRoll(r) => r.created_at.clone(),
        WorldRollEntry::WorldRoll(r) => r.created_at.clone(),
    };
    let next = world_rolls_impl(&t.state, t.other, false, t.world, Some(before), Some(2))
        .await
        .unwrap();
    assert_eq!(ids(&next), vec![first]);
}

/// Spec 084 T013: a reroll and the roll it replaced point at each other in
/// the feed, the one by `rerollOf` and the other by `rerolledBy`.
#[tokio::test]
async fn a_feed_of_two_chained_rolls_shows_both_links() {
    let t = a_table();
    let first = roll(&t, t.roller, RollVisibility::Everyone).await;
    let second = roll(&t, t.roller, RollVisibility::Everyone).await;
    {
        let mut conn = t.state.db_pool.get().unwrap();
        diesel::update(world_roll_records::table.find(second))
            .set((
                world_roll_records::reroll_of.eq(first),
                world_roll_records::reroll_spent.eq("inspiration"),
            ))
            .execute(&mut conn)
            .unwrap();
    }
    let feed = world_rolls_impl(&t.state, t.other, false, t.world, None, None)
        .await
        .unwrap();
    let whole = |id: Uuid| {
        feed.iter()
            .find_map(|e| match e {
                WorldRollEntry::WorldRoll(r) if r.id == id => Some(r.clone()),
                _ => None,
            })
            .expect("both rolls are open to the table")
    };
    let (old, new) = (whole(first), whole(second));
    assert_eq!(old.rerolled_by, Some(second));
    assert_eq!(old.reroll_of, None);
    assert_eq!(new.reroll_of, Some(first));
    assert_eq!(new.rerolled_by, None);
    // A world with no system names nothing: the spend shows by its id.
    assert_eq!(new.spent.map(|s| s.id), Some("inspiration".to_string()));
    assert!(old.reroll_offers.is_empty() && old.reroll_until.is_none());
}
