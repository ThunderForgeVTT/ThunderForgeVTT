//! A Player cannot administer the world they play in.
//!
//! # Why this exists
//!
//! A Game Master on vtt-dev reported that their Players could see the world's
//! settings, its delete button and its invite-link generator. The dashboard
//! drew those controls for every member. These tests hold the server's half
//! of the line: every world-administration operation, sent through the real
//! schema by a `Player` member (and by a signed-in stranger), is refused and
//! changes nothing, while the same document sent by the world's owner works.
//!
//! The documents are sent through the schema rather than to the `_impl`
//! functions so that a resolver that forgot to call its `_impl`'s gate, or a
//! new root field that skipped one, is caught here too.

use async_graphql::Request;
use diesel::prelude::*;
use uuid::Uuid;

use crate::auth_middleware::AuthenticatedUser;
use crate::state::AppState;
use crate::test_support::{
    insert_test_user, insert_test_world, insert_test_world_member, test_app_state,
};

fn schema(state: AppState) -> crate::graphql::AppSchema {
    async_graphql::Schema::build(
        crate::graphql::QueryRoot::default(),
        crate::graphql::MutationRoot::default(),
        crate::graphql::SubscriptionRoot,
    )
    .data(state)
    .finish()
}

fn signed_in(user_id: Uuid) -> AuthenticatedUser {
    AuthenticatedUser {
        user_id,
        session_id: Uuid::now_v7(),
        expires_at: chrono::Utc::now().naive_utc() + chrono::Duration::hours(1),
        is_admin: false,
        role: "User".to_string(),
        disabled: false,
    }
}

/// Everything about the world that an administration operation could change,
/// read straight from the tables rather than through a resolver.
fn fingerprint(conn: &mut PgConnection, world: Uuid) -> String {
    use crate::schema::{world_authoring_tool_grants, world_invites, world_members, worlds};
    let row = worlds::table
        .find(world)
        .select((
            worlds::name,
            worlds::game_system_id,
            worlds::interface_pack_id,
            worlds::session_notes,
            worlds::allow_player_created_actors,
            worlds::allow_player_actor_art,
            worlds::genie_resource_carryover_enabled,
            worlds::default_scene_grid_type,
            worlds::auto_apply_npc_damage,
            worlds::updated_at,
        ))
        .first::<(
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            bool,
            bool,
            bool,
            String,
            bool,
            chrono::NaiveDateTime,
        )>(conn)
        .optional()
        .unwrap();
    let members = world_members::table
        .filter(world_members::world_id.eq(world))
        .order(world_members::user_id)
        .select((world_members::user_id, world_members::role))
        .load::<(Uuid, String)>(conn)
        .unwrap();
    let invites = world_invites::table
        .filter(world_invites::world_id.eq(world))
        .order(world_invites::id)
        .select((world_invites::id, world_invites::revoked))
        .load::<(Uuid, bool)>(conn)
        .unwrap();
    let grants = world_authoring_tool_grants::table
        .count()
        .get_result::<i64>(conn)
        .unwrap();
    format!("{row:?} {members:?} {invites:?} grants={grants}")
}

fn member_row_id(conn: &mut PgConnection, world: Uuid, user: Uuid) -> Uuid {
    use crate::schema::world_members;
    world_members::table
        .filter(world_members::world_id.eq(world))
        .filter(world_members::user_id.eq(user))
        .select(world_members::id)
        .first::<Uuid>(conn)
        .unwrap()
}

/// Every world-administration root field, with `{world}`, `{invite}`, `{gm}`,
/// `{player}` and `{player_member}` filled in per test.
const ADMINISTRATION: &[(&str, &str)] = &[
    (
        "deleteWorld",
        r#"mutation { deleteWorld(id: "{world}") { id } }"#,
    ),
    (
        "renameWorld",
        r#"mutation { renameWorld(worldId: "{world}", worldName: "Taken over") { id } }"#,
    ),
    (
        "updateWorldGameSystem",
        r#"mutation { updateWorldGameSystem(input: { worldId: "{world}", gameSystemId: "dnd5e" }) { id } }"#,
    ),
    (
        "updateWorldSessionNotes",
        r#"mutation { updateWorldSessionNotes(input: { worldId: "{world}", notes: "rewritten" }) { id } }"#,
    ),
    (
        "updateWorldInterfacePack",
        r#"mutation { updateWorldInterfacePack(input: { worldId: "{world}", interfacePackId: null }) { id } }"#,
    ),
    (
        "updateWorldAllowPlayerCreatedActors",
        r#"mutation { updateWorldAllowPlayerCreatedActors(input: { worldId: "{world}", allow: true }) { id } }"#,
    ),
    (
        "updateWorldAllowPlayerActorArt",
        r#"mutation { updateWorldAllowPlayerActorArt(input: { worldId: "{world}", allow: false }) { id } }"#,
    ),
    (
        "updateWorldGenieResourceCarryover",
        r#"mutation { updateWorldGenieResourceCarryover(input: { worldId: "{world}", enabled: true }) { id } }"#,
    ),
    (
        "updateWorldDefaultSceneGridType",
        r#"mutation { updateWorldDefaultSceneGridType(input: { worldId: "{world}", gridType: "hex" }) { id } }"#,
    ),
    (
        "updateWorldAutoApplyNpcDamage",
        r#"mutation { updateWorldAutoApplyNpcDamage(input: { worldId: "{world}", enabled: true }) { id } }"#,
    ),
    (
        "setWorldSystemSetting",
        r#"mutation { setWorldSystemSetting(worldId: "{world}", key: "anything", value: true) { key } }"#,
    ),
    (
        "generateInviteCode",
        r#"mutation { generateInviteCode(input: { worldId: "{world}", maxUses: 5 }) { inviteCode } }"#,
    ),
    (
        "revokeInviteCode",
        r#"mutation { revokeInviteCode(inviteId: "{invite}") { id } }"#,
    ),
    (
        "rotateInviteCode",
        r#"mutation { rotateInviteCode(inviteId: "{invite}") { id } }"#,
    ),
    (
        "updateMemberRole (promote self)",
        r#"mutation { updateMemberRole(input: { worldId: "{world}", userId: "{player}", role: "Owner" }) { role } }"#,
    ),
    (
        "updateMemberRole (demote the GM)",
        r#"mutation { updateMemberRole(input: { worldId: "{world}", userId: "{gm}", role: "Player" }) { role } }"#,
    ),
    (
        "removeMember",
        r#"mutation { removeMember(worldId: "{world}", userId: "{gm}") }"#,
    ),
    (
        "setAuthoringToolGrant",
        r#"mutation { setAuthoringToolGrant(worldId: "{world}", worldMemberId: "{player_member}", tool: "walls", granted: true) }"#,
    ),
    (
        "beginLoreRepositoryConnection",
        r#"mutation { beginLoreRepositoryConnection(worldId: "{world}") { __typename } }"#,
    ),
    (
        "removeLoreRepositoryConnection",
        r#"mutation { removeLoreRepositoryConnection(worldId: "{world}") }"#,
    ),
    (
        "worldInvites",
        r#"query { worldInvites(worldId: "{world}") { inviteCode } }"#,
    ),
];

struct Table {
    state: AppState,
    world: Uuid,
    owner: Uuid,
    gm: Uuid,
    player: Uuid,
    outsider: Uuid,
    invite: Uuid,
    player_member: Uuid,
}

async fn table() -> Table {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let world = insert_test_world(&mut conn, owner);
    let gm = insert_test_user(&mut conn);
    insert_test_world_member(&mut conn, world, gm, "GM");
    let player = insert_test_user(&mut conn);
    insert_test_world_member(&mut conn, world, player, "Player");
    let outsider = insert_test_user(&mut conn);
    let player_member = member_row_id(&mut conn, world, player);
    drop(conn);

    // The owner mints the link the revoke and rotate attempts aim at, through
    // the same schema — so the link is a real one.
    let response = schema(state.clone())
        .execute(
            Request::new(format!(
                r#"mutation {{ generateInviteCode(input: {{ worldId: "{world}", maxUses: 5 }}) {{ id }} }}"#
            ))
            .data(signed_in(owner)),
        )
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let json = serde_json::to_value(&response.data).unwrap();
    let invite = json["generateInviteCode"]["id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();

    Table {
        state,
        world,
        owner,
        gm,
        player,
        outsider,
        invite,
        player_member,
    }
}

fn fill(template: &str, t: &Table) -> String {
    template
        .replace("{world}", &t.world.to_string())
        .replace("{invite}", &t.invite.to_string())
        .replace("{gm}", &t.gm.to_string())
        .replace("{player_member}", &t.player_member.to_string())
        .replace("{player}", &t.player.to_string())
}

async fn assert_every_operation_refused(t: &Table, caller: Uuid, who: &str) {
    let schema = schema(t.state.clone());
    let before = fingerprint(&mut t.state.db_pool.get().unwrap(), t.world);

    for (operation, template) in ADMINISTRATION {
        let response = schema
            .execute(Request::new(fill(template, t)).data(signed_in(caller)))
            .await;
        let json = serde_json::to_value(&response).unwrap();
        assert!(
            !response.errors.is_empty(),
            "a {who} was allowed `{operation}`: {json}"
        );
        assert!(
            json["data"].is_null(),
            "`{operation}` answered a {who} with data: {json}"
        );
        // The refusal must be the permission gate, not an input the test got
        // wrong: an invalid input would refuse the owner too, and hide a hole.
        let message = response.errors[0].message.to_lowercase();
        assert!(
            [
                "forbidden",
                "only",
                "permission",
                "not a member",
                "not authorized",
                "not allowed",
                "owner",
            ]
            .iter()
            .any(|word| message.contains(word)),
            "`{operation}` refused a {who}, but not for lack of authority: {message}"
        );
        assert_eq!(
            fingerprint(&mut t.state.db_pool.get().unwrap(), t.world),
            before,
            "`{operation}` by a {who} changed the world"
        );
    }
}

#[tokio::test]
async fn a_player_cannot_administer_the_world() {
    let t = table().await;
    assert_every_operation_refused(&t, t.player, "Player").await;
}

#[tokio::test]
async fn a_stranger_cannot_administer_the_world() {
    let t = table().await;
    assert_every_operation_refused(&t, t.outsider, "non-member").await;
}

/// Deleting ends the world for everyone, so it stays with the Owner: a Game
/// Master, who runs the table, is refused too.
#[tokio::test]
async fn only_the_owner_deletes_the_world() {
    let t = table().await;
    let schema = schema(t.state.clone());
    let document = format!(r#"mutation {{ deleteWorld(id: "{}") {{ id }} }}"#, t.world);

    let by_gm = schema
        .execute(Request::new(document.clone()).data(signed_in(t.gm)))
        .await;
    assert!(!by_gm.errors.is_empty(), "a GM deleted the world");

    let by_owner = schema
        .execute(Request::new(document).data(signed_in(t.owner)))
        .await;
    assert!(by_owner.errors.is_empty(), "{:?}", by_owner.errors);
    assert_eq!(
        fingerprint(&mut t.state.db_pool.get().unwrap(), t.world)
            .split(' ')
            .next(),
        Some("None"),
        "the owner's delete left the world in place"
    );
}

/// The positive control for the invite operations: a Game Master may manage
/// links, so a refusal above is about the Player, not about the document.
#[tokio::test]
async fn a_game_master_manages_invite_links() {
    let t = table().await;
    let schema = schema(t.state.clone());
    for template in [
        r#"query { worldInvites(worldId: "{world}") { inviteCode } }"#,
        r#"mutation { generateInviteCode(input: { worldId: "{world}", maxUses: 5 }) { inviteCode } }"#,
        r#"mutation { revokeInviteCode(inviteId: "{invite}") { id } }"#,
    ] {
        let response = schema
            .execute(Request::new(fill(template, &t)).data(signed_in(t.gm)))
            .await;
        assert!(
            response.errors.is_empty(),
            "{template}: {:?}",
            response.errors
        );
    }
}
