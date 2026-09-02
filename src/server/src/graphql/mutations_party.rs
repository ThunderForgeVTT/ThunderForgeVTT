//! Spec 031 (T055, US4/FR-019): bringing the party across a scene change.
//!
//! # Why tokens are created here rather than moved
//!
//! ADR-056 settles the shape and this module only carries it out: a token
//! belongs to exactly one scene (ADR-040), so "bring the party" means creating
//! tokens in the destination and leaving the previous scene's arrangement
//! alone. Nothing here may be read as moving a token between scenes, and
//! nothing may hold a token id across the change — the character is the
//! identity that survives.
//!
//! # The rule this mutation is answerable for
//!
//! ADR-056's second rule: a character that already has a token in the
//! destination gains no second one. A Game Master who goes tavern -> cellar,
//! back to the tavern, and down again must not find two of everybody. The
//! decision itself is `thunderforge_canvas_core::party` — pure set arithmetic
//! with its own tests — and this module's job is to hand it the truth and act
//! on its answer.
//!
//! # The race
//!
//! Server-authoritative means the client asking twice, or two Game Masters
//! asking at once, cannot produce a second token either. That is settled the
//! way `mutations_pickup.rs` settles two players grabbing one coin purse: at
//! the database. Two things do it together, and both are load-bearing —
//!
//! 1. the destination scene row is locked `FOR UPDATE` before the occupants
//!    are read, so the second transaction reads the party the first one
//!    actually created rather than the emptier scene it started from; and
//! 2. every insert is conditional on that character still having no token in
//!    the scene, so its row count — not the caller's earlier read — is what
//!    decides whether a token was created.
//!
//! A read-then-insert without the lock is the classic version of this bug: at
//! READ COMMITTED both transactions see an empty cellar and both fill it.
//!
//! # Why the loser is not an error
//!
//! Unlike a pickup, there is no loser. The operation's promise is per
//! character — "this character has exactly one token here" — and a second
//! request that creates nothing has kept it. So the answer reports what was
//! created and what was already present, and the second caller receives the
//! same characters under `alreadyPresent` rather than an
//! `ALREADY_TAKEN`-style refusal. A refusal would have the client report a
//! failure for a scene that is in exactly the state the Game Master asked for.

use async_graphql::{Context, Error, InputObject, Result as GraphQLResult, SimpleObject};
use diesel::prelude::*;
use diesel::result::Error as DieselError;
use diesel::sql_types::{Double, Timestamp, Uuid as SqlUuid};
use thunderforge_canvas_core::party::characters_to_create;
use uuid::Uuid;

use crate::graphql::{app_state, authenticated_user};
use crate::schema::{scenes, tokens, world_actors};
use crate::state::AppState;
use crate::world_events::{EVENT_CODE_TOKEN_CHANGED, record_world_event};

#[derive(InputObject, Debug, Clone)]
pub struct BringPartyToSceneInput {
    /// Where the party is arriving.
    pub scene_id: Uuid,
    /// The characters to bring, in the order they should be laid out.
    ///
    /// Omitted or empty means the whole party — every non-NPC character in
    /// the world, which is the same definition `whole_party` rewards use in
    /// `mutations_genie_session.rs`. A Game Master who says "bring the party"
    /// without listing anybody should not have to enumerate a table they
    /// already own.
    pub actor_ids: Option<Vec<Uuid>>,
}

/// What the destination looks like now, in the terms the caller asked in.
///
/// Deliberately character ids and not tokens. ADR-056 is explicit that token
/// ids do not survive a scene change, so handing them back would invite a
/// client to keep one; every consequence a caller can legitimately want —
/// what arrived, what was already standing there — is expressible about
/// characters.
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLPartyArrival {
    pub scene_id: Uuid,
    /// Characters that gained a token in this call.
    pub arrived_actor_ids: Vec<Uuid>,
    /// Characters that already had one, left exactly as they were.
    pub already_present_actor_ids: Vec<Uuid>,
}

/// Errors out of the transaction closure — the same shape and the same reason
/// as `mutations_pickup.rs`: `conn.transaction` needs `From<DieselError>`
/// because BEGIN and COMMIT can fail on their own.
#[derive(Debug)]
enum PartyError {
    Other(String),
}

impl From<DieselError> for PartyError {
    fn from(e: DieselError) -> Self {
        PartyError::Other(e.to_string())
    }
}

impl From<String> for PartyError {
    fn from(s: String) -> Self {
        PartyError::Other(s)
    }
}

/// Every character in the world that is not somebody the Game Master runs.
fn whole_party(conn: &mut PgConnection, world_id: Uuid) -> Result<Vec<Uuid>, DieselError> {
    world_actors::table
        .filter(world_actors::world_id.eq(world_id))
        .filter(world_actors::is_npc.eq(false))
        .order((world_actors::created_at.asc(), world_actors::id.asc()))
        .select(world_actors::id)
        .load::<Uuid>(conn)
}

/// The characters the destination already holds a token for.
fn destination_occupants(
    conn: &mut PgConnection,
    scene_id: Uuid,
) -> Result<Vec<Uuid>, DieselError> {
    tokens::table
        .filter(tokens::scene_id.eq(scene_id))
        .filter(tokens::actor_id.is_not_null())
        .select(tokens::actor_id)
        .load::<Option<Uuid>>(conn)
        .map(|rows| rows.into_iter().flatten().collect())
}

/// Where the *n*th arrival stands.
///
/// ADR-056's third rule: position is not carried. Two maps do not share a
/// coordinate system, so preserving x/y across the move would be a lie
/// dressed as a feature — the party would arrive inside a wall as often as
/// not. They line up on a grid step instead, in the order they were selected,
/// somewhere the Game Master can see them and drag them where they belong.
fn arrival_position(index: usize, grid_size: f64) -> (f64, f64) {
    const PER_ROW: usize = 5;
    let step = if grid_size > 0.0 { grid_size } else { 5.0 };
    let column = (index % PER_ROW) as f64;
    let row = (index / PER_ROW) as f64;
    (step * (column + 1.0), step * (row + 1.0))
}

/// Create one arrival's token, unless that character already has one here.
///
/// The `WHERE NOT EXISTS` is the arbiter, and its row count is the whole
/// answer — the same shape as `mutations_pickup.rs`'s conditional delete.
/// Written as one statement rather than a check followed by an insert because
/// only the single statement is decided by the database; the two-statement
/// version is decided by whatever this transaction happened to read a moment
/// earlier, which is exactly the thing under contention.
///
/// `owner_user_id` comes from the actor, never from the caller: a Game Master
/// bringing the party must not end up owning the players' tokens. Art is left
/// off deliberately — ADR-056 rule 1 hangs imagery on the actor, so copying a
/// URL onto the token here would freeze today's portrait onto a body that
/// outlives it.
fn create_arrival(
    conn: &mut PgConnection,
    scene_id: Uuid,
    actor_id: Uuid,
    owner_user_id: Uuid,
    position: (f64, f64),
) -> Result<bool, DieselError> {
    let now = chrono::Utc::now().naive_utc();
    let inserted = diesel::sql_query(
        "INSERT INTO tokens \
         (token_id, scene_id, actor_id, x, y, rotation, scale, token_type, \
          owner_user_id, created_at, updated_at) \
         SELECT $1, $2, $3, $4, $5, 0, 1, 'character', $6, $7, $7 \
         WHERE NOT EXISTS ( \
             SELECT 1 FROM tokens WHERE scene_id = $2 AND actor_id = $3 \
         )",
    )
    .bind::<SqlUuid, _>(Uuid::now_v7())
    .bind::<SqlUuid, _>(scene_id)
    .bind::<SqlUuid, _>(actor_id)
    .bind::<Double, _>(position.0)
    .bind::<Double, _>(position.1)
    .bind::<SqlUuid, _>(owner_user_id)
    .bind::<Timestamp, _>(now)
    .execute(conn)?;
    Ok(inserted > 0)
}

/// Testable core of `PartyMutation::bring_party_to_scene`.
///
/// Order matters:
///
/// 1. Lock the destination scene. Everything after this reads a scene no
///    other bring-the-party transaction can be changing underneath it.
/// 2. Check the caller is the Game Master of *that* scene — before any write,
///    so a refusal has changed nothing.
/// 3. Resolve the selection, ask `party::characters_to_create` what is
///    missing, and create exactly those.
pub async fn bring_party_to_scene_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    input: BringPartyToSceneInput,
) -> GraphQLResult<GraphQLPartyArrival> {
    let scene_id = input.scene_id;
    let requested = input.actor_ids.unwrap_or_default();

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let outcome = tokio::task::spawn_blocking(move || {
        conn.transaction(|conn| -> Result<GraphQLPartyArrival, PartyError> {
            // The lock, and the reason it is on the scene rather than on the
            // tokens: the tokens this call is deciding about are the ones that
            // do not exist yet, so there is no row to lock. The destination is
            // the thing two callers contend for, and it is already a row.
            let (world_id, grid_size) = scenes::table
                .filter(scenes::scene_id.eq(scene_id))
                .select((scenes::world_id, scenes::grid_size))
                .for_update()
                .first::<(Uuid, i32)>(conn)
                .optional()?
                .ok_or_else(|| "Scene not found".to_string())?;

            // 🔐 Bringing the party writes tokens onto a scene, which is
            // authoring, so it takes the gate every other content mutation on
            // a scene takes — the Owner or a GM, never a Player, and never
            // "whoever owns the characters".
            if !crate::auth::world_membership::is_dm_of_scene(conn, user_id, is_admin, scene_id)? {
                return Err("Only the DM (Owner or GM) may bring the party"
                    .to_string()
                    .into());
            }

            let selection = if requested.is_empty() {
                whole_party(conn, world_id)?
            } else {
                requested.clone()
            };

            // Every selected character is confirmed to be a player character
            // of *this* world before anything is created. Without this an
            // actor id from another world would become a token here, which no
            // check above would notice — the caller is a legitimate GM, just
            // not of that character.
            let party: Vec<Uuid> = world_actors::table
                .filter(world_actors::world_id.eq(world_id))
                .filter(world_actors::is_npc.eq(false))
                .filter(world_actors::id.eq_any(&selection))
                .select(world_actors::id)
                .load::<Uuid>(conn)?;

            if let Some(stranger) = selection.iter().find(|id| !party.contains(id)) {
                return Err(format!(
                    "Character {stranger} is not a player character in this world"
                )
                .into());
            }

            let occupants = destination_occupants(conn, scene_id)?;

            // The decision, made once, by the module that has tests for it.
            // Selection order survives, so the party lines up in the order the
            // Game Master picked, and a character named twice in one request
            // still gets one token.
            let selection_ids: Vec<String> = selection.iter().map(Uuid::to_string).collect();
            let occupant_ids: Vec<String> = occupants.iter().map(Uuid::to_string).collect();
            let to_create = characters_to_create(&selection_ids, &occupant_ids);
            let to_create: Vec<Uuid> = to_create
                .iter()
                .filter_map(|id| Uuid::parse_str(id).ok())
                .collect();

            let mut arrived = Vec::new();
            // Reported once each, in selection order: a character named twice
            // in one request is one character who was already here, and
            // saying so twice would make the count the caller displays wrong.
            let mut already_present: Vec<Uuid> = Vec::new();
            for actor_id in &selection {
                if occupants.contains(actor_id) && !already_present.contains(actor_id) {
                    already_present.push(*actor_id);
                }
            }

            for (index, actor_id) in to_create.iter().enumerate() {
                let owner_user_id: Uuid = world_actors::table
                    .filter(world_actors::id.eq(actor_id))
                    .select(world_actors::owned_by)
                    .first::<Uuid>(conn)?;

                if create_arrival(
                    conn,
                    scene_id,
                    *actor_id,
                    owner_user_id,
                    arrival_position(index, f64::from(grid_size)),
                )? {
                    arrived.push(*actor_id);
                } else {
                    // The conditional insert declined, so somebody else got
                    // there first. That is not a failure: the character has
                    // exactly one token in the destination, which is what was
                    // asked for.
                    already_present.push(*actor_id);
                }
            }

            if !arrived.is_empty() {
                crate::scene_fingerprint::refresh_scene_fingerprint(conn, scene_id, user_id);

                // One announcement for the arrival, not one per token. The
                // clients' job is to reload the scene's tokens, and saying so
                // six times makes six reloads out of one event.
                let _ = record_world_event(
                    conn,
                    world_id,
                    EVENT_CODE_TOKEN_CHANGED,
                    Some(serde_json::json!({
                        "action": "created",
                        "scene_id": scene_id,
                        "reason": "party_arrived",
                        "actor_ids": arrived,
                    })),
                    user_id,
                );
            }

            Ok(GraphQLPartyArrival {
                scene_id,
                arrived_actor_ids: arrived,
                already_present_actor_ids: already_present,
            })
        })
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?;

    match outcome {
        Ok(arrival) => Ok(arrival),
        Err(PartyError::Other(message)) => Err(Error::new(message)),
    }
}

#[derive(Default)]
pub struct PartyMutation;

#[async_graphql::Object]
impl PartyMutation {
    /// Bring the party's characters to a scene, creating a token for each one
    /// that does not already have one there.
    ///
    /// Idempotent per character: calling it twice leaves the same tokens
    /// standing as calling it once.
    async fn bring_party_to_scene(
        &self,
        ctx: &Context<'_>,
        input: BringPartyToSceneInput,
    ) -> GraphQLResult<GraphQLPartyArrival> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        bring_party_to_scene_impl(state, auth_user.user_id, auth_user.is_admin, input).await
    }
}

/// How many tokens a scene holds for one character. Tests assert on this
/// rather than on the mutation's return value, because "the character did not
/// get a second token" is a statement about the world, not about what a
/// mutation said.
#[cfg(test)]
fn tokens_for(conn: &mut PgConnection, scene_id: Uuid, actor_id: Uuid) -> i64 {
    tokens::table
        .filter(tokens::scene_id.eq(scene_id))
        .filter(tokens::actor_id.eq(actor_id))
        .count()
        .get_result::<i64>(conn)
        .expect("failed to count tokens")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{
        insert_test_scene, insert_test_scene_named, insert_test_user, insert_test_world,
        insert_test_world_member, test_app_state,
    };

    /// A player character — `insert_test_actor` makes NPCs, and an NPC is
    /// precisely what "bring the party" must not pick up.
    fn insert_test_pc(
        conn: &mut PgConnection,
        world_id: Uuid,
        scene_id: Uuid,
        owned_by: Uuid,
        label: &str,
    ) -> Uuid {
        let id = Uuid::now_v7();
        let now = chrono::Utc::now().naive_utc();
        diesel::insert_into(world_actors::table)
            .values((
                world_actors::id.eq(id),
                world_actors::world_id.eq(world_id),
                world_actors::scene_id.eq(scene_id),
                world_actors::actor_type.eq("character"),
                world_actors::game_system_id.eq("dnd5e"),
                world_actors::label.eq(label),
                world_actors::created_by.eq(owned_by),
                world_actors::owned_by.eq(owned_by),
                world_actors::is_public.eq(false),
                world_actors::is_npc.eq(false),
                world_actors::created_at.eq(now),
                world_actors::updated_at.eq(now),
            ))
            .execute(conn)
            .expect("failed to insert test player character");
        id
    }

    fn bring(scene_id: Uuid, actor_ids: Vec<Uuid>) -> BringPartyToSceneInput {
        BringPartyToSceneInput {
            scene_id,
            actor_ids: Some(actor_ids),
        }
    }

    /// FR-019: the party arrives, one token each, in the destination.
    #[tokio::test]
    async fn bringing_the_party_creates_one_token_per_character() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let gm = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, gm);
        let tavern = insert_test_scene(&mut conn, world_id, gm);
        let cellar = insert_test_scene_named(&mut conn, world_id, gm, "Cellar");
        let alice = insert_test_pc(&mut conn, world_id, tavern, gm, "Alice");
        let bob = insert_test_pc(&mut conn, world_id, tavern, gm, "Bob");
        drop(conn);

        let arrival = bring_party_to_scene_impl(&state, gm, false, bring(cellar, vec![alice, bob]))
            .await
            .expect("the world's owner may bring the party");
        assert_eq!(arrival.arrived_actor_ids, vec![alice, bob]);

        let mut conn = state.db_pool.get().unwrap();
        assert_eq!(tokens_for(&mut conn, cellar, alice), 1);
        assert_eq!(tokens_for(&mut conn, cellar, bob), 1);
    }

    /// ADR-056 rule 2, and the spec's edge case: tavern -> cellar -> tavern ->
    /// cellar leaves one token per character in the cellar, not two.
    #[tokio::test]
    async fn a_second_arrival_does_not_give_anybody_a_second_token() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let gm = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, gm);
        let tavern = insert_test_scene(&mut conn, world_id, gm);
        let cellar = insert_test_scene_named(&mut conn, world_id, gm, "Cellar");
        let alice = insert_test_pc(&mut conn, world_id, tavern, gm, "Alice");
        drop(conn);

        bring_party_to_scene_impl(&state, gm, false, bring(cellar, vec![alice]))
            .await
            .expect("first arrival");
        let second = bring_party_to_scene_impl(&state, gm, false, bring(cellar, vec![alice]))
            .await
            .expect("returning to a scene is not a failure");

        assert!(second.arrived_actor_ids.is_empty());
        assert_eq!(second.already_present_actor_ids, vec![alice]);

        let mut conn = state.db_pool.get().unwrap();
        assert_eq!(
            tokens_for(&mut conn, cellar, alice),
            1,
            "a character already in the destination must not gain a second token"
        );
    }

    /// The whole point of doing this at the database: two Game Masters asking
    /// at the same moment still leave one token per character.
    #[tokio::test]
    async fn concurrent_arrivals_create_exactly_one_token_each() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let gm = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, gm);
        let tavern = insert_test_scene(&mut conn, world_id, gm);
        let cellar = insert_test_scene_named(&mut conn, world_id, gm, "Cellar");
        let alice = insert_test_pc(&mut conn, world_id, tavern, gm, "Alice");
        let bob = insert_test_pc(&mut conn, world_id, tavern, gm, "Bob");

        // A second GM, so this is two callers rather than one caller twice —
        // the case a client-side guard could never cover.
        let other_gm = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, other_gm, "GM");
        drop(conn);

        let (first, second) = tokio::join!(
            bring_party_to_scene_impl(&state, gm, false, bring(cellar, vec![alice, bob])),
            bring_party_to_scene_impl(&state, other_gm, false, bring(cellar, vec![alice, bob])),
        );
        let first = first.expect("one GM's request must succeed");
        let second = second.expect("the other's must succeed too, having created nothing new");

        assert_eq!(
            first.arrived_actor_ids.len() + second.arrived_actor_ids.len(),
            2,
            "two characters must be created exactly once between the two calls"
        );

        let mut conn = state.db_pool.get().unwrap();
        assert_eq!(tokens_for(&mut conn, cellar, alice), 1);
        assert_eq!(tokens_for(&mut conn, cellar, bob), 1);
    }

    /// The same character named twice in one request is a slip in the caller,
    /// not a request for two tokens.
    #[tokio::test]
    async fn a_character_selected_twice_arrives_once() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let gm = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, gm);
        let tavern = insert_test_scene(&mut conn, world_id, gm);
        let cellar = insert_test_scene_named(&mut conn, world_id, gm, "Cellar");
        let alice = insert_test_pc(&mut conn, world_id, tavern, gm, "Alice");
        drop(conn);

        bring_party_to_scene_impl(&state, gm, false, bring(cellar, vec![alice, alice]))
            .await
            .expect("a duplicated selection is still a valid request");

        let mut conn = state.db_pool.get().unwrap();
        assert_eq!(tokens_for(&mut conn, cellar, alice), 1);
    }

    /// An empty selection means the whole party — and the whole party is the
    /// player characters, not the Game Master's cast.
    #[tokio::test]
    async fn an_unlisted_selection_brings_the_player_characters_only() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let gm = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, gm);
        let tavern = insert_test_scene(&mut conn, world_id, gm);
        let cellar = insert_test_scene_named(&mut conn, world_id, gm, "Cellar");
        let alice = insert_test_pc(&mut conn, world_id, tavern, gm, "Alice");
        let innkeeper = crate::test_support::insert_test_actor(&mut conn, world_id, tavern, gm);
        drop(conn);

        let arrival = bring_party_to_scene_impl(
            &state,
            gm,
            false,
            BringPartyToSceneInput {
                scene_id: cellar,
                actor_ids: None,
            },
        )
        .await
        .expect("the whole party is a valid request");
        assert_eq!(arrival.arrived_actor_ids, vec![alice]);

        let mut conn = state.db_pool.get().unwrap();
        assert_eq!(tokens_for(&mut conn, cellar, alice), 1);
        assert_eq!(
            tokens_for(&mut conn, cellar, innkeeper),
            0,
            "an NPC is not part of the party"
        );
    }

    /// Principle III: a Player asking creates nothing, and the destination is
    /// exactly as they found it.
    #[tokio::test]
    async fn a_player_may_not_bring_the_party() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let gm = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, gm);
        let tavern = insert_test_scene(&mut conn, world_id, gm);
        let cellar = insert_test_scene_named(&mut conn, world_id, gm, "Cellar");
        let alice = insert_test_pc(&mut conn, world_id, tavern, gm, "Alice");

        let player = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player, "Player");
        drop(conn);

        bring_party_to_scene_impl(&state, player, false, bring(cellar, vec![alice]))
            .await
            .expect_err("a Player may not move the table's tokens");

        let mut conn = state.db_pool.get().unwrap();
        assert_eq!(
            tokens_for(&mut conn, cellar, alice),
            0,
            "a refused request must leave the destination untouched"
        );
    }

    /// A character from another world cannot be smuggled in by id.
    #[tokio::test]
    async fn a_character_from_another_world_is_refused() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let gm = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, gm);
        let cellar = insert_test_scene(&mut conn, world_id, gm);

        let other_world = insert_test_world(&mut conn, gm);
        let other_scene = insert_test_scene(&mut conn, other_world, gm);
        let stranger = insert_test_pc(&mut conn, other_world, other_scene, gm, "Stranger");
        drop(conn);

        bring_party_to_scene_impl(&state, gm, false, bring(cellar, vec![stranger]))
            .await
            .expect_err("a character from another world is not this world's party");

        let mut conn = state.db_pool.get().unwrap();
        assert_eq!(tokens_for(&mut conn, cellar, stranger), 0);
    }

    /// The names the client sends, checked against the schema the server
    /// actually publishes. The web app's mutation is a string; a renamed
    /// field would otherwise fail for the first Game Master to press Launch
    /// with the party, not for the suite.
    #[test]
    fn the_mutation_is_registered_under_the_names_the_client_uses() {
        let schema = async_graphql::Schema::build(
            crate::graphql::QueryRoot::default(),
            crate::graphql::MutationRoot::default(),
            crate::graphql::SubscriptionRoot,
        )
        .finish();
        let sdl = schema.sdl();

        assert!(sdl.contains("bringPartyToScene("));
        assert!(sdl.contains("input BringPartyToSceneInput {"));
        assert!(sdl.contains("arrivedActorIds"));
        assert!(sdl.contains("alreadyPresentActorIds"));
    }

    /// The arrivals are laid out on the scene's grid rather than stacked on
    /// one square, which is what makes them draggable to where they belong.
    #[test]
    fn arrivals_line_up_on_the_grid() {
        assert_eq!(arrival_position(0, 5.0), (5.0, 5.0));
        assert_eq!(arrival_position(1, 5.0), (10.0, 5.0));
        assert_eq!(arrival_position(5, 5.0), (5.0, 10.0));
        // A scene with a nonsense grid still puts them somewhere separate,
        // rather than dividing the party by zero onto one square.
        assert_eq!(arrival_position(1, 0.0), (10.0, 5.0));
    }
}
