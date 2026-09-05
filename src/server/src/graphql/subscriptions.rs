//! The live half of the schema: what a client is told without asking again.
//!
//! `may_watch_world` is the gate every world-scoped stream passes through. It
//! is checked once when the subscription opens, which is the thing to keep in
//! mind when changing it — a stream already running does not re-ask.

use async_graphql::{Context, Result as GraphQLResult, Subscription};
use futures_util::Stream;
use std::time::Duration;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::IntervalStream;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;

use super::*;
use crate::state::AppState;

#[derive(Default)]
pub struct SubscriptionRoot;

/// Whether this subscriber may see this world at all.
///
/// Extracted because it was written once, for `world_events_created`, and
/// then simply not written for `players_online` — which subscribed anyone
/// who could name a world id. Two subscriptions over the same world data
/// must not be able to disagree about who may watch it, and the way to
/// guarantee that is for there to be one check rather than two.
///
/// Answers `false` for every failure — no app state, no session, a pool
/// error, a database error. A subscription is a long-lived grant of
/// access; refusing one because we could not confirm entitlement is the
/// safe direction, and the client's own retry covers the transient case.
async fn may_watch_world(
    ctx: &Context<'_>,
    app_state: &Option<AppState>,
    world_uuid: &Option<uuid::Uuid>,
) -> bool {
    match (app_state, world_uuid) {
        (Some(state), Some(uuid)) => match authenticated_user(ctx) {
            Ok(auth_user) => {
                let user_id = auth_user.user_id;
                let world_uuid = *uuid;
                let pool = state.db_pool.clone();
                tokio::task::spawn_blocking(move || {
                    pool.get()
                        .ok()
                        .and_then(|mut conn| {
                            require_world_member(&mut conn, user_id, world_uuid).ok()
                        })
                        .is_some()
                })
                .await
                .unwrap_or(false)
            }
            Err(_) => false,
        },
        _ => false,
    }
}

#[Subscription]
impl SubscriptionRoot {
    async fn tick(&self) -> impl Stream<Item = i32> {
        let mut value = 0;
        tokio_stream::StreamExt::map(
            IntervalStream::new(tokio::time::interval(Duration::from_secs(1))),
            move |_| {
                value += 1;
                value
            },
        )
    }

    /// Subscribe to world events (tokens, actors, scenes, etc.)
    ///
    /// Phase 4.9.A.2: Real-time event streaming via PostgreSQL pub/sub backplane
    /// Phase 4.9.A.3: Backpressure handling for lagged subscribers
    ///
    /// All subscribers receive events broadcast from the database listener task.
    /// Events are sent immediately as they are recorded in world_events table.
    ///
    /// If a client falls behind (buffer fills), the subscription will stop receiving
    /// events until it catches up. This is graceful degradation under load.
    async fn world_events_created(
        &self,
        ctx: &Context<'_>,
        world_id: String,
    ) -> impl Stream<Item = Result<GraphQLWorldEvent, Error>> {
        use std::pin::Pin;

        let app_state = ctx.data::<AppState>().ok().cloned();
        let world_uuid = uuid::Uuid::parse_str(&world_id).ok();

        // Authorization: this previously had none at all — any authenticated
        // user could subscribe to any world's events by guessing a world_id,
        // bypassing per-world membership entirely.
        let membership_ok = may_watch_world(ctx, &app_state, &world_uuid).await;

        // Collect all validation to happen upfront
        let (has_error, error_msg, rx_opt) = match (&app_state, &world_uuid) {
            (None, _) => (true, "Failed to get app state", None),
            (_, None) => (true, "Invalid world_id format", None),
            (_, _) if !membership_ok => (true, "You must be a member of this world", None),
            (Some(app_state), Some(world_uuid)) => {
                // This world's channel, not the whole process's. The stream
                // below no longer filters, because nothing else can arrive.
                (
                    false,
                    "",
                    Some(app_state.world_events.subscribe(*world_uuid)),
                )
            }
        };

        // Counted, not logged. A subscription storm is 25 of these inside a
        // second, and the churn test opens 155 — see `subscription_metrics`
        // for why a line each is a way to stop delivery rather than a way to
        // observe it. The refusal keeps its line: it is rare, and it is the
        // one that someone has to be able to find.
        if has_error {
            subscription_metrics::REFUSED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            eprintln!(
                "[GraphQL Subscription] 🚫 Refused a subscription to world_id={world_id}: \
                 {error_msg}"
            );
        } else {
            subscription_metrics::OPENED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }

        // Create a combined stream that works for both cases
        // Return type is Pin<Box<dyn Stream>> for type erasure
        if let Some(rx) = rx_opt {
            // Success case: stream this world's channel. The id is no longer
            // needed to *filter* — the receiver is the filter now — but the
            // lag diagnostic below still names the world it lost events for.
            let world_uuid = world_uuid.unwrap();

            let stream =
                tokio_stream::wrappers::BroadcastStream::new(rx).filter_map(move |result| {
                    match result {
                        Ok(event) => {
                            // No world check. The receiver is this world's
                            // channel, so an event arriving here is ours by
                            // construction — the old
                            // `if event.world_id == world_uuid` was the
                            // per-subscriber half of a fan-out that woke every
                            // client in the process for every event and had
                            // each of them throw away what was not theirs.
                            //
                            // Counted rather than logged: this runs once per
                            // event per subscriber, and `eprintln!` here is a
                            // blocking write on the task that carries the
                            // subscription. See `subscription_metrics`.
                            subscription_metrics::DELIVERED
                                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            Some(Ok(GraphQLWorldEvent::from(event)))
                        }
                        // The only error `BroadcastStream` yields is
                        // `Lagged(n)`: this receiver fell far enough behind
                        // that the channel overwrote `n` messages it had not
                        // read. Those events are **gone for this client** —
                        // no retry, no backfill, and the stream continues as
                        // if nothing happened.
                        //
                        // Dropping it to `None` is still the right stream
                        // behaviour (ending the subscription would be worse
                        // than missing an event), but it must not be quiet
                        // about *how many*. This previously logged the error
                        // with `{:?}` and no count in the message, which read
                        // as a transient warning rather than "this client's
                        // view of the world is now wrong".
                        //
                        // The client's recovery is the world sync it performs
                        // on open; there is no resync signal on this wire yet,
                        // which is precisely why the log has to be findable.
                        //
                        // Every one of them is counted; the line itself is
                        // capped at one every ten seconds, because a wedged
                        // subscriber lags on every event after the first and
                        // an uncapped line here is a way to stall the very
                        // fan-out it is reporting on.
                        Err(BroadcastStreamRecvError::Lagged(missed)) => {
                            subscription_metrics::LAGGED_EVENTS
                                .fetch_add(missed, std::sync::atomic::Ordering::Relaxed);
                            if subscription_metrics::should_log_lag() {
                                eprintln!(
                                    "[GraphQL Subscription] ⚠️  DROPPED {missed} event(s) for a \
                                     subscriber of world {world_uuid}: it fell behind the \
                                     broadcast buffer. Those events will never be delivered to \
                                     it. (Further lag lines are suppressed for 10s; the running \
                                     total is in the [PubSub] metrics line.)"
                                );
                            }
                            None
                        }
                    }
                });
            Pin::new(Box::new(stream))
                as Pin<Box<dyn Stream<Item = Result<GraphQLWorldEvent, Error>> + Send>>
        } else {
            // Error case: single error item
            let stream = tokio_stream::iter(vec![Err(Error::new(error_msg))]).filter_map(Some);
            Pin::new(Box::new(stream))
                as Pin<Box<dyn Stream<Item = Result<GraphQLWorldEvent, Error>> + Send>>
        }
    }

    /// Spec 028 (T086): receive WebRTC signaling addressed to this session.
    ///
    /// The stream **is** the registration. It begins when this subscription
    /// is established and ends when it drops, which is what confines peer
    /// connections to the session (FR-050) without a cleanup job that could
    /// be skipped on a crash.
    ///
    /// `sessionId` is a deliberate, minimal extension to the contract's SDL:
    /// a client cannot be reachable without naming the address it wants to be
    /// reachable at, and `PeerSignal` carries no field that could tell it one.
    /// The server treats the value as opaque and forgets it on disconnect.
    async fn peer_signals(
        &self,
        ctx: &Context<'_>,
        world_id: uuid::Uuid,
        session_id: String,
    ) -> impl Stream<Item = Result<crate::peer_signaling::GraphQLPeerSignal, Error>> {
        crate::peer_signaling::peer_signals_stream(ctx, world_id, session_id).await
    }

    /// Subscribe to player presence changes (Phase 4.9.B.3)
    ///
    /// Streams updates when players connect, disconnect, or change scenes.
    /// Returns current list of all online players in the world.
    async fn players_online(
        &self,
        ctx: &Context<'_>,
        world_id: String,
    ) -> impl Stream<Item = Result<GraphQLPlayersOnlineList, Error>> {
        use std::pin::Pin;

        let app_state = ctx.data::<AppState>().ok().cloned();
        let world_uuid = uuid::Uuid::parse_str(&world_id).ok();

        // The same gate `world_events_created` uses, which this subscription
        // did not have: it accepted anyone who could name a world id. Harmless
        // only for as long as the payload below stays empty — which is exactly
        // the kind of "safe because unfinished" that stops being true the day
        // someone finishes it.
        let membership_ok = may_watch_world(ctx, &app_state, &world_uuid).await;

        let (has_error, error_msg, rx_opt) = match (&app_state, &world_uuid) {
            (None, _) => (true, "Failed to get app state", None),
            (_, None) => (true, "Invalid world_id format", None),
            (_, _) if !membership_ok => (true, "You must be a member of this world", None),
            (Some(app_state), Some(_)) => (false, "", Some(app_state.presence_sender.subscribe())),
        };

        // Same reasoning as `world_events_created`: counted, not logged, and
        // only the refusal is worth a line. See `subscription_metrics`.
        if has_error {
            subscription_metrics::REFUSED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            eprintln!(
                "[GraphQL Subscription] 🚫 Refused a presence subscription to \
                 world_id={world_id}: {error_msg}"
            );
        } else {
            subscription_metrics::OPENED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }

        if let (Some(rx), Some(world_id_uuid)) = (rx_opt, world_uuid) {
            // Success case: emit presence notifications
            // Note: This is a simple implementation that emits on each presence event.
            // In production, you'd query the DB to get the full player list on each event.
            let stream =
                tokio_stream::wrappers::BroadcastStream::new(rx).filter_map(move |result| {
                    match result {
                        Ok(_presence_event) => {
                            subscription_metrics::DELIVERED
                                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            // Still a stub. When this is wired up, read it
                            // from `AppState::presence` — presence lives in
                            // memory now, and the `players_online` table is
                            // no longer written on each heartbeat. A helper
                            // that queried that table used to sit below this
                            // file, marked `#[allow(dead_code)]` and waiting
                            // for a resolver; it was removed rather than left
                            // pointing at a table nothing fills.
                            Some(Ok(GraphQLPlayersOnlineList {
                                world_id: world_id_uuid,
                                players: vec![],
                            }))
                        }
                        // Same as the world-event stream above: `Lagged(n)`
                        // means n presence updates were overwritten before
                        // this subscriber read them. Less costly than a lost
                        // world event — presence is a snapshot, and the next
                        // update supersedes the ones missed — but the count
                        // is still the difference between "a blip" and "this
                        // client is minutes stale".
                        Err(BroadcastStreamRecvError::Lagged(missed)) => {
                            subscription_metrics::LAGGED_EVENTS
                                .fetch_add(missed, std::sync::atomic::Ordering::Relaxed);
                            if subscription_metrics::should_log_lag() {
                                eprintln!(
                                    "[GraphQL Subscription] ⚠️  DROPPED {missed} presence \
                                     update(s) for a subscriber: it fell behind the broadcast \
                                     buffer."
                                );
                            }
                            None
                        }
                    }
                });
            Pin::new(Box::new(stream))
                as Pin<Box<dyn Stream<Item = Result<GraphQLPlayersOnlineList, Error>> + Send>>
        } else {
            // Error case: single error item
            let stream = tokio_stream::iter(vec![Err(Error::new(error_msg))]).filter_map(Some);
            Pin::new(Box::new(stream))
                as Pin<Box<dyn Stream<Item = Result<GraphQLPlayersOnlineList, Error>> + Send>>
        }
    }

    /// Subscribe to actor system data changes (D&D 5e, Pathfinder, CoC, etc.)
    ///
    /// PHASE D.2 STUB: This subscription will stream actor system data updates
    /// from the pg_notify backplane when client subscribes.
    /// Full implementation pending async database driver integration.
    ///
    /// For now, returns a tick stream that can be tested.
    async fn world_actor_system_data_updated(
        &self,
        _ctx: &Context<'_>,
        _world_id: String,
        game_system_id: String,
    ) -> impl Stream<Item = GraphQLResult<GraphQLActorSystemDataEvent>> {
        // STUB: Return a placeholder stream
        // In production, this would listen to pg_notify and stream real events
        let game_system_id = game_system_id.clone();
        tokio_stream::StreamExt::map(
            IntervalStream::new(tokio::time::interval(Duration::from_secs(10))),
            move |_| {
                Ok(GraphQLActorSystemDataEvent {
                    id: uuid::Uuid::new_v4(),
                    actor_id: uuid::Uuid::new_v4(),
                    // Echoed back from the subscription's own argument. The
                    // stub used to hardcode a system id here, which is both a
                    // wrong answer for every other system and a thing shared
                    // server code may not say (FR-029).
                    game_system_id: game_system_id.clone(),
                    event_type: "UPDATE".to_string(),
                    ability_data: None,
                    resource_data: None,
                    proficiency_data: None,
                    trait_data: None,
                    spell_data: None,
                    updated_at: chrono::Local::now().naive_utc(),
                })
            },
        )
    }
}
