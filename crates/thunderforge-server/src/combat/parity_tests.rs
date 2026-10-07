//! Spec 079 US3: one fight, two ways, the same fight.
//!
//! A scripted exchange — Aria at the ogre three times, the ogre back at Aria
//! once — under named dice, made once through the server (`make_attack`, the
//! offer taken with `resolve_offer`, every row written) and once through the
//! `thunderforge_combat` crate alone, as the demo makes it. Each attack's
//! to-hit total, outcome and offered damage, and both creatures' hit points
//! at the end, must agree.

use diesel::prelude::*;
use thunderforge_combat::attack::{attack_formulas, damage_source, judge, offered_amount, roll};
use thunderforge_combat::dice::ScriptedDice;
use thunderforge_combat::hit_points::{HitPointChangeKind, HitPoints, apply_to};
use thunderforge_dice::PlaceholderBindings;

use crate::combat::attack::{AttackRequest, Attacker, make_attack};
use crate::combat::fixtures::*;
use crate::combat::offers::resolve_offer;
use crate::combat::records::OFFER_PENDING;
use crate::schema::world_roll_records;
use crate::test_support::test_app_state;

/// One attack of the script: who, with what, at whom, and the faces.
struct Swing {
    by_aria: bool,
    faces: &'static [u32],
}

const SHORTSWORD: (&str, &str) = ("1d20+4", "1d6+2");
const CLUB: (&str, &str) = ("1d20+6", "2d8+4");

const SCRIPT: [Swing; 4] = [
    // 7+4 = 11 against the ogre's 11: a tie hits; 3+2 damage.
    Swing {
        by_aria: true,
        faces: &[7, 3],
    },
    // 6+4 = 10: a miss, and no damage die is drawn.
    Swing {
        by_aria: true,
        faces: &[6],
    },
    // 20+4: a hit; 6+2 damage.
    Swing {
        by_aria: true,
        faces: &[20, 6],
    },
    // 8+6 = 14 against Aria's 14: a hit; 5+7+4 = 16, all she has.
    Swing {
        by_aria: false,
        faces: &[8, 5, 7],
    },
];

/// What one attack decided, wherever it was decided.
#[derive(Debug, PartialEq)]
struct Decided {
    to_hit: f64,
    outcome: String,
    amount: Option<i32>,
}

fn effects(formulas: (&str, &str)) -> Vec<(String, String)> {
    vec![
        ("attack_roll".to_string(), formulas.0.to_string()),
        ("damage".to_string(), formulas.1.to_string()),
    ]
}

/// The fight as the demo has it: the crate, the creatures in memory.
fn through_the_crate() -> (Vec<Decided>, i32, i32) {
    let mut ogre = HitPoints {
        current: OGRE_HP,
        max: OGRE_HP,
        temporary: 0,
    };
    let mut aria = HitPoints {
        current: ARIA_HP,
        max: ARIA_HP,
        temporary: 0,
    };
    let bindings = PlaceholderBindings::new();
    let mut decided = Vec::new();
    for swing in &SCRIPT {
        let (name, formulas, defence) = if swing.by_aria {
            ("Shortsword", SHORTSWORD, OGRE_AC)
        } else {
            ("Club", CLUB, ARIA_AC)
        };
        let mut dice = ScriptedDice::faces(swing.faces.iter().copied());
        let (to_hit, damage) = attack_formulas(name, &effects(formulas)).expect("formulas");
        let (_, total) = roll(&to_hit, &bindings, &mut dice).expect("to hit");
        let outcome = judge(true, Some(defence), total);
        let amount = match damage_source(&damage) {
            Some(source) if outcome == crate::combat::records::OUTCOME_HIT => Some(offered_amount(
                roll(&source, &bindings, &mut dice).expect("damage").1,
            )),
            _ => None,
        };
        if let Some(amount) = amount {
            let target = if swing.by_aria { &mut ogre } else { &mut aria };
            *target = apply_to(*target, HitPointChangeKind::Damage, amount);
        }
        decided.push(Decided {
            to_hit: total,
            outcome: outcome.to_string(),
            amount,
        });
    }
    (decided, ogre.current, aria.current)
}

#[test]
fn a_scripted_fight_is_the_same_fight_on_the_server_and_in_the_crate() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    let shortsword = ability(
        &mut conn,
        t.world_id,
        t.gm,
        "Shortsword",
        SHORTSWORD.0,
        SHORTSWORD.1,
    );
    let club = ability(&mut conn, t.world_id, t.gm, "Club", CLUB.0, CLUB.1);
    attach(&mut conn, t.aria_actor, shortsword);
    attach(&mut conn, t.ogre_actor, club);

    let mut on_the_server = Vec::new();
    for swing in &SCRIPT {
        let (user, attacker, ability, target) = if swing.by_aria {
            (t.player, t.aria, shortsword, t.ogre)
        } else {
            (t.gm, t.ogre, club, t.aria)
        };
        let made = make_attack(
            &mut conn,
            SYSTEMS_DIR,
            user,
            false,
            &AttackRequest {
                attacker: Attacker::Token(attacker),
                ability_id: Some(ability),
                target_token_id: Some(target),
                ..Default::default()
            },
            &mut ScriptedDice::faces(swing.faces.iter().copied()),
        )
        .expect("attack");
        let row = attack_row(&mut conn, made.attack_ids[0]);
        let to_hit = world_roll_records::table
            .filter(world_roll_records::id.eq(row.to_hit_roll_id.expect("to-hit roll")))
            .select(world_roll_records::result_value)
            .first::<f64>(&mut conn)
            .expect("to-hit total");
        let offer = made.offer_ids.first().map(|id| offer_row(&mut conn, *id));
        if let Some(offer) = &offer
            && offer.status == OFFER_PENDING
        {
            resolve_offer(&mut conn, SYSTEMS_DIR, t.gm, false, offer.id, true).expect("take");
        }
        on_the_server.push(Decided {
            to_hit,
            outcome: row.outcome,
            amount: offer.map(|o| o.amount),
        });
    }

    let (in_the_crate, ogre_hp, aria_hp) = through_the_crate();
    assert_eq!(on_the_server, in_the_crate);
    assert_eq!(actor_hp(&mut conn, t.ogre_actor), ogre_hp as i64);
    assert_eq!(actor_hp(&mut conn, t.aria_actor), aria_hp as i64);
    // The script is the fight it says it is.
    assert_eq!(ogre_hp, OGRE_HP - 5 - 8);
    assert_eq!(aria_hp, 0);
}
