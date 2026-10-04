//! Owner decision 2026-09-15: a player cannot reach a hidden NPC by any query
//! path, including by its id, and reaches a shown one, or one they hold.
//!
//! The table is the fight fixture: a Game Master, a player (who plays Aria)
//! and a stranger who is also a member, one character and two NPCs, none of
//! them shown, which is what every NPC is after the migration.

use super::*;
use crate::combat::fixtures::{FightTable, OGRE_NAME, table};
use crate::graphql::mutations_actors::set_actor_visible_to_players_impl;
use crate::graphql::queries::actor::{
    actor_declared_values_impl, actor_sheet_values_impl, actor_system_data_impl,
    search_actors_impl, world_actors_impl,
};
use crate::test_support::test_app_state;

/// Every by-id read a player's client can make of an actor, answered for one
/// caller: `Ok` names the reads that succeeded, so a failure says which path
/// let the actor through.
async fn by_id_reads(state: &AppState, user: Uuid, actor: Uuid) -> Vec<&'static str> {
    let mut reached = Vec::new();
    if actor_system_data_impl(state, user, false, actor)
        .await
        .is_ok()
    {
        reached.push("actorSystemData");
    }
    if actor_sheet_values_impl(state, user, false, actor)
        .await
        .is_ok()
    {
        reached.push("actorSheet");
    }
    if actor_declared_values_impl(state, user, false, actor)
        .await
        .is_ok()
    {
        reached.push("actorDeclaredValues");
    }
    if crate::graphql::queries::inventory::actor_inventory_impl(state, user, false, actor)
        .await
        .is_ok()
    {
        reached.push("actorInventory");
    }
    if crate::graphql::mutations_actor_abilities::actor_abilities_impl(state, user, false, actor)
        .await
        .is_ok()
    {
        reached.push("actorAbilities");
    }
    reached
}

const ALL_BY_ID: [&str; 5] = [
    "actorSystemData",
    "actorSheet",
    "actorDeclaredValues",
    "actorInventory",
    "actorAbilities",
];

async fn listed(state: &AppState, user: Uuid, t: &FightTable) -> Vec<Uuid> {
    world_actors_impl(state, user, false, t.world_id)
        .await
        .expect("a member lists the world's actors")
        .into_iter()
        .map(|a| a.id)
        .collect()
}

async fn searched(state: &AppState, user: Uuid, t: &FightTable, query: &str) -> Vec<Uuid> {
    search_actors_impl(state, user, false, t.world_id, query)
        .await
        .expect("a member searches the world's actors")
        .into_iter()
        .map(|a| a.id)
        .collect()
}

#[tokio::test]
async fn a_hidden_npc_reaches_no_player_by_any_query_path_including_its_id() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    drop(conn);

    for player in [t.player, t.stranger] {
        let list = listed(&state, player, &t).await;
        assert!(
            list.contains(&t.aria_actor),
            "a character is listed for every member"
        );
        assert!(!list.contains(&t.ogre_actor), "a hidden NPC is not listed");
        assert!(
            !list.contains(&t.goblin_actor),
            "a hidden NPC is not listed"
        );

        assert!(
            searched(&state, player, &t, "Grukk").await.is_empty(),
            "a hidden NPC is not found by its name"
        );
        assert!(
            searched(&state, player, &t, "")
                .await
                .iter()
                .all(|id| *id == t.aria_actor),
            "an empty search lists only what may be seen"
        );

        assert_eq!(
            by_id_reads(&state, player, t.ogre_actor).await,
            Vec::<&str>::new(),
            "no by-id read reaches a hidden NPC"
        );
        let refused = actor_sheet_values_impl(&state, player, false, t.ogre_actor)
            .await
            .expect_err("hidden");
        let missing = actor_sheet_values_impl(&state, player, false, Uuid::now_v7())
            .await
            .expect_err("missing");
        assert_eq!(
            refused.message, missing.message,
            "a hidden NPC answers exactly as a missing actor does"
        );

        let targets = crate::graphql::queries::lore::lore_link_targets_impl(
            &state, player, false, t.world_id, "Gr",
        )
        .await
        .expect("link targets");
        assert!(
            targets
                .iter()
                .all(|target| target.id != t.ogre_actor && target.title != OGRE_NAME),
            "a hidden NPC is not offered as a lore link"
        );
    }

    // The Game Master sees everything.
    let list = listed(&state, t.gm, &t).await;
    assert!(list.contains(&t.ogre_actor) && list.contains(&t.goblin_actor));
    assert_eq!(
        searched(&state, t.gm, &t, "Grukk").await,
        vec![t.ogre_actor]
    );
    assert_eq!(
        by_id_reads(&state, t.gm, t.ogre_actor).await,
        ALL_BY_ID.to_vec()
    );
    // And a character is read by id by any member, as before.
    assert_eq!(
        by_id_reads(&state, t.stranger, t.aria_actor).await,
        ALL_BY_ID.to_vec()
    );
}

#[tokio::test]
async fn a_shown_npc_reaches_every_player_and_hiding_it_again_withdraws_it() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    drop(conn);

    set_actor_visible_to_players_impl(&state, t.gm, false, t.ogre_actor, true)
        .await
        .expect("the Game Master shows the ogre");

    assert!(listed(&state, t.player, &t).await.contains(&t.ogre_actor));
    assert_eq!(
        searched(&state, t.player, &t, "Grukk").await,
        vec![t.ogre_actor]
    );
    assert_eq!(
        by_id_reads(&state, t.player, t.ogre_actor).await,
        ALL_BY_ID.to_vec()
    );
    assert!(
        !listed(&state, t.player, &t).await.contains(&t.goblin_actor),
        "showing one NPC shows no other"
    );

    set_actor_visible_to_players_impl(&state, t.gm, false, t.ogre_actor, false)
        .await
        .expect("and hides it again");
    assert!(!listed(&state, t.player, &t).await.contains(&t.ogre_actor));
    assert!(by_id_reads(&state, t.player, t.ogre_actor).await.is_empty());
}

#[tokio::test]
async fn only_a_game_master_shows_an_npc_and_a_character_has_nothing_to_show() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    drop(conn);

    let refused = set_actor_visible_to_players_impl(&state, t.player, false, t.ogre_actor, true)
        .await
        .expect_err("a player may not show an NPC");
    assert_eq!(refused.message, "Actor not found");
    assert!(!listed(&state, t.stranger, &t).await.contains(&t.ogre_actor));

    set_actor_visible_to_players_impl(&state, t.gm, false, t.aria_actor, false)
        .await
        .expect_err("a character is seen by every player and cannot be hidden");
    assert!(listed(&state, t.stranger, &t).await.contains(&t.aria_actor));
}

#[tokio::test]
async fn a_player_who_holds_a_hidden_npc_still_reaches_it() {
    use crate::schema::world_actor_permissions;

    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);

    // A Viewer row is the default spelled out, and holds nothing.
    let now = chrono::Utc::now().naive_utc();
    diesel::insert_into(world_actor_permissions::table)
        .values((
            world_actor_permissions::id.eq(Uuid::now_v7()),
            world_actor_permissions::actor_id.eq(t.goblin_actor),
            world_actor_permissions::user_id.eq(t.stranger),
            world_actor_permissions::level.eq("Viewer"),
            world_actor_permissions::created_at.eq(now),
            world_actor_permissions::updated_at.eq(now),
        ))
        .execute(&mut conn)
        .expect("viewer grant");
    drop(conn);
    assert!(
        !listed(&state, t.stranger, &t)
            .await
            .contains(&t.goblin_actor)
    );
    assert!(
        by_id_reads(&state, t.stranger, t.goblin_actor)
            .await
            .is_empty()
    );

    // Owner on the goblin: the Game Master handed it to the stranger.
    let mut conn = state.db_pool.get().expect("conn");
    diesel::update(
        world_actor_permissions::table
            .filter(world_actor_permissions::actor_id.eq(t.goblin_actor))
            .filter(world_actor_permissions::user_id.eq(t.stranger)),
    )
    .set(world_actor_permissions::level.eq("Owner"))
    .execute(&mut conn)
    .expect("owner grant");
    // A token of the ogre the player owns: the ogre is theirs to move.
    crate::combat::fixtures::place(
        &mut conn,
        t.scene_id,
        Some(t.ogre_actor),
        Some(t.player),
        true,
        None,
        "Ogre",
        (300.0, 0.0),
    );
    drop(conn);

    let stranger_sees = listed(&state, t.stranger, &t).await;
    assert!(
        stranger_sees.contains(&t.goblin_actor),
        "an Owner grant holds it"
    );
    assert!(
        !stranger_sees.contains(&t.ogre_actor),
        "somebody else's token does not"
    );
    assert_eq!(
        by_id_reads(&state, t.stranger, t.goblin_actor).await,
        ALL_BY_ID.to_vec()
    );

    let player_sees = listed(&state, t.player, &t).await;
    assert!(
        player_sees.contains(&t.ogre_actor),
        "owning a token of it holds it"
    );
    assert!(!player_sees.contains(&t.goblin_actor));
    assert_eq!(
        searched(&state, t.player, &t, "Grukk").await,
        vec![t.ogre_actor]
    );
    assert_eq!(
        by_id_reads(&state, t.player, t.ogre_actor).await,
        ALL_BY_ID.to_vec()
    );
}

#[tokio::test]
async fn a_claim_holds_the_actor_it_names() {
    use crate::schema::{world_actor_claims, world_actors, world_members};

    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    // Claims are made on characters; a character later marked an NPC keeps
    // its claim, and its player keeps their character.
    let member: Uuid = world_members::table
        .filter(world_members::world_id.eq(t.world_id))
        .filter(world_members::user_id.eq(t.player))
        .select(world_members::id)
        .first(&mut conn)
        .expect("membership");
    diesel::insert_into(world_actor_claims::table)
        .values((
            world_actor_claims::id.eq(Uuid::now_v7()),
            world_actor_claims::actor_id.eq(t.aria_actor),
            world_actor_claims::world_member_id.eq(member),
        ))
        .execute(&mut conn)
        .expect("claim");
    diesel::update(world_actors::table.filter(world_actors::id.eq(t.aria_actor)))
        .set((
            world_actors::is_npc.eq(true),
            world_actors::actor_type.eq("npc"),
        ))
        .execute(&mut conn)
        .expect("now an NPC");
    drop(conn);

    assert!(listed(&state, t.player, &t).await.contains(&t.aria_actor));
    assert_eq!(
        by_id_reads(&state, t.player, t.aria_actor).await,
        ALL_BY_ID.to_vec()
    );
    assert!(!listed(&state, t.stranger, &t).await.contains(&t.aria_actor));
}

/// A lore entry naming a hidden NPC renders as an unresolved span for a
/// player, with no link to its id, and as a link once it is shown.
#[tokio::test]
async fn a_lore_link_to_a_hidden_npc_resolves_for_nobody_but_a_game_master() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    let body = format!("Beware [[{OGRE_NAME}]].");

    let (_, dm) = crate::markdown::links::extract_and_resolve(&mut conn, t.world_id, &body, true)
        .expect("resolves");
    assert_eq!(dm[0].target_actor_id, Some(t.ogre_actor));

    let (_, player) =
        crate::markdown::links::extract_and_resolve(&mut conn, t.world_id, &body, false)
            .expect("resolves");
    assert_eq!(player[0].target_kind, "unresolved");
    assert_eq!(player[0].target_actor_id, None);
    assert!(player[0].href.is_none());
    drop(conn);

    set_actor_visible_to_players_impl(&state, t.gm, false, t.ogre_actor, true)
        .await
        .expect("shown");
    let mut conn = state.db_pool.get().expect("conn");
    let (_, player) =
        crate::markdown::links::extract_and_resolve(&mut conn, t.world_id, &body, false)
            .expect("resolves");
    assert_eq!(player[0].target_actor_id, Some(t.ogre_actor));
}

/// Every NPC written by a path that does not decide is hidden: the column's
/// default is the migration's backfill.
#[tokio::test]
async fn an_npc_written_without_a_choice_is_hidden() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    let visible: bool = world_actors::table
        .filter(world_actors::id.eq(t.ogre_actor))
        .select(world_actors::visible_to_players)
        .first(&mut conn)
        .expect("ogre");
    assert!(!visible);
}

// ---------------------------------------------------------------------------
// Its tokens' names (owner decision 2026-09-15)
// ---------------------------------------------------------------------------

/// Every path that serves a token's name, asked about the ogre for one
/// viewer: `Ok` names the paths that said "Grukk the Ogre", so a failure says
/// which one let the name through.
fn names_reaching(conn: &mut PgConnection, t: &FightTable, user: Uuid) -> Vec<&'static str> {
    use crate::combat::fixtures::{OGRE_NAME, SYSTEMS_DIR, fight};
    use crate::combat::redaction::SceneSight;
    use crate::graphql::mutations_combat::{combat_world, load_combat};

    let runs_the_world =
        crate::auth::world_membership::actor_in_world(conn, user, false, t.world_id)
            .runs_the_world();
    let mut reached = Vec::new();

    // The board: the token list the canvas draws names from.
    let rows = tokens::table
        .filter(tokens::token_id.eq(t.ogre))
        .select(crate::models::Token::as_select())
        .load::<crate::models::Token>(conn)
        .expect("the ogre's token");
    let served = crate::graphql::token_art::tokens_with_art(conn, rows, runs_the_world)
        .expect("the board's tokens");
    // As text, so the question asked is the one that matters: does the name
    // appear anywhere in what this viewer is sent.
    if format!("{served:?}").contains(OGRE_NAME) {
        reached.push("sceneTokens");
    }

    // The combat tracker, and the refusal that names whose turn it is.
    let combat_id = fight(conn, t, &[(t.aria, "Aria"), (t.ogre, OGRE_NAME)], t.ogre);
    let row = combat_world(conn, combat_id).expect("combat");
    let tracker = load_combat(conn, SYSTEMS_DIR, row, user, false).expect("tracker");
    if tracker.combatants.iter().any(|c| c.label == OGRE_NAME) {
        reached.push("combat tracker");
    }
    let check =
        crate::combat::turn::turn_check(conn, t.scene_id, t.aria, user, false).expect("turn check");
    if check
        .refusal()
        .is_some_and(|sentence| sentence.contains(OGRE_NAME))
    {
        reached.push("out-of-turn refusal");
    }
    diesel::delete(
        crate::schema::world_combats::table.filter(crate::schema::world_combats::id.eq(combat_id)),
    )
    .execute(conn)
    .expect("end the fight");

    // The attack log's redaction.
    let sight = SceneSight::for_viewer(conn, SYSTEMS_DIR, user, false, t.scene_id).expect("sight");
    if !sight.party(Some(t.ogre), OGRE_NAME).redacted {
        reached.push("attack log");
    }

    reached
}

/// Every path a player can be told a name by.
const EVERY_NAME_PATH: [&str; 4] = [
    "sceneTokens",
    "combat tracker",
    "out-of-turn refusal",
    "attack log",
];

/// A Game Master is never refused for acting out of turn, so that one path
/// has no answer for one. The other three are all theirs.
const EVERY_GM_NAME_PATH: [&str; 3] = ["sceneTokens", "combat tracker", "attack log"];

/// The decision: one switch. The ogre's token says "Grukk the Ogre" and its
/// own `name_visible_to_players` is true — the default — and yet no player
/// reads it, because the creature it stands for is hidden. Remove the actor's
/// half of `player_may_read_token_name` and every path here hands the name
/// over.
#[test]
fn a_hidden_npcs_token_is_nameless_to_players_on_every_path() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    let own_switch: bool = tokens::table
        .filter(tokens::token_id.eq(t.ogre))
        .select(tokens::name_visible_to_players)
        .first(&mut conn)
        .expect("the ogre's token");
    assert!(
        own_switch,
        "the token's own switch is on, and still says nothing"
    );

    assert_eq!(
        names_reaching(&mut conn, &t, t.player),
        Vec::<&str>::new(),
        "a hidden NPC's name reaches no player by any path"
    );
    assert_eq!(
        names_reaching(&mut conn, &t, t.stranger),
        Vec::<&str>::new(),
        "nor a member who controls nothing"
    );
    assert_eq!(
        names_reaching(&mut conn, &t, t.gm),
        EVERY_GM_NAME_PATH.to_vec(),
        "and the Game Master reads it everywhere they can be told it"
    );
}

/// Showing the NPC hands the name back at once — the same reads, no reload.
/// And the token's own switch still has the last word over a shown creature.
#[tokio::test]
async fn showing_the_npc_names_its_token_and_the_tokens_own_switch_still_hides_it() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    drop(conn);

    set_actor_visible_to_players_impl(&state, t.gm, false, t.ogre_actor, true)
        .await
        .expect("the Game Master shows the ogre");

    let mut conn = state.db_pool.get().expect("conn");
    assert_eq!(
        names_reaching(&mut conn, &t, t.player),
        EVERY_NAME_PATH.to_vec(),
        "a shown NPC is named on every path"
    );

    diesel::update(tokens::table.filter(tokens::token_id.eq(t.ogre)))
        .set(tokens::name_visible_to_players.eq(false))
        .execute(&mut conn)
        .expect("hide this one token's name");
    assert_eq!(
        names_reaching(&mut conn, &t, t.player),
        Vec::<&str>::new(),
        "and a Game Master may still hide one token of a creature players know"
    );
}

/// And the nudges that make "at once" true: showing an NPC tells every board
/// to re-read its tokens, and the tracker to re-read its combatants. Without
/// them a revealed creature reads "Unknown" until a reload.
#[tokio::test]
async fn showing_an_npc_tells_every_board_and_tracker_to_re_read() {
    use crate::schema::world_events;
    use crate::world_events::{EVENT_CODE_COMBAT_CHANGED, EVENT_CODE_TOKEN_CHANGED};

    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    crate::combat::fixtures::fight(
        &mut conn,
        &t,
        &[
            (t.aria, "Aria"),
            (t.ogre, crate::combat::fixtures::OGRE_NAME),
        ],
        t.ogre,
    );
    drop(conn);

    set_actor_visible_to_players_impl(&state, t.gm, false, t.ogre_actor, true)
        .await
        .expect("shown");

    let mut conn = state.db_pool.get().expect("conn");
    let codes: Vec<i32> = world_events::table
        .filter(world_events::world_id.eq(t.world_id))
        .select(world_events::event_code)
        .load(&mut conn)
        .expect("events");
    assert!(
        codes.contains(&EVENT_CODE_TOKEN_CHANGED),
        "every board re-reads its tokens: {codes:?}"
    );
    assert!(
        codes.contains(&EVENT_CODE_COMBAT_CHANGED),
        "and the tracker its combatants: {codes:?}"
    );
}
