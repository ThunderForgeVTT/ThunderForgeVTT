//! Replaying changes made while disconnected (spec 028 US7, T075–T078).
//!
//! `contracts/graphql-delta-sync.md` specifies this mutation; the rules it
//! adjudicates by live in [`thunderforge_cache_core::conflict`], which until
//! now had no caller anywhere in the tree.
//!
//! # What makes this safe to replay
//!
//! The client stores the emitted world-store command **verbatim** and sends
//! it back untouched. So a queued change is not a special kind of edit with
//! its own privileges — it is the same edit, arriving late, and it is
//! authorized here against **current** permissions rather than against
//! whatever the user had when they made it (FR-042). Someone removed from a
//! world while offline reconnects to find their queued edits refused, which
//! is the entire point: the alternative is a window in which revoked access
//! still writes.
//!
//! # Exactly one outcome per submitted change
//!
//! FR-041 prohibits silent loss, and the shape of the code is what enforces
//! it: outcomes are produced by mapping over the inputs, so an input without
//! an outcome is not something this function can express. Every early return
//! inside the loop produces a rejection rather than a `continue`.
//!
//! # Conflict bookkeeping, and its honest limit
//!
//! `conflict::resolve` needs to know what already landed on the same item and
//! who put it there. That memory lives in [`ReconcileMarks`], an in-process
//! map, and the limitation is stated rather than hidden: **a server restart
//! forgets it**, after which two players reconnecting either side of the
//! restart both apply and the later one silently wins on last-write. The
//! window this matters in is the minutes between one client reconnecting and
//! the next, and the cost of being wrong is a token position — recoverable,
//! visible, and re-doable.
//!
//! Making it durable means a table keyed by (world, item) with the winning
//! role and reconnect sequence, written in the same transaction as the edit.
//! That is a schema decision, and it is the right one if offline authoring
//! ever widens past FR-035a's token position/rotation/scale — at which point
//! the thing being lost stops being a position and starts being work.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use async_graphql::{Context, Enum, InputObject, Object, SimpleObject};
use diesel::prelude::*;
use serde::Deserialize;
use thunderforge_cache_core::conflict::{self, Contender, ReconnectSeq, Role, Winner};
use uuid::Uuid;

use crate::auth::world_membership::require_world_member;
use crate::graphql::{GraphQLResult, app_state, authenticated_user};
use crate::world_events::{EVENT_CODE_TOKEN_CHANGED, record_world_event};

/// Why a queued change did not stand. Mirrors
/// `thunderforge_cache_core::queue::RejectionReason`.
#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum GraphQLRejectionReason {
    /// The user may no longer make this change (FR-042).
    PermissionDenied,
    /// A conflicting change took precedence. Deliberately distinct from a
    /// generic failure: it is a working rule, not an error, and the UI has to
    /// be able to say which.
    Superseded,
    /// The thing being changed no longer exists.
    GoneAway,
    /// Malformed, or outside what may be edited offline (FR-035a).
    Invalid,
}

/// One change made while disconnected.
#[derive(InputObject)]
pub struct QueuedChangeInput {
    /// Client-generated correlation id. Echoed back so a client can match an
    /// outcome to the edit it made, without the server needing to understand
    /// the edit.
    pub local_id: String,
    /// The emitted world-store command, verbatim.
    pub command: async_graphql::Json<serde_json::Value>,
}

/// What became of one queued change.
#[derive(SimpleObject)]
pub struct GraphQLReconcileOutcome {
    pub local_id: String,
    pub applied: bool,
    pub reason: Option<GraphQLRejectionReason>,
    /// Set when `reason` is `SUPERSEDED`: who won, so the UI can say so
    /// rather than reporting an anonymous failure.
    pub superseded_by_role: Option<String>,
}

impl GraphQLReconcileOutcome {
    /// Named `accepted` rather than `applied`: `SimpleObject` generates a
    /// field accessor called `applied`, and a constructor of the same name
    /// collides with it.
    fn accepted(local_id: String) -> Self {
        Self {
            local_id,
            applied: true,
            reason: None,
            superseded_by_role: None,
        }
    }

    fn rejected(local_id: String, reason: GraphQLRejectionReason) -> Self {
        Self {
            local_id,
            applied: false,
            reason: Some(reason),
            superseded_by_role: None,
        }
    }

    fn superseded(local_id: String, by: Role) -> Self {
        Self {
            local_id,
            applied: false,
            reason: Some(GraphQLRejectionReason::Superseded),
            superseded_by_role: Some(role_name(by).to_string()),
        }
    }
}

fn role_name(role: Role) -> &'static str {
    match role {
        Role::GameMaster => "GameMaster",
        Role::Player => "Player",
    }
}

/// Who last won an item, and the reconnect that won it.
#[derive(Clone, Copy)]
struct Mark {
    role: Role,
    reconnect_seq: ReconnectSeq,
}

/// Per-world memory of what reconciliation has already settled.
#[derive(Default)]
struct ReconcileMarks {
    /// Monotonic, server-assigned, one per world. This is `ReconnectSeq`:
    /// deliberately a counter and never a clock, because a client timestamp
    /// is forgeable and a skewed one would silently overwrite other people's
    /// work — the exact failure a conflict rule exists to prevent.
    next_seq: HashMap<Uuid, ReconnectSeq>,
    /// The winner so far, per (world, token).
    marks: HashMap<(Uuid, Uuid), Mark>,
}

fn marks() -> &'static Mutex<ReconcileMarks> {
    static MARKS: OnceLock<Mutex<ReconcileMarks>> = OnceLock::new();
    MARKS.get_or_init(|| Mutex::new(ReconcileMarks::default()))
}

/// Take this reconnection's position in the order, for one world.
fn take_reconnect_seq(world_id: Uuid) -> ReconnectSeq {
    let Ok(mut state) = marks().lock() else {
        // A poisoned lock means another thread panicked mid-update. Handing
        // out 0 makes this reconnection sort first, which is the *generous*
        // reading — it can only lose to a GM, never silently beat an equal
        // peer that reconnected earlier.
        return 0;
    };
    let slot = state.next_seq.entry(world_id).or_insert(0);
    let seq = *slot;
    *slot = slot.saturating_add(1);
    seq
}

/// The FR-035a shape: a token move or manipulate, and nothing else.
///
/// Parsed structurally rather than by trusting a `type` field alone, because
/// the restriction is what keeps conflict resolution honest — precedence can
/// settle two positions, and cannot settle a create racing a delete without
/// destroying work someone cannot see was destroyed.
#[derive(Debug, Deserialize)]
struct UpsertTokenCommand {
    #[serde(rename = "type")]
    kind: String,
    token: CommandToken,
}

#[derive(Debug, Deserialize)]
struct CommandToken {
    id: Uuid,
    x: Option<f64>,
    y: Option<f64>,
    rotation: Option<f64>,
    scale: Option<f64>,
}

/// What a queued change is asking for, once it has been understood.
struct TokenEdit {
    token_id: Uuid,
    x: Option<f64>,
    y: Option<f64>,
    rotation: Option<f64>,
    scale: Option<f64>,
}

/// Interpret a queued command, refusing anything outside FR-035a.
///
/// Returns `None` for a command this mutation will not replay — which is a
/// rejection, never a silent skip.
fn parse_token_edit(command: &serde_json::Value) -> Option<TokenEdit> {
    let parsed: UpsertTokenCommand = serde_json::from_value(command.clone()).ok()?;
    if parsed.kind != "upsert_token" {
        return None;
    }
    // At least one of the permitted fields must be present, or this is a
    // no-op dressed as an edit.
    if parsed.token.x.is_none()
        && parsed.token.y.is_none()
        && parsed.token.rotation.is_none()
        && parsed.token.scale.is_none()
    {
        return None;
    }
    Some(TokenEdit {
        token_id: parsed.token.id,
        x: parsed.token.x,
        y: parsed.token.y,
        rotation: parsed.token.rotation,
        scale: parsed.token.scale,
    })
}

/// A world membership role, as `conflict` sees it.
///
/// Owner and GM both run the table, which is the same convention
/// `roleBadgeLabel` uses in the client and `ability_impl` uses server-side.
pub fn role_from_membership(member_role: &str) -> Role {
    match member_role {
        "Owner" | "GM" => Role::GameMaster,
        _ => Role::Player,
    }
}

#[derive(Default)]
pub struct ReconcileMutation;

#[Object]
impl ReconcileMutation {
    /// Replay changes made while disconnected, and report what became of
    /// each (US7).
    ///
    /// Applied in submitted order, so a client's own sequential edits do not
    /// reorder against each other — replaying "move right, then move up"
    /// backwards lands the token somewhere the user never put it.
    async fn reconcile_queued_changes(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
        changes: Vec<QueuedChangeInput>,
    ) -> GraphQLResult<Vec<GraphQLReconcileOutcome>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let user_id = auth_user.user_id;

        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| async_graphql::Error::new("Failed to get DB connection"))?;

        // FR-042, and the first half of why this is safe: membership is read
        // now, not when the edits were made. A user removed from the world
        // while offline gets every queued change refused.
        let member_role = match require_world_member(&mut conn, user_id, world_id) {
            Ok(role) => role,
            Err(_) => {
                // Every change rejected, one outcome each. Deliberately not
                // an error: the client needs the per-change verdicts to know
                // what to tell the user and what to stop replaying.
                return Ok(changes
                    .into_iter()
                    .map(|change| {
                        GraphQLReconcileOutcome::rejected(
                            change.local_id,
                            GraphQLRejectionReason::PermissionDenied,
                        )
                    })
                    .collect());
            }
        };
        let role = role_from_membership(&member_role);
        let reconnect_seq = take_reconnect_seq(world_id);

        let mut outcomes = Vec::with_capacity(changes.len());
        for change in changes {
            outcomes.push(apply_one(
                &mut conn,
                world_id,
                user_id,
                role,
                reconnect_seq,
                change,
            ));
        }
        Ok(outcomes)
    }
}

/// Adjudicate and apply one queued change, always answering with an outcome.
fn apply_one(
    conn: &mut PgConnection,
    world_id: Uuid,
    user_id: Uuid,
    role: Role,
    reconnect_seq: ReconnectSeq,
    change: QueuedChangeInput,
) -> GraphQLReconcileOutcome {
    let local_id = change.local_id;

    let Some(edit) = parse_token_edit(&change.command.0) else {
        return GraphQLReconcileOutcome::rejected(local_id, GraphQLRejectionReason::Invalid);
    };

    use crate::schema::tokens;
    let existing = tokens::table
        .filter(tokens::token_id.eq(edit.token_id))
        .select(crate::models::Token::as_select())
        .first::<crate::models::Token>(conn)
        .optional();

    let existing = match existing {
        Ok(Some(token)) => token,
        // Deleted server-side while the client was away. Discarded with a
        // reason rather than resurrected — recreating something someone
        // deliberately removed is the failure FR-035a's create/delete
        // restriction exists to avoid, and it would be perverse to do it here.
        Ok(None) => {
            return GraphQLReconcileOutcome::rejected(local_id, GraphQLRejectionReason::GoneAway);
        }
        Err(_) => {
            return GraphQLReconcileOutcome::rejected(local_id, GraphQLRejectionReason::Invalid);
        }
    };

    // The second half of FR-042. A Game Master may move anything in their
    // world; a player may move only what they own. Same rule `move_own_token`
    // applies to a live edit, asked again at reconnect time.
    let may_edit = match role {
        Role::GameMaster => true,
        Role::Player => existing.owner_user_id == Some(user_id),
    };
    if !may_edit {
        return GraphQLReconcileOutcome::rejected(
            local_id,
            GraphQLRejectionReason::PermissionDenied,
        );
    }

    // FR-040: who wins. `conflict::resolve` is shared with the client, which
    // predicts with it so the UI can say what will happen — the two answers
    // must never differ, which is why nothing here reimplements the rule.
    let incoming = Contender {
        role,
        reconnect_seq,
    };
    if let Some(previous) = existing_mark(world_id, edit.token_id) {
        let held = Contender {
            role: previous.role,
            reconnect_seq: previous.reconnect_seq,
        };
        if matches!(conflict::resolve(held, incoming), Winner::A) {
            return GraphQLReconcileOutcome::superseded(local_id, previous.role);
        }
    }

    let update = crate::models::TokenUpdate {
        actor_id: None,
        x: edit.x,
        y: edit.y,
        rotation: edit.rotation,
        scale: edit.scale,
        metadata: None,
        owner_user_id: None,
        is_primary: None,
        photo_url: None,
        // Not offline-editable (FR-035a), and explicitly `None` rather than
        // carried from the command: health is adjudicated by the ruleset, and
        // a client that queued a stale value while disconnected must not be
        // able to write it back hours later.
        health: None,
        max_health: None,
    };

    let updated = diesel::update(tokens::table.filter(tokens::token_id.eq(edit.token_id)))
        .set(&update)
        .execute(conn);

    if updated.is_err() {
        return GraphQLReconcileOutcome::rejected(local_id, GraphQLRejectionReason::Invalid);
    }

    remember_mark(world_id, edit.token_id, role, reconnect_seq);

    // An ordinary world event, deliberately: other clients learn about a
    // reconciled change through the subscription they already have, with no
    // special path to keep in step with the normal one.
    //
    // `reconciled`, `by_user` and `by_role` are what make the
    // `Applied → Superseded` case detectable on the client (FR-041). A player
    // whose change applied at their own reconnect is long gone from that call
    // by the time a GM reconnects and overrides it, so the only way they learn
    // is this event — and to recognise it as *their* work being overridden
    // rather than ordinary table activity, they need to know the change was a
    // replay and that somebody else made it.
    let _ = record_world_event(
        conn,
        world_id,
        EVENT_CODE_TOKEN_CHANGED,
        Some(serde_json::json!({
            "token_id": edit.token_id,
            "reconciled": true,
            "by_user": user_id,
            "by_role": role_name(role),
        })),
        user_id,
    );

    GraphQLReconcileOutcome::accepted(local_id)
}

fn existing_mark(world_id: Uuid, token_id: Uuid) -> Option<Mark> {
    marks()
        .lock()
        .ok()?
        .marks
        .get(&(world_id, token_id))
        .copied()
}

fn remember_mark(world_id: Uuid, token_id: Uuid, role: Role, reconnect_seq: ReconnectSeq) {
    if let Ok(mut state) = marks().lock() {
        state
            .marks
            .insert((world_id, token_id), Mark { role, reconnect_seq });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command(kind: &str, id: Uuid, x: Option<f64>) -> serde_json::Value {
        serde_json::json!({
            "type": kind,
            "token": { "id": id, "x": x, "y": 2.0 },
        })
    }

    #[test]
    fn a_token_move_is_understood() {
        let id = Uuid::now_v7();
        let edit = parse_token_edit(&command("upsert_token", id, Some(1.0)))
            .expect("a token move is exactly what this replays");

        assert_eq!(edit.token_id, id);
        assert_eq!(edit.x, Some(1.0));
        assert_eq!(edit.y, Some(2.0));
    }

    /// FR-035a. Anything that is not a token move/manipulate is refused, and
    /// refusal has to be a *rejection* rather than a skip — a skipped input
    /// would produce no outcome, which is the silent loss FR-041 prohibits.
    #[test]
    fn a_command_that_is_not_a_token_edit_is_refused() {
        let id = Uuid::now_v7();
        assert!(parse_token_edit(&command("remove_token", id, Some(1.0))).is_none());
        assert!(parse_token_edit(&serde_json::json!({"type": "create_wall"})).is_none());
        assert!(parse_token_edit(&serde_json::json!("not even an object")).is_none());
    }

    /// A command naming a token and changing nothing is not an edit. Applying
    /// it would take the conflict mark for an item nobody actually moved,
    /// which would then supersede someone's real change.
    #[test]
    fn an_edit_that_changes_nothing_is_refused() {
        let id = Uuid::now_v7();
        assert!(
            parse_token_edit(&serde_json::json!({
                "type": "upsert_token",
                "token": { "id": id },
            }))
            .is_none()
        );
    }

    /// Rotation and scale are permitted alongside position (FR-035a), so a
    /// manipulate replays as readily as a move.
    #[test]
    fn rotation_and_scale_are_permitted() {
        let id = Uuid::now_v7();
        let edit = parse_token_edit(&serde_json::json!({
            "type": "upsert_token",
            "token": { "id": id, "rotation": 90.0, "scale": 2.0 },
        }))
        .expect("rotation and scale are offline-editable");

        assert_eq!(edit.rotation, Some(90.0));
        assert_eq!(edit.scale, Some(2.0));
        assert_eq!(edit.x, None);
    }

    /// Owner and GM both run the table. A player is anything else, including
    /// a role string this server does not recognise — the safe reading of an
    /// unknown role is the least privileged one.
    #[test]
    fn owner_and_gm_are_both_game_masters() {
        assert_eq!(role_from_membership("Owner"), Role::GameMaster);
        assert_eq!(role_from_membership("GM"), Role::GameMaster);
        assert_eq!(role_from_membership("Player"), Role::Player);
        assert_eq!(role_from_membership("something-new"), Role::Player);
    }

    fn insert_token(conn: &mut PgConnection, scene_id: Uuid, owner: Option<Uuid>) -> Uuid {
        use crate::schema::tokens;
        let token_id = Uuid::now_v7();
        let now = chrono::Utc::now().naive_utc();
        diesel::insert_into(tokens::table)
            .values((
                tokens::token_id.eq(token_id),
                tokens::scene_id.eq(scene_id),
                tokens::x.eq(0.0_f64),
                tokens::y.eq(0.0_f64),
                tokens::owner_user_id.eq(owner),
                tokens::created_at.eq(now),
                tokens::updated_at.eq(now),
            ))
            .execute(conn)
            .expect("failed to insert test token");
        token_id
    }

    fn move_command(token_id: Uuid, x: f64) -> QueuedChangeInput {
        QueuedChangeInput {
            local_id: Uuid::now_v7().to_string(),
            command: async_graphql::Json(serde_json::json!({
                "type": "upsert_token",
                "token": { "id": token_id, "x": x, "y": 0.0 },
            })),
        }
    }

    fn token_x(conn: &mut PgConnection, token_id: Uuid) -> f64 {
        use crate::schema::tokens;
        tokens::table
            .filter(tokens::token_id.eq(token_id))
            .select(tokens::x)
            .first::<f64>(conn)
            .expect("token should exist")
    }

    /// T078/FR-041: every submitted change gets exactly one outcome, whatever
    /// happened to it. Silent loss is the prohibited failure, and the way it
    /// would appear is an input with no matching outcome.
    #[test]
    fn every_change_gets_exactly_one_outcome_including_the_bad_ones() {
        let state = crate::test_support::test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner = crate::test_support::insert_test_user(&mut conn);
        let world = crate::test_support::insert_test_world(&mut conn, owner);
        let scene = crate::test_support::insert_test_scene(&mut conn, world, owner);
        let token = insert_token(&mut conn, scene, Some(owner));
        let gone = Uuid::now_v7();

        let changes = vec![
            move_command(token, 5.0),
            move_command(gone, 7.0),
            QueuedChangeInput {
                local_id: Uuid::now_v7().to_string(),
                command: async_graphql::Json(serde_json::json!({"type": "delete_token"})),
            },
        ];
        let submitted: Vec<String> = changes.iter().map(|c| c.local_id.clone()).collect();

        let outcomes: Vec<GraphQLReconcileOutcome> = changes
            .into_iter()
            .map(|change| {
                apply_one(
                    &mut conn,
                    world,
                    owner,
                    Role::GameMaster,
                    take_reconnect_seq(world),
                    change,
                )
            })
            .collect();

        assert_eq!(outcomes.len(), submitted.len());
        let answered: Vec<String> = outcomes.iter().map(|o| o.local_id.clone()).collect();
        assert_eq!(answered, submitted, "outcomes must match inputs one for one");

        assert!(outcomes[0].applied, "a valid move by the owner applies");
        assert_eq!(
            outcomes[1].reason,
            Some(GraphQLRejectionReason::GoneAway),
            "a token deleted server-side is discarded with a reason, not resurrected"
        );
        assert_eq!(
            outcomes[2].reason,
            Some(GraphQLRejectionReason::Invalid),
            "a command outside FR-035a is rejected, never skipped"
        );
    }

    /// FR-042: a player may replay a move of their own token, and not of
    /// someone else's — asked at reconnect time, not at edit time.
    #[test]
    fn a_player_may_only_replay_moves_of_their_own_token() {
        let state = crate::test_support::test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner = crate::test_support::insert_test_user(&mut conn);
        let player = crate::test_support::insert_test_user(&mut conn);
        let world = crate::test_support::insert_test_world(&mut conn, owner);
        let scene = crate::test_support::insert_test_scene(&mut conn, world, owner);
        let theirs = insert_token(&mut conn, scene, Some(player));
        let someone_elses = insert_token(&mut conn, scene, Some(owner));

        let mine = apply_one(
            &mut conn,
            world,
            player,
            Role::Player,
            take_reconnect_seq(world),
            move_command(theirs, 3.0),
        );
        let not_mine = apply_one(
            &mut conn,
            world,
            player,
            Role::Player,
            take_reconnect_seq(world),
            move_command(someone_elses, 3.0),
        );

        assert!(mine.applied);
        assert_eq!(
            not_mine.reason,
            Some(GraphQLRejectionReason::PermissionDenied)
        );
        assert_eq!(
            token_x(&mut conn, someone_elses),
            0.0,
            "a refused replay must not have written anything"
        );
    }

    /// FR-040, the sharp edge: a player reconnects first and their change
    /// applies; the GM reconnects later with a conflicting edit and takes
    /// precedence. The GM's change is what stands.
    #[test]
    fn a_game_master_reconnecting_later_still_wins() {
        let state = crate::test_support::test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let gm = crate::test_support::insert_test_user(&mut conn);
        let player = crate::test_support::insert_test_user(&mut conn);
        let world = crate::test_support::insert_test_world(&mut conn, gm);
        let scene = crate::test_support::insert_test_scene(&mut conn, world, gm);
        let token = insert_token(&mut conn, scene, Some(player));

        let player_first = apply_one(
            &mut conn,
            world,
            player,
            Role::Player,
            take_reconnect_seq(world),
            move_command(token, 10.0),
        );
        assert!(player_first.applied, "the player got back first and applied");
        assert_eq!(token_x(&mut conn, token), 10.0);

        let gm_later = apply_one(
            &mut conn,
            world,
            gm,
            Role::GameMaster,
            take_reconnect_seq(world),
            move_command(token, 20.0),
        );

        assert!(gm_later.applied, "a GM beats a player regardless of order");
        assert_eq!(token_x(&mut conn, token), 20.0);
    }

    /// And the other direction, which is the case the UI has to explain: the
    /// GM reconnected first, so the player's queued edit is superseded rather
    /// than failing. `supersededByRole` is what lets the client say who won
    /// instead of reporting an anonymous error.
    #[test]
    fn a_player_losing_to_a_game_master_is_told_who_won() {
        let state = crate::test_support::test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let gm = crate::test_support::insert_test_user(&mut conn);
        let player = crate::test_support::insert_test_user(&mut conn);
        let world = crate::test_support::insert_test_world(&mut conn, gm);
        let scene = crate::test_support::insert_test_scene(&mut conn, world, gm);
        let token = insert_token(&mut conn, scene, Some(player));

        apply_one(
            &mut conn,
            world,
            gm,
            Role::GameMaster,
            take_reconnect_seq(world),
            move_command(token, 42.0),
        );

        let player_late = apply_one(
            &mut conn,
            world,
            player,
            Role::Player,
            take_reconnect_seq(world),
            move_command(token, 99.0),
        );

        assert!(!player_late.applied);
        assert_eq!(
            player_late.reason,
            Some(GraphQLRejectionReason::Superseded),
            "losing to a GM is a working rule, not an error"
        );
        assert_eq!(
            player_late.superseded_by_role.as_deref(),
            Some("GameMaster"),
            "the UI has to be able to say who won"
        );
        assert_eq!(
            token_x(&mut conn, token),
            42.0,
            "the GM's value stands"
        );
    }

    /// The sequence is per world and monotonic. Two worlds reconnecting do
    /// not share an ordering, and one world's sequence never repeats — a
    /// repeat would make two different reconnections tie, and
    /// `conflict::resolve` breaks a tie by favouring the *held* side, so the
    /// later client would lose without a rule ever having said so.
    #[test]
    fn reconnect_sequences_are_per_world_and_increasing() {
        let world_a = Uuid::now_v7();
        let world_b = Uuid::now_v7();

        let first = take_reconnect_seq(world_a);
        let second = take_reconnect_seq(world_a);
        let other = take_reconnect_seq(world_b);

        assert!(second > first, "a world's sequence must advance");
        assert_eq!(other, 0, "a different world starts its own ordering");
    }
}
