//! Spec 031 (T043, T044): taking a placed item off the map.
//!
//! # Why this is one mutation and not two
//!
//! A pickup is two writes — the token leaves the scene, and an inventory entry
//! appears — and the pair has no valid half. A token removed with nothing to
//! show for it is an item the table watched vanish; an entry created with the
//! token still lying there is an item duplicated by anyone who clicks twice.
//! So both happen inside one transaction, or neither does
//! (`contracts/graphql-mutations.md`, "Pick up a placed item").
//!
//! # Who decides
//!
//! ADR-054 lets the engine remove the token optimistically so the click feels
//! immediate. That is a picture, not a decision: this mutation is the only
//! authority on whether the item moved, and a refusal must leave the map and
//! every inventory exactly as they were (FR-017). Nothing below writes before
//! the refusal is ruled out.
//!
//! # The race
//!
//! Two players clicking the same coin purse in the same second must produce
//! one entry, not two (FR-016). That is settled the way spec 017 settles two
//! players claiming one character and the way `mutations_interactives.rs`
//! settles two players firing one `once` interactive: at the database, by a
//! conditional write whose row count is the answer. See `claim_token` below.

use async_graphql::{Context, Error, ErrorExtensions, InputObject, Result as GraphQLResult};
use diesel::prelude::*;
use diesel::result::Error as DieselError;
use uuid::Uuid;

use crate::auth::actor_permissions::require_actor_permission;
use crate::graphql::mutations_inventory::upsert_inventory_entry;
use crate::graphql::types::{ActorPermissionLevel, GraphQLInventoryEntry};
use crate::graphql::{app_state, authenticated_user};
use crate::models::ActorInventoryEntry;
use crate::schema::{interactives, tokens, world_actors};
use crate::state::AppState;
use crate::world_events::{EVENT_CODE_TOKEN_CHANGED, record_world_event, world_id_for_scene};

/// The effect id a placed item's interactive carries.
///
/// Declared as a literal here rather than imported from the item subsystem's
/// declaration because the two land independently (T041 is engine-side) and
/// the server must be able to honour a pickup either way. When the
/// declaration exists, this is the same string it publishes.
const PICKUP_EFFECT: &str = "item.pickup";

/// The extension code the client keys on to tell "it is gone" from "it broke".
///
/// A player who loses the race is told something true and specific — the item
/// is no longer there — and the client restores the token and says so. A
/// generic failure would have the client report a malfunction for what is
/// simply somebody else being quicker.
pub const ALREADY_TAKEN: &str = "ALREADY_TAKEN";

#[derive(InputObject, Debug, Clone)]
pub struct PickUpPlacedItemInput {
    /// The token lying on the scene.
    pub token_id: Uuid,
    /// The character receiving it.
    pub actor_id: Uuid,
}

/// What a placed item's authoring says it is.
#[derive(Debug, Clone, Copy)]
struct PlacedItem {
    item_id: Uuid,
    quantity: i32,
}

/// Reads the item a token stands for out of its authoring.
///
/// Two sources, in order of authority: the `item.pickup` interactive attached
/// to the token, and — for a token placed without one — the token's own
/// metadata. Both are read, rather than one being required, because a placed
/// item is authored through the interactives surface but the token is what a
/// player clicks; insisting on the interactive would make a perfectly ordinary
/// token that names an item unpickable for no reason a GM could see.
fn placed_item_of(
    conn: &mut PgConnection,
    token_id: Uuid,
) -> Result<Option<PlacedItem>, DieselError> {
    let from_interactive: Option<serde_json::Value> = interactives::table
        .filter(interactives::subject_ref.eq(token_id))
        .filter(interactives::effect_id.eq(PICKUP_EFFECT))
        .select(interactives::effect_config)
        .first::<Option<serde_json::Value>>(conn)
        .optional()?
        .flatten();

    if let Some(config) = from_interactive.as_ref()
        && let Some(placed) = read_placed_item(config)
    {
        return Ok(Some(placed));
    }

    let metadata: Option<serde_json::Value> = tokens::table
        .filter(tokens::token_id.eq(token_id))
        .select(tokens::metadata)
        .first::<Option<serde_json::Value>>(conn)?;

    Ok(metadata.as_ref().and_then(read_placed_item))
}

/// Pulls `(item, quantity)` out of an effect configuration or token metadata.
///
/// Accepts `item`, `itemId` and `item_id` for the reference. The first is the
/// key the effect declaration uses (the existing effects spell their fields
/// `destination` and `entry`); the other two are what a token's metadata is
/// written with elsewhere in the app. Reading all three costs nothing and
/// spares a GM an item that is on the map and cannot be picked up.
fn read_placed_item(config: &serde_json::Value) -> Option<PlacedItem> {
    let item_id = ["item", "itemId", "item_id"]
        .iter()
        .find_map(|key| config.get(*key)?.as_str())
        .and_then(|raw| Uuid::parse_str(raw).ok())?;

    // A quantity that is absent, unparseable or non-positive means one. A
    // placed item is a thing on the floor; zero of it is not a thing anyone
    // authored deliberately.
    let quantity = config
        .get("quantity")
        .and_then(serde_json::Value::as_i64)
        .filter(|n| *n > 0)
        .and_then(|n| i32::try_from(n).ok())
        .unwrap_or(1);

    Some(PlacedItem { item_id, quantity })
}

fn gone() -> Error {
    Error::new("That item is no longer there").extend_with(|_, ext| ext.set("code", ALREADY_TAKEN))
}

/// Take the token, if it is still on the map.
///
/// A conditional delete, and its row count is the whole answer. Both racing
/// transactions issue this; Postgres serialises them on the row, and the
/// second one to arrive finds nothing to delete and reports zero. Neither
/// caller has to be trusted about ordering, which is the point — the client
/// that clicked first is not necessarily the request that arrives first.
///
/// This runs *before* the inventory write in the transaction, so the loser
/// blocks and then loses without ever having touched an inventory.
fn claim_token(conn: &mut PgConnection, token_id: Uuid) -> Result<bool, DieselError> {
    let removed =
        diesel::delete(tokens::table.filter(tokens::token_id.eq(token_id))).execute(conn)?;
    Ok(removed > 0)
}

/// Errors out of the transaction closure. `conn.transaction` requires
/// `From<diesel::result::Error>` because the wrapper itself may fail to
/// BEGIN or COMMIT — the same reason `mutations_actor_claims.rs` has one.
#[derive(Debug)]
enum PickupError {
    /// The token was gone by the time this transaction reached it.
    Gone,
    Other(String),
}

impl From<DieselError> for PickupError {
    fn from(e: DieselError) -> Self {
        PickupError::Other(e.to_string())
    }
}

impl From<String> for PickupError {
    fn from(s: String) -> Self {
        PickupError::Other(s)
    }
}

/// Testable core of `PickupMutation::pick_up_placed_item`.
///
/// Order matters and is not incidental:
///
/// 1. Read what the token is and where it lives — no writes.
/// 2. Check the caller may act for the receiving character. A refusal here
///    has changed nothing, because nothing has been written.
/// 3. One transaction: claim the token, then write the entry.
pub async fn pick_up_placed_item_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    input: PickUpPlacedItemInput,
) -> GraphQLResult<ActorInventoryEntry> {
    let token_id = input.token_id;
    let actor_id = input.actor_id;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let (scene_id, placed) = tokio::task::spawn_blocking(move || {
        let scene_id: Option<Uuid> = tokens::table
            .filter(tokens::token_id.eq(token_id))
            .select(tokens::scene_id)
            .first::<Uuid>(&mut conn)
            .optional()?;
        let Some(scene_id) = scene_id else {
            return Ok::<_, DieselError>(None);
        };
        let placed = placed_item_of(&mut conn, token_id)?;
        Ok(Some((scene_id, placed)))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|e| Error::new(format!("Failed to read the placed item: {e}")))?
    // An item that was picked up a moment ago is not "not found" — it is
    // gone, which is a different thing to tell a player and the same answer
    // the loser of a race gets.
    .ok_or_else(gone)?;

    let placed = placed.ok_or_else(|| Error::new("That token is not a placed item"))?;

    // 🔐 Who may take a placed item into an inventory is the same question
    // spec 013 already answers for `addItemToInventory`: Editor on the
    // *receiving actor*, not on the item. A player picking up a sword needs
    // authority over their own character sheet, not over the sword.
    require_actor_permission(
        state,
        user_id,
        is_admin,
        actor_id,
        ActorPermissionLevel::Editor,
    )
    .await?;

    // The receiving character and the item must belong to one world. Without
    // this a token id from one world could seed an inventory in another,
    // which no permission check above would notice.
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let item_id = placed.item_id;
    let quantity = placed.quantity;

    let result = tokio::task::spawn_blocking(move || {
        conn.transaction(|conn| -> Result<(ActorInventoryEntry, Uuid), PickupError> {
            let world_id = world_id_for_scene(conn, scene_id)
                .map_err(|_| "Scene not found".to_string())?;

            let actor_world: Uuid = world_actors::table
                .filter(world_actors::id.eq(actor_id))
                .select(world_actors::world_id)
                .first::<Uuid>(conn)
                .map_err(|_| "Character not found".to_string())?;

            if actor_world != world_id {
                return Err("That character is not in this world".to_string().into());
            }

            // The arbiter. Everything after this line runs only for the one
            // caller who actually removed the token.
            if !claim_token(conn, token_id)? {
                return Err(PickupError::Gone);
            }

            // The interactive that made the token pickable goes with it.
            // Left behind it would be an activation target pointing at a
            // token nobody can see.
            diesel::delete(interactives::table.filter(interactives::subject_ref.eq(token_id)))
                .execute(conn)?;

            // The same write `addItemToInventory` performs, enlisted in this
            // transaction rather than reimplemented beside it.
            // `user_id` is the player who activated the pickup — the same
            // caller `require_actor_permission` above just authorised, and
            // the right answer to "who put this in the inventory".
            let entry = upsert_inventory_entry(conn, actor_id, item_id, quantity, user_id)?;

            crate::scene_fingerprint::refresh_scene_fingerprint(conn, scene_id, user_id);

            // Announced inside the transaction, so the news and the fact
            // commit together. A rolled-back pickup announces nothing.
            let _ = record_world_event(
                conn,
                world_id,
                EVENT_CODE_TOKEN_CHANGED,
                Some(serde_json::json!({
                    "action": "deleted",
                    "token_id": token_id,
                    "scene_id": scene_id,
                    "reason": "picked_up",
                })),
                user_id,
            );

            Ok((entry, world_id))
        })
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?;

    match result {
        Ok((entry, _)) => Ok(entry),
        Err(PickupError::Gone) => Err(gone()),
        Err(PickupError::Other(message)) => Err(Error::new(message)),
    }
}

#[derive(Default)]
pub struct PickupMutation;

#[async_graphql::Object]
impl PickupMutation {
    /// Take a placed item off the map and into a character's inventory.
    ///
    /// All-or-nothing, and exactly one caller wins a contested item.
    async fn pick_up_placed_item(
        &self,
        ctx: &Context<'_>,
        input: PickUpPlacedItemInput,
    ) -> GraphQLResult<GraphQLInventoryEntry> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        pick_up_placed_item_impl(state, auth_user.user_id, auth_user.is_admin, input)
            .await
            .map(GraphQLInventoryEntry::from)
    }
}

/// How many inventory entries exist for one `(actor, item)` pair, and their
/// total quantity. Tests assert on this rather than on a mutation's return
/// value, because "exactly one player got it" is a statement about the table.
#[cfg(test)]
fn inventory_totals(conn: &mut PgConnection, item_id: Uuid) -> (usize, i64) {
    use crate::schema::world_actor_inventory;
    let rows: Vec<i32> = world_actor_inventory::table
        .filter(world_actor_inventory::item_id.eq(item_id))
        .select(world_actor_inventory::quantity)
        .load::<i32>(conn)
        .expect("failed to read inventory");
    let total = rows.iter().map(|q| i64::from(*q)).sum();
    (rows.len(), total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{
        insert_test_actor, insert_test_item, insert_test_scene, insert_test_user,
        insert_test_world, insert_test_world_member, test_app_state,
    };

    /// Places `item_id` on `scene_id` as a token whose metadata names it.
    fn place_item(
        conn: &mut PgConnection,
        scene_id: Uuid,
        item_id: Uuid,
        quantity: i32,
    ) -> Uuid {
        let token_id = Uuid::now_v7();
        let now = chrono::Utc::now().naive_utc();
        diesel::insert_into(tokens::table)
            .values((
                tokens::token_id.eq(token_id),
                tokens::scene_id.eq(scene_id),
                tokens::x.eq(1.0),
                tokens::y.eq(2.0),
                tokens::rotation.eq(0.0),
                tokens::scale.eq(1.0),
                tokens::token_type.eq("object"),
                tokens::metadata.eq(Some(serde_json::json!({
                    "item": item_id.to_string(),
                    "quantity": quantity,
                }))),
                tokens::created_at.eq(now),
                tokens::updated_at.eq(now),
            ))
            .execute(conn)
            .expect("failed to place test item token");
        token_id
    }

    fn token_exists(conn: &mut PgConnection, token_id: Uuid) -> bool {
        tokens::table
            .filter(tokens::token_id.eq(token_id))
            .count()
            .get_result::<i64>(conn)
            .expect("failed to count tokens")
            > 0
    }

    /// FR-015: the token leaves the map and the entry appears, together.
    #[tokio::test]
    async fn pickup_removes_the_token_and_creates_one_entry() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let actor_id = insert_test_actor(&mut conn, world_id, scene_id, owner_id);
        let item_id = insert_test_item(&mut conn, world_id, owner_id);
        let token_id = place_item(&mut conn, scene_id, item_id, 2);
        drop(conn);

        let entry = pick_up_placed_item_impl(
            &state,
            owner_id,
            false,
            PickUpPlacedItemInput { token_id, actor_id },
        )
        .await
        .expect("the world's owner may pick up a placed item");

        assert_eq!(entry.item_id, Some(item_id));
        assert_eq!(entry.quantity, 2, "the authored quantity is what arrives");

        let mut conn = state.db_pool.get().unwrap();
        assert!(
            !token_exists(&mut conn, token_id),
            "a successful pickup must remove the token from the scene"
        );
        assert_eq!(inventory_totals(&mut conn, item_id), (1, 2));
    }

    /// FR-016 / SC-006: two pickups of one placed item, genuinely
    /// concurrent, yield exactly one inventory entry — and the loser is told
    /// the item is gone rather than that something broke.
    #[tokio::test]
    async fn concurrent_pickups_yield_exactly_one_entry() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let item_id = insert_test_item(&mut conn, world_id, owner_id);
        let token_id = place_item(&mut conn, scene_id, item_id, 1);

        // Two different characters, so a double-award would show up as two
        // rows rather than one row of quantity 2 — either would be a bug,
        // and this makes the more likely one impossible to miss.
        let first_actor = insert_test_actor(&mut conn, world_id, scene_id, owner_id);
        let second_actor = insert_test_actor(&mut conn, world_id, scene_id, owner_id);
        drop(conn);

        let attempts = [first_actor, second_actor].map(|actor_id| {
            let state = state.clone();
            tokio::spawn(async move {
                pick_up_placed_item_impl(
                    &state,
                    owner_id,
                    false,
                    PickUpPlacedItemInput { token_id, actor_id },
                )
                .await
            })
        });

        let mut winners = 0;
        let mut gone_refusals = 0;
        for attempt in attempts {
            match attempt.await.expect("pickup task must not panic") {
                Ok(_) => winners += 1,
                Err(e) => {
                    let code = e
                        .extensions
                        .as_ref()
                        .and_then(|ext| ext.get("code"))
                        .map(|v| format!("{v:?}"))
                        .unwrap_or_default();
                    assert!(
                        code.contains(ALREADY_TAKEN),
                        "the loser must be told the item is gone, not handed a \
                         generic failure; got {e:?}"
                    );
                    gone_refusals += 1;
                }
            }
        }

        assert_eq!(winners, 1, "exactly one caller may win a contested item");
        assert_eq!(gone_refusals, 1);

        let mut conn = state.db_pool.get().unwrap();
        assert_eq!(
            inventory_totals(&mut conn, item_id),
            (1, 1),
            "one placed item must become exactly one inventory entry"
        );
        assert!(!token_exists(&mut conn, token_id));
    }

    /// The guard itself, isolated: a second conditional delete against an
    /// already-taken token affects zero rows. The test above proves the race
    /// end to end; this one proves *why* it holds, and keeps failing for a
    /// readable reason if the delete is ever softened into a read-then-write.
    #[tokio::test]
    async fn the_second_claim_of_a_token_affects_zero_rows() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let item_id = insert_test_item(&mut conn, world_id, owner_id);
        let token_id = place_item(&mut conn, scene_id, item_id, 1);

        assert!(claim_token(&mut conn, token_id).unwrap());
        assert!(
            !claim_token(&mut conn, token_id).unwrap(),
            "the second claim must find nothing to take"
        );
    }

    /// FR-017: a caller without authority over the receiving character
    /// changes nothing — the token stays on the map and no inventory moves.
    #[tokio::test]
    async fn a_refused_pickup_leaves_the_map_and_inventories_untouched() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let actor_id = insert_test_actor(&mut conn, world_id, scene_id, owner_id);
        let item_id = insert_test_item(&mut conn, world_id, owner_id);
        let token_id = place_item(&mut conn, scene_id, item_id, 1);

        // A Player with no explicit grant resolves to Viewer on the actor,
        // which is below the Editor this mutation requires.
        let player_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");
        drop(conn);

        let refusal = pick_up_placed_item_impl(
            &state,
            player_id,
            false,
            PickUpPlacedItemInput { token_id, actor_id },
        )
        .await
        .expect_err("a Viewer on the character may not take an item for it");
        assert!(
            !refusal.message.contains("no longer there"),
            "a permission refusal must not masquerade as the item being gone"
        );

        let mut conn = state.db_pool.get().unwrap();
        assert!(
            token_exists(&mut conn, token_id),
            "a refused pickup must leave the token on the map"
        );
        assert_eq!(
            inventory_totals(&mut conn, item_id),
            (0, 0),
            "a refused pickup must leave every inventory untouched"
        );
    }

    /// Picking up something that has already been taken is "it is gone",
    /// with the same code the race's loser receives — not "not found".
    #[tokio::test]
    async fn picking_up_an_already_taken_item_reports_it_gone() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let actor_id = insert_test_actor(&mut conn, world_id, scene_id, owner_id);
        let item_id = insert_test_item(&mut conn, world_id, owner_id);
        let token_id = place_item(&mut conn, scene_id, item_id, 1);
        drop(conn);

        pick_up_placed_item_impl(
            &state,
            owner_id,
            false,
            PickUpPlacedItemInput { token_id, actor_id },
        )
        .await
        .expect("first pickup succeeds");

        let refusal = pick_up_placed_item_impl(
            &state,
            owner_id,
            false,
            PickUpPlacedItemInput { token_id, actor_id },
        )
        .await
        .expect_err("the second pickup of the same placed item must be refused");

        let code = refusal
            .extensions
            .as_ref()
            .and_then(|ext| ext.get("code"))
            .map(|v| format!("{v:?}"))
            .unwrap_or_default();
        assert!(
            code.contains(ALREADY_TAKEN),
            "an item already taken is reported as gone; got {refusal:?}"
        );

        let mut conn = state.db_pool.get().unwrap();
        assert_eq!(
            inventory_totals(&mut conn, item_id),
            (1, 1),
            "the refused second pickup must not top the entry up"
        );
    }
}
