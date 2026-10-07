//! Shared PostgreSQL LISTEN/NOTIFY backplane helper for real-time sync.
//!
//! Records an audit-trail row in `world_events` and triggers `pg_notify`
//! on the `world_events_channel`, mirroring the pattern already used by
//! token upserts (crates/thunderforge-server/src/graphql.rs) and invite/membership
//! mutations (crates/thunderforge-server/src/graphql/mutations_invites.rs). Centralized
//! here because the canvas-authoring mutations (walls, light sources,
//! shapes, map import) are the first callers outside those two modules.

use async_graphql::{Error, Result as GraphQLResult};
use chrono::Utc;
use diesel::PgConnection;
use diesel::prelude::*;
use uuid::Uuid;

use crate::schema::world_events;

/// Event codes for the `world_events` audit trail / NOTIFY payload.
/// 1-5 are already used by token sync and invite/membership mutations
/// (crates/thunderforge-server/src/graphql.rs, crates/thunderforge-server/src/graphql/mutations_invites.rs).
/// 10 (wall), 11 (light), 12 (shape), 13 (map import), 14 (token) are
/// documented in `apps/web/src/engine/world/sync/tokens.ts`'s doc comment;
/// 15 (spec 018, Genie session state) is documented there too.
pub const EVENT_CODE_WALL_CHANGED: i32 = 10;
pub const EVENT_CODE_LIGHT_SOURCE_CHANGED: i32 = 11;
pub const EVENT_CODE_SHAPE_CHANGED: i32 = 12;
pub const EVENT_CODE_MAP_IMPORTED: i32 = 13;
pub const EVENT_CODE_TOKEN_CHANGED: i32 = 14;
/// Spec 018 (User Story 7): Genie session-loop state changes — the
/// Session Wish Pool, Doom Clock, Puzzle Clocks, and Session Resource
/// trades (data-model.md `world_events`). Reuses this existing generic
/// broadcast mechanism rather than a dedicated subscription (research.md
/// R7); consumed by the same `worldEventsCreated(worldId)` subscription
/// every world-member client already holds open. Payload shape
/// (`token_event` JSON column):
/// `{ "kind": "wish_pool" | "doom_clock" | "puzzle_clock" | "resource_trade" | "resource_grant" | "purchase" | "clock_reward", "session_id": "...", ...kind-specific fields }`
/// (contracts/genie-session-loop.md; the last three kinds added by spec
/// 020, contracts/genie-economy.md).
pub const EVENT_CODE_GENIE_SESSION_STATE: i32 = 15;
/// Spec 022 (Scene Management Overhaul, ADR-046): a GM launched a scene
/// (`worlds.active_scene_id` changed). Payload: `{ "sceneId": "..." }`.
/// Consumed by every world member's already-open `worldEventsCreated`
/// subscription while in Play to live-switch which scene is loaded.
pub const EVENT_CODE_SCENE_LAUNCHED: i32 = 16;
/// Play-view Chat: a message was posted to this world. Payload:
/// `{ "messageId": "..." }` — the nudge only; the client refetches the
/// backscroll rather than trusting a body delivered over the bus, so a
/// GM-only message never reaches a non-GM client even as an event payload.
pub const EVENT_CODE_CHAT_MESSAGE: i32 = 17;
/// Play-view Combat: the shared initiative tracker changed (started,
/// combatant added/updated/removed, turn advanced, ended). Payload:
/// `{ "combatId": "..." }`. Same refetch-on-nudge shape as chat above —
/// turn order is small and always read whole, so there is nothing to gain
/// from diffing it over the wire.
pub const EVENT_CODE_COMBAT_CHANGED: i32 = 18;

/// Spec 029: what a token discloses about a resource changed.
///
/// Distinct from `EVENT_CODE_TOKEN_CHANGED` because a value changing and a
/// change in what may be *known* about a value are different facts, and a
/// client reacts differently: the first moves a bar, the second can make one
/// appear, vanish, or stop being an estimate.
pub const EVENT_CODE_TOKEN_DISCLOSURE_CHANGED: i32 = 19;

/// Spec 030: an interactive was authored, edited, deleted, reset, or fired.
///
/// Its own code rather than a reuse of the wall or token codes, because the
/// subject changing and the *interaction attached to it* changing are
/// different facts. A client redraws a wall for one and re-reads what a player
/// may click for the other.
pub const EVENT_CODE_INTERACTIVE_CHANGED: i32 = 20;

/// Spec 030: a door's state, lock, secrecy or designation changed.
///
/// Distinct from `EVENT_CODE_WALL_CHANGED` deliberately. Wall geometry moving
/// is a Game Master editing the map; a door opening is play, happens far more
/// often, and re-resolves vision and movement for everybody. Collapsing them
/// would make every door click look like a map edit to every client.
pub const EVENT_CODE_DOOR_CHANGED: i32 = 21;

/// Spec 030: a player asked, and the Game Master has not decided yet.
///
/// Carries the request rather than its outcome — the outcome arrives as
/// whatever the approved effect did, which is a different event.
pub const EVENT_CODE_INTERACTION_REQUEST: i32 = 22;

/// Spec 032 (SC-001): the world's interface pack changed.
///
/// Carried on the world-event channel rather than through a mechanism of its
/// own, because every other cross-participant change in this product travels
/// this way — walls, lights, tokens, doors, combat — and a second path would
/// be a second thing to get wrong on reconnect. It also means the spec 028
/// catch-up covers a client that was offline when the look changed, at no
/// additional cost.
///
/// Payload: `{"action": "changed", "interfacePackId": <id> | null}`.
pub const EVENT_CODE_WORLD_APPEARANCE_CHANGED: i32 = 23;

/// A world's **game system** changed (spec 033, ADR-065).
///
/// Its palette changing has been announced since spec 032; its ruleset
/// changing was not, which is the larger of the two events by some distance.
/// Every participant's compendium presents differently afterwards, so the same
/// argument that put the appearance change on this channel applies here with
/// more force — including spec 028's catch-up for a client that was offline
/// when it happened.
///
/// Payload: `{"action": "changed", "gameSystemId": <id>}`.
pub const EVENT_CODE_WORLD_SYSTEM_CHANGED: i32 = 24;

/// A scene's **ambient light** changed (playtest 2026-09-10 P9).
///
/// Every client showing the scene draws its darkness layer from this level,
/// so it has to reach all of them, not only the Game Master who set it. The
/// level travels in the payload rather than behind a re-read: a scene's light
/// is no secret, and everyone in the scene is about to see it anyway.
///
/// Payload: `{"action": "changed", "sceneId": <id>, "ambientLight":
/// "bright" | "dim" | "dark"}`.
pub const EVENT_CODE_SCENE_LIGHTING_CHANGED: i32 = 25;

/// A character's own data changed — its sheet, not its token.
///
/// Spec 045 FR-067. Sheet edits announced nothing at all before this: a
/// character who gained darkvision kept their old sight on every board until
/// somebody reloaded, because nothing told anybody the sheet had moved.
///
/// Distinct from `EVENT_CODE_TOKEN_CHANGED` on purpose, and for the reason
/// spec 045 phase 1 had to learn the hard way with doors: announcing a change
/// on a channel that describes something else reaches the wrong listeners and
/// misreports what happened. A sheet is not a token. A character may have no
/// token at all, or several.
///
/// Payload: `{"action": "changed", "actorId": <id>, "dataType": <slot>}`.
pub const EVENT_CODE_ACTOR_SHEET_CHANGED: i32 = 26;

/// A Game Master reset what a scene remembers being explored.
///
/// Spec 045 US7. The **fast path**, not the mechanism: what makes a reset
/// stick is the epoch a client compares on arrival, so a player who was
/// offline still finds out. This is how a player who is looking at the board
/// finds out immediately instead.
///
/// Payload: `{"action": "reset", "sceneId": <id>, "forUser": <id|null>,
/// "epoch": <n>}`. `forUser` is null for a reset that reaches everyone.
pub const EVENT_CODE_SCENE_EXPLORATION_RESET: i32 = 27;

/// An operator paused this world's play.
///
/// Spec 051 FR-020, research R1. The **fast path**, not the lock: every open
/// `worldEventsCreated` stream delivers it within the listener's 100 ms poll,
/// and an honest client leaves the playfield on receipt. What makes the pause
/// hold is `play_pause::gate` and the stream poll in `session_lifetime`, which
/// end every stream for the world within five seconds whether or not the
/// client listened.
///
/// There is no event for a lift. Nothing is streaming a paused world to hear
/// one; the notice asks `worldPlayState` instead (research R6).
///
/// Payload: `{"pausedAt": <iso8601>}` and nothing else. Every member of the
/// world receives this, so it never carries grounds, a trigger or who paused it
/// (FR-011).
pub const EVENT_CODE_WORLD_PLAY_PAUSED: i32 = 28;

/// Somebody made an attack (spec 046 FR-002, contract §4).
///
/// Every member of the world receives every event, and an attack is exactly
/// the kind of thing a viewer may not be allowed to know all of: who made it,
/// when the attacker is out of their sight or its name is hidden (FR-002a).
/// So the payload is the attack's id and **nothing else** — no attacker, no
/// target, no ability, no number — and each client asks `attack(id)`, which
/// the server answers per viewer (`combat::redaction`).
///
/// Payload: `{"attackId": <id>}`.
pub const EVENT_CODE_ATTACK_MADE: i32 = 29;

/// An offer of damage or healing was made, taken, declined or applied
/// (spec 046 FR-005, FR-008, FR-009).
///
/// Ids only, for the same reason as `EVENT_CODE_ATTACK_MADE`: the offer's
/// target is a token a viewer may not be able to see, so even the target's id
/// stays out of the payload. A client re-reads `pendingOffers`, and
/// `attack(id)` for an attack it has on screen.
///
/// Payload: `{"offerId": <id>}`.
pub const EVENT_CODE_OFFER_CHANGED: i32 = 30;

/// Who may write to a character changed (spec 063, ADR-110).
///
/// A claim now grants Editor and a release takes it back, so a player's
/// ability to edit a sheet they already have open can appear or vanish under
/// them. Nothing announced an access change before this, by any route: a
/// Game Master's hand grant took effect on the server at once and on the
/// player's page at their next reload.
///
/// Its own code rather than a reuse of `EVENT_CODE_ACTOR_SHEET_CHANGED`, for
/// the reason that code gives for not reusing the token one: the sheet did
/// not move, and every board listening for sheet changes would re-read
/// status bars for nothing.
///
/// The payload names the character and **not** the person. Every member of
/// the world receives every event, and who holds what on a character is the
/// ownership block, which only the Game Master may read. Each client asks
/// again what *it* may do, and the server answers per caller.
///
/// Payload: `{"action": "changed", "actorId": <id>}`.
pub const EVENT_CODE_ACTOR_ACCESS_CHANGED: i32 = 31;

/// A game system's standing difficulty for the table changed.
///
/// Reserved here and written only by the pack that owns it, as code 15 is:
/// the codes are one namespace across the product and every pack, and this
/// list is the one place two features would find out they had chosen the
/// same number. The server never records this event itself.
///
/// Payload: `{"action": "set" | "cleared"}`. Not the number — each client
/// reads it again, and the read checks membership.
pub const EVENT_CODE_TABLE_DIFFICULTY_CHANGED: i32 = 32;

/// A scene's levels changed: one was added, renamed, reordered, given a new
/// board, made the entry level, or removed.
///
/// Every member receives this and re-reads `sceneLevels`, which answers each
/// of them only for the levels they may read. So the payload can name the
/// level without telling a player anything: an id they cannot ask about.
///
/// Payload: `{"action": "created"|"updated"|"reordered"|"deleted",
/// "scene_id": <id>, "level_id": <id|null>}`. `level_id` is null for a
/// reorder, which concerns all of them.
pub const EVENT_CODE_SCENE_LEVEL_CHANGED: i32 = 33;

/// A token moved from one level of a scene to another — by the stairs, or by
/// a Game Master's hand.
///
/// Its own code rather than `EVENT_CODE_TOKEN_CHANGED` alone, because the
/// right reaction is different. A token that changed is re-read; a token that
/// travelled may have left the level a client is showing, or arrived on it,
/// or — for the player who controls it — changed which level they may read
/// at all. A client re-reads its whole level, starting with `sceneLevels`.
///
/// The payload deliberately names neither level. Every member of the world
/// receives every event, and where a token went is what a player on another
/// floor is not told.
///
/// Payload: `{"token_id": <id>, "scene_id": <id>}`.
pub const EVENT_CODE_TOKEN_TRAVELLED: i32 = 34;

/// A Game Master changed one of the settings the world's game system
/// declares (spec 067).
///
/// The payload names the key and not the value. Settings are readable by
/// every member, so the value is no secret — it is left out because the
/// convention here is "the event says what to re-read", and a client that
/// re-reads `worldSystemSettings` gets the declaration's verdict on the
/// value along with it.
///
/// Payload: `{"key": <setting id>}`.
pub const EVENT_CODE_WORLD_SYSTEM_SETTING_CHANGED: i32 = 35;
/// Spec 081: a roll was made. Payload: [`roll_event_payload`], the roll's id
/// and its visibility and nothing else (FR-002) — every member's
/// subscription sees every event, so the dice are fetched per viewer from
/// `worldRoll`. A `gm_only` one is not delivered to players at all (FR-005a,
/// `graphql::subscriptions`).
pub const EVENT_CODE_ROLL_MADE: i32 = 36;
/// Spec 081: the GM showed a hidden roll to the table. Same payload, with the
/// roll's original visibility; every member refetches and animates it.
pub const EVENT_CODE_ROLL_REVEALED: i32 = 37;
/// Spec 082: a Game Master gave a player an authoring tool or took one
/// away. The player's rail re-reads `authoringTools` on it, so a revoked
/// tool goes without a reload (SC-005). Payload: `{"userId": <the player>}`.
pub const EVENT_CODE_AUTHORING_TOOLS_CHANGED: i32 = 38;

/// The whole payload of a roll event (FR-002).
pub fn roll_event_payload(
    roll_id: Uuid,
    visibility: crate::rolls::visibility::Visibility,
) -> serde_json::Value {
    serde_json::json!({ "rollId": roll_id, "visibility": visibility.as_str() })
}

/// Whether a roll event may go to a subscriber who does or does not see behind
/// the screen. Every other event goes to everyone.
pub fn roll_event_reaches(
    event_code: i32,
    payload: Option<&serde_json::Value>,
    is_gm_or_admin: bool,
) -> bool {
    if event_code != EVENT_CODE_ROLL_MADE {
        return true;
    }
    let visibility = payload
        .and_then(|p| p.get("visibility"))
        .and_then(|v| v.as_str())
        .map(crate::rolls::visibility::Visibility::parse)
        .unwrap_or(crate::rolls::visibility::Visibility::GmOnly);
    crate::rolls::visibility::event_reaches(visibility, is_gm_or_admin)
}

/// Announce [`EVENT_CODE_ACTOR_ACCESS_CHANGED`] for one character.
///
/// One function for the five writers — claim, bind, release, hand set, hand
/// remove — so the payload has one shape and none of them can add the user
/// id the constant's comment explains is left out. Called after the write
/// has committed, and best-effort like every other announcement: a failure
/// is logged by `record_world_event` and costs a stale page, not the grant.
pub fn announce_actor_access_changed(
    conn: &mut PgConnection,
    world_id: Uuid,
    actor_id: Uuid,
    caller_id: Uuid,
) {
    let _ = record_world_event(
        conn,
        world_id,
        EVENT_CODE_ACTOR_ACCESS_CHANGED,
        Some(serde_json::json!({
            "action": "changed",
            "actorId": actor_id,
        })),
        caller_id,
    );
}

/// Record a world event to the audit trail and trigger NOTIFY for real-time sync.
///
/// # Failures are logged here, not at the call sites
///
/// Almost every caller writes `let _ = record_world_event(...)`, and that is
/// the right call shape: the mutation itself has already succeeded and
/// committed, so failing it now would report an error for work that was done.
/// An event is a nudge to other clients, not part of the transaction's
/// promise.
///
/// But `let _ =` at a dozen call sites meant a failed event write was
/// **completely invisible**: the mutation returned success, every layer
/// downstream behaved perfectly, and there was simply no row — no log, no
/// metric, nothing to find afterwards. That is the worst shape a bug can
/// have, and it was a live candidate for a real event-loss investigation
/// precisely because nothing could rule it out.
///
/// So the diagnostic lives here, once, where it cannot be forgotten by the
/// next caller to adopt the same `let _ =`. The signature still returns the
/// error, so a caller that *does* care keeps the choice.
pub fn record_world_event(
    conn: &mut PgConnection,
    world_id: Uuid,
    event_code: i32,
    event_payload: Option<serde_json::Value>,
    user_id: Uuid,
) -> GraphQLResult<i64> {
    let result = record_world_event_inner(conn, world_id, event_code, event_payload, user_id);
    if let Err(err) = &result {
        // Deliberately loud. A world event that was not recorded means every
        // other client in that world is now looking at stale state until
        // something else happens to refresh it.
        eprintln!(
            "[world_events] ⚠️  FAILED to record event code={} world={} user={}: {} \
             — subscribers will not be told about this change",
            event_code, world_id, user_id, err.message
        );
    }
    result
}

fn record_world_event_inner(
    conn: &mut PgConnection,
    world_id: Uuid,
    event_code: i32,
    event_payload: Option<serde_json::Value>,
    user_id: Uuid,
) -> GraphQLResult<i64> {
    let now = Utc::now().naive_utc();

    let event_id = diesel::insert_into(world_events::table)
        .values((
            world_events::world_id.eq(world_id),
            world_events::event_code.eq(event_code),
            world_events::token_event.eq(event_payload),
            world_events::schema_version.eq(1),
            world_events::created_at.eq(now),
            world_events::updated_at.eq(now),
            world_events::created_by.eq(user_id),
            world_events::updated_by.eq(user_id),
        ))
        .returning(world_events::id)
        .get_result::<i64>(conn)
        .map_err(|e| Error::new(format!("Failed to record event: {}", e)))?;

    // Vestigial, and kept deliberately: `listener.rs` issues `LISTEN
    // world_events_channel` and then never reads a notification — delivery is
    // entirely the 100ms poll. This stays because an `AFTER INSERT` trigger
    // sends the same notification anyway, and because a future listener that
    // does consume it should not have to rediscover that the publish side was
    // removed. It is not what makes an event arrive today.
    diesel::sql_query("SELECT pg_notify('world_events_channel', $1)")
        .bind::<diesel::sql_types::Text, _>(event_id.to_string())
        .execute(conn)
        .map_err(|e| Error::new(format!("Failed to notify: {}", e)))?;

    Ok(event_id)
}

/// Look up the `world_id` that owns a scene, for callers (walls/lights/
/// shapes/import) that only carry `scene_id`.
pub fn world_id_for_scene(conn: &mut PgConnection, scene_id: Uuid) -> GraphQLResult<Uuid> {
    use crate::schema::scenes;

    scenes::table
        .filter(scenes::scene_id.eq(scene_id))
        .select(scenes::world_id)
        .first::<Uuid>(conn)
        .map_err(|e| Error::new(format!("Failed to resolve scene's world: {}", e)))
}

#[cfg(test)]
mod roll_event_tests {
    use super::*;
    use crate::rolls::visibility::Visibility;

    /// Spec 081 T005: a roll event carries the roll's id and visibility and
    /// nothing else, so nothing about the dice can ride along to a player.
    #[test]
    fn a_roll_event_payload_is_exactly_its_id_and_visibility() {
        let id = Uuid::now_v7();
        let payload = roll_event_payload(id, Visibility::GmEyes);
        let mut keys: Vec<&str> = payload
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort();
        assert_eq!(keys, ["rollId", "visibility"]);
        assert_eq!(payload["rollId"], id.to_string());
        assert_eq!(payload["visibility"], "gm_eyes");
    }

    #[test]
    fn only_a_gm_only_roll_made_is_withheld() {
        let hidden = roll_event_payload(Uuid::now_v7(), Visibility::GmOnly);
        assert!(!roll_event_reaches(
            EVENT_CODE_ROLL_MADE,
            Some(&hidden),
            false
        ));
        assert!(roll_event_reaches(
            EVENT_CODE_ROLL_MADE,
            Some(&hidden),
            true
        ));
        assert!(roll_event_reaches(
            EVENT_CODE_ROLL_REVEALED,
            Some(&hidden),
            false
        ));
        assert!(roll_event_reaches(29, None, false));
        // A roll event without a readable visibility is treated as hidden.
        assert!(!roll_event_reaches(EVENT_CODE_ROLL_MADE, None, false));
    }
}
