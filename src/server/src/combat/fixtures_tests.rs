//! A table for the fight's server tests (spec 046 T058–T060): a Game Master,
//! a player with Aria, a second player with nobody, a goblin copy, an ogre,
//! weapons that always hit or always miss, and helpers to start a fight.
//!
//! Committed rather than inside a test transaction, as `hit_points_tests`
//! does: locks and "once only" are claims about separate connections.

use diesel::PgConnection;
use diesel::prelude::*;
use rand::SeedableRng;
use serde_json::json;
use uuid::Uuid;

use crate::combat::attack::{AttackRequest, MadeAttack, make_attack};
use crate::combat::records::{AttackRecord, OfferRecord};
use crate::schema::{
    tokens, walls, world_abilities, world_ability_effects, world_actor_abilities,
    world_actor_system_data, world_actors, world_attacks, world_combatants, world_combats,
    world_offers, worlds,
};
use crate::test_support::{
    insert_test_scene, insert_test_user, insert_test_world, insert_test_world_member,
};

pub const SYSTEMS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../packs/systems");

pub const ARIA_HP: i32 = 16;
pub const ARIA_AC: i32 = 14;
pub const GOBLIN_HP: i32 = 7;
pub const GOBLIN_AC: i32 = 13;
pub const OGRE_HP: i32 = 59;
pub const OGRE_AC: i32 = 11;
/// The name the Game Master hides in the redaction tests.
pub const OGRE_NAME: &str = "Grukk the Ogre";

pub struct FightTable {
    pub world_id: Uuid,
    pub scene_id: Uuid,
    pub gm: Uuid,
    pub player: Uuid,
    /// A member who controls nothing.
    pub stranger: Uuid,
    pub aria_actor: Uuid,
    pub aria: Uuid,
    pub goblin_actor: Uuid,
    pub goblin: Uuid,
    pub ogre_actor: Uuid,
    pub ogre: Uuid,
    /// Aria's: `1d20+100` to hit, `5` damage.
    pub longsword: Uuid,
    /// Aria's: `1d20-100` to hit — never lands.
    pub whiff: Uuid,
    /// The ogre's: `1d20+100` to hit, `9` damage.
    pub greatclub: Uuid,
}

pub fn rng() -> rand::rngs::StdRng {
    rand::rngs::StdRng::seed_from_u64(46)
}

fn actor(
    conn: &mut PgConnection,
    t: (Uuid, Uuid, Uuid),
    label: &str,
    is_npc: bool,
    hp: i32,
    ac: i32,
) -> Uuid {
    let (world_id, scene_id, gm) = t;
    let id = Uuid::now_v7();
    let now = chrono::Utc::now().naive_utc();
    diesel::insert_into(world_actors::table)
        .values((
            world_actors::id.eq(id),
            world_actors::world_id.eq(world_id),
            world_actors::scene_id.eq(scene_id),
            world_actors::actor_type.eq(if is_npc { "npc" } else { "character" }),
            world_actors::game_system_id.eq("dnd5e"),
            world_actors::label.eq(label),
            world_actors::created_by.eq(gm),
            world_actors::owned_by.eq(gm),
            world_actors::is_public.eq(false),
            world_actors::is_npc.eq(is_npc),
            world_actors::created_at.eq(now),
            world_actors::updated_at.eq(now),
        ))
        .execute(conn)
        .expect("insert actor");
    diesel::insert_into(world_actor_system_data::table)
        .values((
            world_actor_system_data::actor_id.eq(id),
            world_actor_system_data::game_system_id.eq("dnd5e"),
            world_actor_system_data::resource_data
                .eq(json!({ "current_hp": hp, "max_hp": hp, "temporary_hp": 0 })),
            world_actor_system_data::ability_data.eq(json!({
                "strength": 14, "dexterity": 12, "constitution": 12,
                "intelligence": 10, "wisdom": 10, "charisma": 10,
                "armor_class": ac
            })),
            world_actor_system_data::created_by.eq(gm),
            world_actor_system_data::updated_by.eq(gm),
        ))
        .execute(conn)
        .expect("insert sheet");
    id
}

#[allow(clippy::too_many_arguments)]
pub fn place(
    conn: &mut PgConnection,
    scene_id: Uuid,
    actor_id: Option<Uuid>,
    owner: Option<Uuid>,
    linked: bool,
    system_data: Option<serde_json::Value>,
    label: &str,
    at: (f64, f64),
) -> Uuid {
    let id = Uuid::now_v7();
    let now = chrono::Utc::now().naive_utc();
    diesel::insert_into(tokens::table)
        .values((
            tokens::token_id.eq(id),
            tokens::scene_id.eq(scene_id),
            tokens::actor_id.eq(actor_id),
            tokens::owner_user_id.eq(owner),
            tokens::linked.eq(linked),
            tokens::system_data.eq(system_data),
            tokens::metadata.eq(Some(json!({ "label": label }))),
            tokens::x.eq(at.0),
            tokens::y.eq(at.1),
            tokens::rotation.eq(0.0),
            tokens::scale.eq(1.0),
            tokens::created_at.eq(now),
            tokens::updated_at.eq(now),
        ))
        .execute(conn)
        .expect("insert token");
    id
}

pub fn ability(
    conn: &mut PgConnection,
    world_id: Uuid,
    gm: Uuid,
    name: &str,
    to_hit: &str,
    damage: &str,
) -> Uuid {
    let id = Uuid::now_v7();
    diesel::insert_into(world_abilities::table)
        .values((
            world_abilities::id.eq(id),
            world_abilities::world_id.eq(world_id),
            world_abilities::name.eq(name),
            world_abilities::classification.eq("feat"),
            world_abilities::created_by.eq(gm),
            world_abilities::updated_by.eq(gm),
        ))
        .execute(conn)
        .expect("insert ability");
    for (order, (kind, formula)) in [("attack_roll", to_hit), ("damage", damage)]
        .into_iter()
        .enumerate()
    {
        diesel::insert_into(world_ability_effects::table)
            .values((
                world_ability_effects::id.eq(Uuid::now_v7()),
                world_ability_effects::ability_id.eq(id),
                world_ability_effects::effect_type.eq(kind),
                world_ability_effects::formula.eq(formula),
                world_ability_effects::target.eq("one creature"),
                world_ability_effects::sort_order.eq(order as i32),
            ))
            .execute(conn)
            .expect("insert effect");
    }
    id
}

pub fn attach(conn: &mut PgConnection, actor_id: Uuid, ability_id: Uuid) {
    diesel::insert_into(world_actor_abilities::table)
        .values((
            world_actor_abilities::id.eq(Uuid::now_v7()),
            world_actor_abilities::actor_id.eq(actor_id),
            world_actor_abilities::ability_id.eq(Some(ability_id)),
            world_actor_abilities::ability_name_snapshot.eq("x"),
        ))
        .execute(conn)
        .expect("attach ability");
}

/// Aria at the origin, the goblin two squares east, the ogre four east. No
/// walls, daylight: everybody sees everybody until a test says otherwise.
pub fn table(conn: &mut PgConnection) -> FightTable {
    let gm = insert_test_user(conn);
    let player = insert_test_user(conn);
    let stranger = insert_test_user(conn);
    let world_id = insert_test_world(conn, gm);
    diesel::update(worlds::table.filter(worlds::id.eq(world_id)))
        .set(worlds::game_system_id.eq(Some("dnd5e")))
        .execute(conn)
        .expect("set system");
    insert_test_world_member(conn, world_id, player, "Player");
    insert_test_world_member(conn, world_id, stranger, "Player");
    let scene_id = insert_test_scene(conn, world_id, gm);
    let t = (world_id, scene_id, gm);

    let aria_actor = actor(conn, t, "Aria", false, ARIA_HP, ARIA_AC);
    let goblin_actor = actor(conn, t, "Goblin", true, GOBLIN_HP, GOBLIN_AC);
    let ogre_actor = actor(conn, t, OGRE_NAME, true, OGRE_HP, OGRE_AC);

    let aria = place(
        conn,
        scene_id,
        Some(aria_actor),
        Some(player),
        true,
        None,
        "Aria",
        (0.0, 0.0),
    );
    let goblin = place(
        conn,
        scene_id,
        Some(goblin_actor),
        Some(gm),
        false,
        Some(json!({ "current_hp": GOBLIN_HP, "max_hp": GOBLIN_HP, "temporary_hp": 0 })),
        "Goblin",
        (100.0, 0.0),
    );
    let ogre = place(
        conn,
        scene_id,
        Some(ogre_actor),
        Some(gm),
        true,
        None,
        OGRE_NAME,
        (200.0, 0.0),
    );

    let longsword = ability(conn, world_id, gm, "Longsword", "1d20+100", "5");
    let whiff = ability(conn, world_id, gm, "Whiff", "1d20-100", "5");
    let greatclub = ability(conn, world_id, gm, "Greatclub", "1d20+100", "9");
    attach(conn, aria_actor, longsword);
    attach(conn, aria_actor, whiff);
    attach(conn, ogre_actor, greatclub);

    FightTable {
        world_id,
        scene_id,
        gm,
        player,
        stranger,
        aria_actor,
        aria,
        goblin_actor,
        goblin,
        ogre_actor,
        ogre,
        longsword,
        whiff,
        greatclub,
    }
}

/// A running fight in the scene with the given combatants, on `active`'s turn.
pub fn fight(
    conn: &mut PgConnection,
    t: &FightTable,
    combatants: &[(Uuid, &str)],
    active: Uuid,
) -> Uuid {
    let combat_id = Uuid::now_v7();
    diesel::insert_into(world_combats::table)
        .values(&crate::models::NewCombat {
            id: combat_id,
            world_id: t.world_id,
            scene_id: Some(t.scene_id),
            created_by: t.gm,
        })
        .execute(conn)
        .expect("start combat");
    let mut active_id = None;
    for (index, (token, label)) in combatants.iter().enumerate() {
        let id = Uuid::now_v7();
        diesel::insert_into(world_combatants::table)
            .values(&crate::models::NewCombatant {
                id,
                combat_id,
                actor_id: None,
                token_id: Some(*token),
                label: (*label).into(),
                initiative: 20 - index as i32,
                tiebreak: 0,
                is_npc: false,
            })
            .execute(conn)
            .expect("add combatant");
        if *token == active {
            active_id = Some(id);
        }
    }
    diesel::update(world_combats::table.filter(world_combats::id.eq(combat_id)))
        .set(world_combats::active_combatant_id.eq(active_id))
        .execute(conn)
        .expect("set turn");
    combat_id
}

pub fn attack(
    conn: &mut PgConnection,
    user: Uuid,
    attacker: Uuid,
    ability: Uuid,
    target: Option<Uuid>,
) -> Result<MadeAttack, crate::combat::attack::FightRefusal> {
    make_attack(
        conn,
        SYSTEMS_DIR,
        user,
        false,
        &AttackRequest {
            attacker: crate::combat::attack::Attacker::Token(attacker),
            ability_id: Some(ability),
            target_token_id: target,
            ..Default::default()
        },
        &mut rng(),
    )
}

pub fn attack_row(conn: &mut PgConnection, id: Uuid) -> AttackRecord {
    world_attacks::table
        .filter(world_attacks::id.eq(id))
        .select(AttackRecord::as_select())
        .first(conn)
        .expect("attack row")
}

pub fn offer_row(conn: &mut PgConnection, id: Uuid) -> OfferRecord {
    world_offers::table
        .filter(world_offers::id.eq(id))
        .select(OfferRecord::as_select())
        .first(conn)
        .expect("offer row")
}

pub fn attacks_in(conn: &mut PgConnection, scene_id: Uuid) -> i64 {
    world_attacks::table
        .filter(world_attacks::scene_id.eq(scene_id))
        .count()
        .get_result(conn)
        .expect("count attacks")
}

pub fn offers_in(conn: &mut PgConnection, world_id: Uuid) -> i64 {
    world_offers::table
        .filter(world_offers::world_id.eq(world_id))
        .count()
        .get_result(conn)
        .expect("count offers")
}

/// A copy's current hit points.
pub fn copy_hp(conn: &mut PgConnection, token: Uuid) -> i64 {
    tokens::table
        .filter(tokens::token_id.eq(token))
        .select(tokens::system_data)
        .first::<Option<serde_json::Value>>(conn)
        .expect("token")
        .and_then(|d| d.get("current_hp").and_then(|v| v.as_i64()))
        .expect("current_hp")
}

/// An actor's current hit points.
pub fn actor_hp(conn: &mut PgConnection, actor: Uuid) -> i64 {
    world_actor_system_data::table
        .filter(world_actor_system_data::actor_id.eq(actor))
        .select(world_actor_system_data::resource_data)
        .first::<Option<serde_json::Value>>(conn)
        .expect("sheet")
        .and_then(|d| d.get("current_hp").and_then(|v| v.as_i64()))
        .expect("current_hp")
}

/// A vision-blocking wall from (x1, y1) to (x2, y2).
pub fn wall(conn: &mut PgConnection, t: &FightTable, from: (f64, f64), to: (f64, f64)) {
    let now = chrono::Utc::now().naive_utc();
    diesel::insert_into(walls::table)
        .values((
            walls::wall_id.eq(Uuid::now_v7()),
            walls::scene_id.eq(t.scene_id),
            walls::x1.eq(from.0),
            walls::y1.eq(from.1),
            walls::x2.eq(to.0),
            walls::y2.eq(to.1),
            walls::blocks_vision.eq(true),
            walls::blocks_movement.eq(true),
            walls::created_by.eq(t.gm),
            walls::updated_by.eq(t.gm),
            walls::created_at.eq(now),
            walls::updated_at.eq(now),
        ))
        .execute(conn)
        .expect("insert wall");
}

pub fn pause(conn: &mut PgConnection, t: &FightTable) {
    crate::play_pause::pause_world(
        conn,
        t.gm,
        t.world_id,
        "Stopping play.",
        crate::play_pause::TriggerDetail::operator(),
    )
    .expect("paused");
}
