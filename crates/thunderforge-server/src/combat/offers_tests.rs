//! Spec 046 T059: an offer — C6 once only, by a controller or a Game Master on
//! their behalf, a stranger refused, a taken offer changing hit points in the
//! same transaction, C10, and the relink rule.

use super::*;
use crate::combat::fixtures::*;
use crate::test_support::test_app_state;

fn offer_against_aria(conn: &mut PgConnection, t: &FightTable) -> Uuid {
    attack(conn, t.gm, t.ogre, t.greatclub, Some(t.aria))
        .expect("the ogre hits Aria")
        .offer_ids[0]
}

fn offer_against_goblin(conn: &mut PgConnection, t: &FightTable) -> Uuid {
    attack(conn, t.player, t.aria, t.longsword, Some(t.goblin))
        .expect("Aria hits the goblin")
        .offer_ids[0]
}

fn resolve(
    conn: &mut PgConnection,
    user: Uuid,
    offer: Uuid,
    take: bool,
) -> Result<OfferRecord, FightRefusal> {
    resolve_offer(conn, SYSTEMS_DIR, user, false, offer, take)
}

#[test]
fn a_controller_takes_their_offer_and_their_sheet_changes_with_it() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    let offer = offer_against_aria(&mut conn, &t);

    let taken = resolve(&mut conn, t.player, offer, true).expect("Aria's player takes it");
    assert_eq!(taken.status, OFFER_TAKEN);
    assert_eq!(taken.resolved_by, Some(t.player));
    assert!(!taken.resolved_on_behalf);
    assert!(taken.resolved_at.is_some());
    assert_eq!(actor_hp(&mut conn, t.aria_actor), (ARIA_HP - 9) as i64);
}

#[test]
fn declining_changes_nothing() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    let offer = offer_against_aria(&mut conn, &t);
    let declined = resolve(&mut conn, t.player, offer, false).expect("declined");
    assert_eq!(declined.status, OFFER_DECLINED);
    assert_eq!(actor_hp(&mut conn, t.aria_actor), ARIA_HP as i64);
}

#[test]
fn c6_an_offer_resolves_once_and_a_second_call_by_anyone_is_refused() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    let offer = offer_against_aria(&mut conn, &t);
    resolve(&mut conn, t.player, offer, true).expect("first");

    for (who, user, take) in [
        ("the player", t.player, true),
        ("the Game Master", t.gm, false),
    ] {
        let refused = resolve(&mut conn, user, offer, take).unwrap_err();
        assert_eq!(
            refused,
            FightRefusal::Invalid("That offer has already been resolved".into()),
            "{who}"
        );
    }
    assert_eq!(
        actor_hp(&mut conn, t.aria_actor),
        (ARIA_HP - 9) as i64,
        "the damage landed once"
    );
}

#[test]
fn c6_two_takes_at_once_resolve_it_once() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    let offer = offer_against_goblin(&mut conn, &t);
    drop(conn);

    let (gm, pool_a, pool_b) = (t.gm, state.db_pool.clone(), state.db_pool.clone());
    let a = std::thread::spawn(move || {
        let mut conn = pool_a.get().expect("conn");
        resolve(&mut conn, gm, offer, true).is_ok()
    });
    let b = std::thread::spawn(move || {
        let mut conn = pool_b.get().expect("conn");
        resolve(&mut conn, gm, offer, true).is_ok()
    });
    let succeeded = [a.join().unwrap(), b.join().unwrap()];
    assert_eq!(
        succeeded.iter().filter(|ok| **ok).count(),
        1,
        "{succeeded:?}"
    );
    let mut conn = state.db_pool.get().expect("conn");
    assert_eq!(copy_hp(&mut conn, t.goblin), (GOBLIN_HP - 5) as i64);
}

#[test]
fn a_game_master_resolves_a_players_offer_on_their_behalf_and_their_own_not() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);

    let for_aria = offer_against_aria(&mut conn, &t);
    let taken = resolve(&mut conn, t.gm, for_aria, true).expect("the Game Master takes it for her");
    assert!(taken.resolved_on_behalf, "a player controls Aria");
    assert_eq!(actor_hp(&mut conn, t.aria_actor), (ARIA_HP - 9) as i64);

    let for_goblin = offer_against_goblin(&mut conn, &t);
    let taken = resolve(&mut conn, t.gm, for_goblin, true).expect("their own goblin");
    assert!(!taken.resolved_on_behalf, "nobody else controls the goblin");
}

#[test]
fn a_player_who_does_not_control_the_target_is_refused() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    let for_aria = offer_against_aria(&mut conn, &t);
    let for_goblin = offer_against_goblin(&mut conn, &t);

    assert_eq!(
        resolve(&mut conn, t.stranger, for_aria, true).unwrap_err(),
        FightRefusal::NotControlled
    );
    // The attacker does not decide what the target takes.
    assert_eq!(
        resolve(&mut conn, t.player, for_goblin, false).unwrap_err(),
        FightRefusal::NotControlled
    );
    assert_eq!(offer_row(&mut conn, for_aria).status, OFFER_PENDING);
    assert_eq!(offer_row(&mut conn, for_goblin).status, OFFER_PENDING);
}

#[test]
fn a_taken_offer_and_its_hit_points_commit_together_or_not_at_all() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    let offer = offer_against_goblin(&mut conn, &t);
    // The copy loses its record between the offer and the take: the change
    // cannot be made, so the offer must not read as taken.
    diesel::update(tokens::table.filter(tokens::token_id.eq(t.goblin)))
        .set(tokens::system_data.eq(None::<serde_json::Value>))
        .execute(&mut conn)
        .expect("strip the copy");

    let refused = resolve(&mut conn, t.gm, offer, true).unwrap_err();
    assert!(matches!(refused, FightRefusal::Invalid(_)), "{refused:?}");
    assert_eq!(offer_row(&mut conn, offer).status, OFFER_PENDING);
    assert_eq!(offer_row(&mut conn, offer).resolved_by, None);
}

#[test]
fn c10_a_paused_world_refuses_resolving_an_offer() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    let offer = offer_against_aria(&mut conn, &t);
    pause(&mut conn, &t);
    let refused = resolve(&mut conn, t.player, offer, true).unwrap_err();
    assert!(matches!(refused, FightRefusal::Paused(_)), "{refused:?}");
    assert_eq!(offer_row(&mut conn, offer).status, OFFER_PENDING);
}

// ---------------------------------------------------------------------------
// The relink rule (module documentation, contract C6a)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn an_offer_against_a_copy_relinked_since_is_refused_on_take_and_never_lands_on_the_npc() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    let offer = offer_against_goblin(&mut conn, &t);

    // The Game Master relinks the goblin to its NPC, through the mutation.
    crate::graphql::mutations_token_links::set_token_link_impl(&state, t.gm, false, t.goblin, true)
        .await
        .expect("relinked");

    let refused = resolve(&mut conn, t.gm, offer, true).unwrap_err();
    assert_eq!(refused, FightRefusal::Invalid(RELINKED_SINCE.into()));
    assert_eq!(
        actor_hp(&mut conn, t.goblin_actor),
        GOBLIN_HP as i64,
        "the NPC's own sheet is untouched"
    );
    assert_eq!(offer_row(&mut conn, offer).status, OFFER_PENDING);

    // Declining it is always possible.
    let declined = resolve(&mut conn, t.gm, offer, false).expect("declined");
    assert_eq!(declined.status, OFFER_DECLINED);
}

// ---------------------------------------------------------------------------
// FR-008: pending offers on return
// ---------------------------------------------------------------------------

#[test]
fn pending_offers_are_a_players_own_and_every_one_for_a_game_master() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    let for_aria = offer_against_aria(&mut conn, &t);
    let for_goblin = offer_against_goblin(&mut conn, &t);

    let ids = |offers: Vec<OfferRecord>| offers.into_iter().map(|o| o.id).collect::<Vec<_>>();
    assert_eq!(
        ids(pending_offers(&mut conn, t.player, false, t.world_id).unwrap()),
        vec![for_aria]
    );
    assert!(
        pending_offers(&mut conn, t.stranger, false, t.world_id)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        ids(pending_offers(&mut conn, t.gm, false, t.world_id).unwrap()),
        vec![for_aria, for_goblin]
    );

    // Resolved offers are not pending; one never resolves itself.
    resolve(&mut conn, t.player, for_aria, false).expect("declined");
    assert!(
        pending_offers(&mut conn, t.player, false, t.world_id)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        ids(pending_offers(&mut conn, t.gm, false, t.world_id).unwrap()),
        vec![for_goblin]
    );
}
