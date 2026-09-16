//! Spec 046 T060: FR-002a, per-viewer redaction (contract §3).
//!
//! Proved at two levels: the sight (`SceneSight::party`), and the answer a
//! client actually receives (`types_attacks::build_attacks`), searched as
//! text for the hidden ogre's id and name — the claim is "in no field", so the
//! test reads every field.

use super::*;
use crate::combat::fixtures::*;
use crate::graphql::types::types_attacks::{Sights, build_attacks, build_offers};
use crate::test_support::test_app_state;

fn sight(conn: &mut PgConnection, user: Uuid, t: &FightTable) -> SceneSight {
    SceneSight::for_viewer(conn, SYSTEMS_DIR, user, false, t.scene_id).expect("sight")
}

fn hide_ogre_name(conn: &mut PgConnection, t: &FightTable) {
    diesel::update(tokens::table.filter(tokens::token_id.eq(t.ogre)))
        .set(tokens::name_visible_to_players.eq(false))
        .execute(conn)
        .expect("hide the ogre's name");
}

/// A wall between Aria (at the origin) and the ogre (200 east), leaving the
/// goblin (100 east) in plain sight.
fn wall_off_ogre(conn: &mut PgConnection, t: &FightTable) {
    wall(conn, t, (150.0, -500.0), (150.0, 500.0));
}

#[test]
fn a_game_master_sees_everyone() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    hide_ogre_name(&mut conn, &t);
    wall_off_ogre(&mut conn, &t);
    let gm = sight(&mut conn, t.gm, &t);
    assert_eq!(
        gm.party(Some(t.ogre), OGRE_NAME),
        Party {
            token_id: Some(t.ogre),
            label: OGRE_NAME.into(),
            redacted: false
        }
    );
}

#[test]
fn a_creature_in_plain_sight_is_named() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    // The party has met these two, so sight is the only thing judged here.
    show_npcs(&mut conn, &t);
    let player = sight(&mut conn, t.player, &t);
    assert_eq!(player.party(Some(t.ogre), OGRE_NAME).token_id, Some(t.ogre));
    assert_eq!(player.party(Some(t.goblin), "Goblin").label, "Goblin");
}

#[test]
fn a_hidden_name_reads_unknown_even_in_plain_sight() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    hide_ogre_name(&mut conn, &t);
    let player = sight(&mut conn, t.player, &t);
    assert_eq!(
        player.party(Some(t.ogre), OGRE_NAME),
        Party {
            token_id: None,
            label: UNKNOWN.into(),
            redacted: true
        }
    );
}

#[test]
fn a_creature_behind_a_wall_reads_unknown_and_one_in_front_of_it_does_not() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    // The party has met these two, so sight is the only thing judged here.
    show_npcs(&mut conn, &t);
    wall_off_ogre(&mut conn, &t);
    let player = sight(&mut conn, t.player, &t);
    assert!(player.party(Some(t.ogre), OGRE_NAME).redacted);
    assert!(!player.party(Some(t.goblin), "Goblin").redacted);
}

#[test]
fn darkness_hides_and_a_light_or_darkvision_reveals() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    // The party has met these two, so sight is the only thing judged here.
    show_npcs(&mut conn, &t);
    diesel::update(scenes::table.filter(scenes::scene_id.eq(t.scene_id)))
        .set(scenes::ambient_light.eq("dark"))
        .execute(&mut conn)
        .expect("lights out");
    assert!(
        sight(&mut conn, t.player, &t)
            .party(Some(t.ogre), OGRE_NAME)
            .redacted
    );

    // A torch by the ogre.
    diesel::insert_into(light_sources::table)
        .values((
            light_sources::light_id.eq(Uuid::now_v7()),
            light_sources::scene_id.eq(t.scene_id),
            light_sources::x.eq(200.0),
            light_sources::y.eq(0.0),
            light_sources::radius.eq(60.0),
            light_sources::bright_radius.eq(30.0),
            light_sources::intensity.eq(1.0),
            light_sources::casts_shadows.eq(true),
            light_sources::created_by.eq(t.gm),
            light_sources::updated_by.eq(t.gm),
            light_sources::created_at.eq(chrono::Utc::now().naive_utc()),
            light_sources::updated_at.eq(chrono::Utc::now().naive_utc()),
        ))
        .execute(&mut conn)
        .expect("a torch");
    assert!(
        !sight(&mut conn, t.player, &t)
            .party(Some(t.ogre), OGRE_NAME)
            .redacted
    );
}

#[test]
fn a_viewer_always_knows_their_own_creature_and_one_with_no_token_knows_nobody() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    wall_off_ogre(&mut conn, &t);
    // Aria's name hidden and she is her player's: her player still knows her.
    diesel::update(tokens::table.filter(tokens::token_id.eq(t.aria)))
        .set(tokens::name_visible_to_players.eq(false))
        .execute(&mut conn)
        .expect("hide Aria's name");
    assert!(
        !sight(&mut conn, t.player, &t)
            .party(Some(t.aria), "Aria")
            .redacted
    );
    // The stranger has no token here, so sees nobody, even in daylight.
    let stranger = sight(&mut conn, t.stranger, &t);
    assert!(stranger.party(Some(t.goblin), "Goblin").redacted);
    // And a deleted token is known to no player.
    assert!(sight(&mut conn, t.player, &t).party(None, "Gone").redacted);
}

#[test]
fn redaction_is_decided_when_the_answer_is_built_so_it_follows_the_board() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    // The party has met these two, so sight is the only thing judged here.
    show_npcs(&mut conn, &t);
    wall_off_ogre(&mut conn, &t);
    let made = attack(&mut conn, t.gm, t.ogre, t.greatclub, Some(t.aria)).expect("attack");
    let record = attack_row(&mut conn, made.attack_ids[0]);

    let read = |conn: &mut PgConnection| {
        let mut sights = Sights::new(SYSTEMS_DIR, t.player, false);
        build_attacks(conn, &mut sights, vec![record.clone()])
            .expect("built")
            .remove(0)
    };
    assert_eq!(read(&mut conn).attacker.label, UNKNOWN);

    // Aria walks round the wall's end.
    diesel::update(tokens::table.filter(tokens::token_id.eq(t.aria)))
        .set((tokens::x.eq(150.0), tokens::y.eq(600.0)))
        .execute(&mut conn)
        .expect("move");
    assert_eq!(read(&mut conn).attacker.label, OGRE_NAME);
}

#[test]
fn a_redacted_attacker_leaves_no_id_name_or_ability_in_any_field_of_the_answer() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    hide_ogre_name(&mut conn, &t);
    wall_off_ogre(&mut conn, &t);

    let made = attack(&mut conn, t.gm, t.ogre, t.greatclub, Some(t.aria)).expect("the ogre swings");
    let record = attack_row(&mut conn, made.attack_ids[0]);

    let mut sights = Sights::new(SYSTEMS_DIR, t.player, false);
    let seen = build_attacks(&mut conn, &mut sights, vec![record.clone()])
        .expect("built")
        .remove(0);
    assert_eq!(seen.attacker.token_id, None);
    assert_eq!(seen.attacker.label, UNKNOWN);
    assert_eq!(
        seen.ability_name, None,
        "attacker redaction nulls the ability"
    );
    // Aria's player knows what was hit: their own creature.
    assert_eq!(seen.target.as_ref().and_then(|p| p.token_id), Some(t.aria));
    assert_eq!(seen.defence, Some(ARIA_AC));
    let offer = seen.offer.as_ref().expect("Aria's offer");
    assert!(offer.may_resolve);

    let everything = format!("{seen:?}");
    for secret in [
        t.ogre.to_string(),
        t.ogre_actor.to_string(),
        OGRE_NAME.to_string(),
        "Greatclub".to_string(),
    ] {
        assert!(
            !everything.contains(&secret),
            "{secret} reached the player in {everything}"
        );
    }

    // The same attack, read by the Game Master, is whole.
    let mut sights = Sights::new(SYSTEMS_DIR, t.gm, false);
    let whole = build_attacks(&mut conn, &mut sights, vec![record])
        .expect("built")
        .remove(0);
    assert_eq!(whole.attacker.token_id, Some(t.ogre));
    assert_eq!(whole.ability_name.as_deref(), Some("Greatclub"));

    // Nor is it in any event payload.
    use crate::schema::world_events;
    let payloads: Vec<Option<serde_json::Value>> = world_events::table
        .filter(world_events::world_id.eq(t.world_id))
        .select(world_events::token_event)
        .load(&mut conn)
        .expect("events");
    let payloads = format!("{payloads:?}");
    assert!(!payloads.contains(&t.ogre.to_string()), "{payloads}");
    assert!(!payloads.contains(OGRE_NAME), "{payloads}");
}

#[test]
fn a_redacted_target_nulls_its_defence_and_leaves_no_trace_in_the_answer() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    // The party has met these two, so sight is the only thing judged here.
    show_npcs(&mut conn, &t);
    hide_ogre_name(&mut conn, &t);
    // The goblin swings at the ogre; Aria's player can see the goblin, not
    // who it swung at.
    let made = attack(&mut conn, t.gm, t.goblin, t.greatclub, Some(t.ogre)).expect("attack");
    let record = attack_row(&mut conn, made.attack_ids[0]);
    let mut sights = Sights::new(SYSTEMS_DIR, t.player, false);
    let seen = build_attacks(&mut conn, &mut sights, vec![record])
        .expect("built")
        .remove(0);
    let target = seen.target.as_ref().expect("there was a target");
    assert_eq!(target.token_id, None);
    assert_eq!(target.label, UNKNOWN);
    assert_eq!(seen.defence, None, "target redaction nulls the defence");
    assert_eq!(seen.attacker.label, "Goblin");
    let everything = format!("{seen:?}");
    assert!(!everything.contains(&t.ogre.to_string()), "{everything}");
    assert!(!everything.contains(OGRE_NAME), "{everything}");
}

#[test]
fn pending_offers_read_by_their_controller_are_never_redacted() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    wall_off_ogre(&mut conn, &t);
    hide_ogre_name(&mut conn, &t);
    attack(&mut conn, t.gm, t.ogre, t.greatclub, Some(t.aria)).expect("attack");
    let offers = crate::combat::offers::pending_offers(&mut conn, t.player, false, t.world_id)
        .expect("offers");
    let mut sights = Sights::new(SYSTEMS_DIR, t.player, false);
    let built = build_offers(&mut conn, &mut sights, offers).expect("built");
    assert_eq!(built.len(), 1);
    assert_eq!(built[0].target.token_id, Some(t.aria));
    assert!(!format!("{built:?}").contains(OGRE_NAME));
}

/// Spec 046 Phase 7's decision, pinned (research R11, contract §3): an
/// attack's line of sight is judged from the squares a creature fills, and a
/// viewer's redaction from token centres, as the engine draws tokens. A Large
/// ogre half round a corner can be swung at with line of sight, while the
/// player whose board does not draw it still reads "Unknown".
#[test]
fn redaction_follows_the_board_by_centres_while_an_attacks_sight_follows_footprints() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    // A 50-unit grid whose lines fall on multiples of 50; the ogre is Large.
    diesel::update(scenes::table.filter(scenes::scene_id.eq(t.scene_id)))
        .set((
            scenes::grid_size.eq(50),
            scenes::width.eq(1000),
            scenes::height.eq(1000),
        ))
        .execute(&mut conn)
        .expect("grid");
    diesel::update(
        world_actor_system_data::table.filter(world_actor_system_data::actor_id.eq(t.ogre_actor)),
    )
    .set(world_actor_system_data::trait_data.eq(Some(
        serde_json::json!({ "class": "monster", "level": 1, "size": "large" }),
    )))
    .execute(&mut conn)
    .expect("Large");
    let at = |conn: &mut PgConnection, token: Uuid, x: f64, y: f64| {
        diesel::update(tokens::table.filter(tokens::token_id.eq(token)))
            .set((tokens::x.eq(x), tokens::y.eq(y)))
            .execute(conn)
            .expect("move");
    };
    // Aria in cell (0,3). The ogre fills (1..2, 0..1), centred at (100, 50).
    // A wall along y = 100 ends at x = 75: the line between the two centres
    // meets it at x = 70, while the ogre's right-hand squares are past it.
    at(&mut conn, t.aria, 25.0, 175.0);
    at(&mut conn, t.ogre, 100.0, 50.0);
    at(&mut conn, t.goblin, 1025.0, 1025.0);
    wall(&mut conn, &t, (-500.0, 100.0), (75.0, 100.0));

    let made = attack(&mut conn, t.player, t.aria, t.longsword, Some(t.ogre)).expect("attack");
    let row = attack_row(&mut conn, made.attack_ids[0]);
    assert!(
        !row.flags
            .iter()
            .flatten()
            .any(|f| f == crate::combat::records::FLAG_NO_LINE_OF_SIGHT),
        "the swing has line of sight from Aria's square to the ogre's: {:?}",
        row.flags
    );
    assert!(
        sight(&mut conn, t.player, &t)
            .party(Some(t.ogre), OGRE_NAME)
            .redacted,
        "and her log still names what her board draws: nothing, by centres"
    );
}
