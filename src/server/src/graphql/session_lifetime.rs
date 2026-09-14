//! Spec 036 FR-010: a stream outlives the session that opened it, until now.
//!
//! # The gap this closes
//!
//! `may_watch_world` is checked **once**, when a subscription opens — the
//! header of `subscriptions.rs` says so, and it is correct about membership:
//! a person removed from a world reconnects soon enough. It is not correct
//! about revocation. Ending a session is something a person does *because*
//! they no longer trust the client holding it, and a client whose session was
//! ended an hour ago went on receiving every world event that account could
//! see, for as long as its socket stayed open. SC-004 says a revoked session
//! "is refused on its next request **and receives no further world events**";
//! only the first half was true.
//!
//! # Why a poll rather than a signal
//!
//! A broadcast of revoked ids would close the stream in milliseconds, and it
//! would close it *in this process only*. Sessions are ended by whichever
//! server handled the mutation, and nothing says that is the one holding the
//! socket. A poll asks the database, which every process shares, and answers
//! **expiry** with the same mechanism it answers revocation with — a stream
//! opened before `expires_at` used to run past it forever, which was the same
//! defect wearing a different hat.
//!
//! The cost is a bounded delay, and the query is a primary-key lookup on a
//! table with one row per live session.
//!
//! # A paused world asks the same question
//!
//! Spec 051 (research R1): a stream into a world an operator paused must end
//! too, and for the same reasons a revoked session's must — the pause is made
//! by whichever process handled the operator's mutation, and nothing says that
//! is the one holding the socket. [`until_stream_must_end`] therefore folds
//! "and this world is not paused" into the session's own poll rather than
//! running a second one beside it: one query per stream per tick, so a table
//! of ten is still two queries a second and not four.
//!
//! The two endings are told apart. A session that ends completes the stream,
//! as before. A pause yields one final `WORLD_PLAY_PAUSED` error first,
//! because a silent completion is what the server going away looks like, and
//! a client that cannot tell the two apart freezes on a paused world instead
//! of saying so.

use diesel::prelude::*;
use futures_util::Stream;
use futures_util::StreamExt as _;
use std::sync::{Arc, Mutex};
use std::task::Poll;
use std::time::Duration;

use crate::AppState;
use crate::play_pause::gate::PlayPaused;
use crate::schema::user_sessions;

/// How long a revoked session's stream may keep running.
///
/// Five seconds: short enough that "I ended that session" and "that screen
/// stopped updating" are the same event to the person who did it, long enough
/// that a table of ten clients is two queries a second rather than a load.
const LIVENESS_POLL: Duration = Duration::from_secs(5);

/// Stop `inner` once `session_id` is no longer a live session.
///
/// Ending the stream is all this does. The client sees its subscription
/// complete, which is what a GraphQL client already handles — it is the same
/// shape as the server going away — and its reconnect is refused by the auth
/// middleware, which is where "sign in again" is said properly.
pub fn until_session_ends<S>(
    state: AppState,
    session_id: uuid::Uuid,
    inner: S,
) -> impl Stream<Item = S::Item> + Send
where
    S: Stream + Send + 'static,
    S::Item: Send,
{
    // Boxed and pinned so the returned stream stays `Unpin` when `inner` is:
    // an `async` block is not, and every caller here hands the result to
    // `Pin::new(Box::new(..))`.
    inner.take_until(Box::pin(async move {
        let mut ticker = tokio::time::interval(LIVENESS_POLL);
        // `interval` fires immediately; the session was live a moment ago or
        // the subscription would not have opened.
        ticker.tick().await;
        loop {
            ticker.tick().await;
            if !session_is_live(&state, session_id).await {
                return;
            }
        }
    }))
}

/// Stop `inner` once `session_id` is no longer live, or `world_id` is paused.
///
/// For every world-scoped stream (contracts/live-play-lock.md).
/// [`until_session_ends`] stays for streams that are not about one world.
///
/// - **Session gone**: the stream completes, exactly as `until_session_ends`
///   does.
/// - **World paused**: the stream yields one `Err` carrying
///   `WORLD_PLAY_PAUSED`, then completes. Whatever `inner` had already
///   produced is still delivered first.
/// - **Unreadable**: it carries on, for the reason [`session_is_live`] gives.
///   Opening the stream is gated and fails closed; this is the one direction
///   that fails open.
pub fn until_stream_must_end<S, T>(
    state: AppState,
    session_id: uuid::Uuid,
    world_id: uuid::Uuid,
    inner: S,
) -> impl Stream<Item = async_graphql::Result<T>> + Send
where
    S: Stream<Item = async_graphql::Result<T>> + Send + 'static,
    T: Send + 'static,
{
    // Where the ending future leaves its reason for the tail to read. A
    // `poll_fn` rather than an `async` tail so the whole stream stays `Unpin`
    // when `inner` is, as `until_session_ends`'s does.
    let verdict: Arc<Mutex<Option<PlayPaused>>> = Arc::new(Mutex::new(None));
    let written = Arc::clone(&verdict);

    let ended = inner.take_until(Box::pin(async move {
        let mut ticker = tokio::time::interval(LIVENESS_POLL);
        ticker.tick().await;
        loop {
            ticker.tick().await;
            match stream_standing(&state, session_id, world_id).await {
                Standing::Live => {}
                Standing::SessionGone => return,
                Standing::Paused(paused) => {
                    *written.lock().unwrap_or_else(|p| p.into_inner()) = Some(paused);
                    return;
                }
            }
        }
    }));

    // Polled once `ended` completes, for whatever reason it did. `take`
    // empties the cell, so the error is yielded once and the next poll
    // completes the stream.
    let tail = futures_util::stream::poll_fn(move |_| {
        Poll::Ready(
            verdict
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .take()
                .map(|paused| Err(paused.into())),
        )
    });

    ended.chain(tail)
}

/// What one tick found.
enum Standing {
    Live,
    SessionGone,
    Paused(PlayPaused),
}

#[derive(diesel::QueryableByName)]
struct StandingRow {
    #[diesel(sql_type = diesel::sql_types::Bool)]
    live: bool,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Timestamp>)]
    paused_at: Option<chrono::NaiveDateTime>,
}

/// Session live, and world not paused — in one query.
///
/// The session half is [`session_is_live`]'s two conditions; the pause half
/// reads the same partial index `play_pause::gate` does. A session that is
/// gone wins over a pause: a revoked client is owed no explanation, and the
/// reconnect it attempts is refused by the auth middleware regardless.
async fn stream_standing(
    state: &AppState,
    session_id: uuid::Uuid,
    world_id: uuid::Uuid,
) -> Standing {
    let now = chrono::Utc::now().naive_utc();
    let Ok(mut conn) = state.db_pool.get() else {
        return Standing::Live;
    };

    let row = tokio::task::spawn_blocking(move || {
        diesel::sql_query(
            "SELECT \
               EXISTS (SELECT 1 FROM user_sessions \
                        WHERE id = $1 AND revoked_at IS NULL AND expires_at > $2) AS live, \
               (SELECT paused_at FROM world_play_pauses \
                 WHERE world_id = $3 AND lifted_at IS NULL) AS paused_at",
        )
        .bind::<diesel::sql_types::Uuid, _>(session_id)
        .bind::<diesel::sql_types::Timestamp, _>(now)
        .bind::<diesel::sql_types::Uuid, _>(world_id)
        .get_result::<StandingRow>(&mut conn)
    })
    .await;

    match row {
        Ok(Ok(StandingRow { live: false, .. })) => Standing::SessionGone,
        Ok(Ok(StandingRow {
            paused_at: Some(paused_at),
            ..
        })) => Standing::Paused(PlayPaused {
            world_id,
            paused_at,
        }),
        _ => Standing::Live,
    }
}

/// Whether this session would still authenticate a request.
///
/// The same two conditions `live_sessions` uses — not revoked, not expired —
/// so a session cannot be absent from its owner's list while still feeding a
/// stream.
///
/// An unreadable answer counts as **live**, deliberately. Closing every
/// subscription on the instance because one query failed would turn a
/// database blip into every table going dark, and it buys nothing: world
/// events are read from that same database, so a client whose session was
/// revoked during an outage receives nothing to be wrong about. The next tick
/// closes it.
async fn session_is_live(state: &AppState, session_id: uuid::Uuid) -> bool {
    let now = chrono::Utc::now().naive_utc();
    let Ok(mut conn) = state.db_pool.get() else {
        return true;
    };

    tokio::task::spawn_blocking(move || {
        user_sessions::table
            .filter(user_sessions::id.eq(session_id))
            .filter(user_sessions::revoked_at.is_null())
            .filter(user_sessions::expires_at.gt(now))
            .select(user_sessions::id)
            .first::<uuid::Uuid>(&mut conn)
            .optional()
    })
    .await
    .map_or(true, |result| result.map_or(true, |row| row.is_some()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::ClientDescription;
    use crate::auth::session_registry::end_session;
    use crate::auth::sessions::issue_session_cookie;
    use crate::test_support::{insert_test_user, insert_test_world, test_app_state};
    use tower_cookies::Cookies;

    /// A stream that never ends on its own, so the only thing that can end it
    /// is the session check.
    fn forever() -> impl Stream<Item = u32> + Send + 'static {
        tokio_stream::wrappers::IntervalStream::new(tokio::time::interval(Duration::from_millis(
            20,
        )))
        .map(|_| 1)
    }

    #[tokio::test]
    async fn a_live_session_keeps_its_stream() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let user_id = insert_test_user(&mut conn);
        drop(conn);
        let session = issue_session_cookie(
            &state,
            &Cookies::default(),
            user_id,
            ClientDescription::unknown(),
        )
        .await
        .expect("a session");

        let mut stream = Box::pin(until_session_ends(state, session.id, forever()));

        // Long enough to cross a liveness tick, so a stream that closes on a
        // live session fails here rather than passing by being quick.
        tokio::time::timeout(LIVENESS_POLL * 2, async {
            for _ in 0..3 {
                assert_eq!(stream.next().await, Some(1));
            }
        })
        .await
        .expect("a live session's stream keeps delivering");
    }

    /// FR-010 and the second half of SC-004. The stream ends on its own,
    /// without the client asking anything.
    #[tokio::test]
    async fn ending_the_session_ends_the_stream() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let user_id = insert_test_user(&mut conn);
        drop(conn);
        let session = issue_session_cookie(
            &state,
            &Cookies::default(),
            user_id,
            ClientDescription::unknown(),
        )
        .await
        .expect("a session");

        let mut stream = Box::pin(until_session_ends(state.clone(), session.id, forever()));
        assert_eq!(stream.next().await, Some(1), "it delivers while live");

        end_session(&state, user_id, session.id)
            .await
            .expect("the session ends");

        // Drained rather than asserted on the next item: whatever the inner
        // stream had already produced is still legitimately in flight, and
        // FR-010 is about the stream *ending*, not about the last event
        // before it.
        let ended = tokio::time::timeout(LIVENESS_POLL * 3, async {
            while stream.next().await.is_some() {}
        })
        .await;

        assert!(
            ended.is_ok(),
            "a revoked session's stream must end on its own, within a few \
             seconds of the revocation — it ran until the timeout instead",
        );
    }
    // ----- Spec 051: a paused world's streams end too -----

    /// A world-scoped stream that never ends on its own.
    fn forever_in_a_world() -> impl Stream<Item = async_graphql::Result<u32>> + Send + 'static {
        forever().map(Ok)
    }

    /// A member of a world, signed in, and an operator to pause it.
    struct Table {
        state: AppState,
        operator: uuid::Uuid,
        world: uuid::Uuid,
        session: uuid::Uuid,
        member: uuid::Uuid,
    }

    async fn a_table() -> Table {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let operator = insert_test_user(&mut conn);
        let member = insert_test_user(&mut conn);
        let world = insert_test_world(&mut conn, member);
        drop(conn);
        let session = issue_session_cookie(
            &state,
            &Cookies::default(),
            member,
            ClientDescription::unknown(),
        )
        .await
        .expect("a session");
        Table {
            state,
            operator,
            world,
            session: session.id,
            member,
        }
    }

    /// Everything the stream yields until it completes, or `None` if it ran
    /// past `limit`.
    async fn drain(
        stream: &mut (impl Stream<Item = async_graphql::Result<u32>> + Unpin),
        limit: Duration,
    ) -> Option<Vec<async_graphql::Error>> {
        tokio::time::timeout(limit, async {
            let mut errors = Vec::new();
            while let Some(item) = stream.next().await {
                if let Err(e) = item {
                    errors.push(e);
                }
            }
            errors
        })
        .await
        .ok()
    }

    /// FR-020, SC-001: after `pause_world`, the stream yields one
    /// `WORLD_PLAY_PAUSED` error and completes, on its own, within a tick.
    #[tokio::test]
    async fn pausing_the_world_ends_the_stream_with_one_pause_error() {
        let t = a_table().await;
        let mut stream = Box::pin(until_stream_must_end(
            t.state.clone(),
            t.session,
            t.world,
            forever_in_a_world(),
        ));
        assert!(
            matches!(stream.next().await, Some(Ok(1))),
            "it delivers while open"
        );

        let mut conn = t.state.db_pool.get().unwrap();
        crate::play_pause::pause_world(
            &mut conn,
            t.operator,
            t.world,
            "Stopping play.",
            crate::play_pause::TriggerDetail::operator(),
        )
        .expect("paused");
        drop(conn);

        // One tick, plus the moment the tick's query takes.
        let errors = drain(&mut stream, LIVENESS_POLL + Duration::from_secs(2))
            .await
            .expect("a paused world's stream must end on its own within one liveness tick");

        let [error] = errors.as_slice() else {
            panic!("exactly one error item expected, got {}", errors.len());
        };
        let extensions = serde_json::to_value(error.extensions.as_ref().unwrap()).unwrap();
        assert_eq!(extensions["code"], "WORLD_PLAY_PAUSED");
        assert_eq!(extensions["worldId"], t.world.to_string());
        assert!(stream.next().await.is_none(), "and it stays ended");
    }

    #[tokio::test]
    async fn a_lifted_pause_does_not_end_a_stream() {
        let t = a_table().await;
        let mut conn = t.state.db_pool.get().unwrap();
        let pause = crate::play_pause::pause_world(
            &mut conn,
            t.operator,
            t.world,
            "Stopping play.",
            crate::play_pause::TriggerDetail::operator(),
        )
        .unwrap()
        .pause;
        crate::play_pause::lift_pause(&mut conn, t.operator, pause.id, "Resolved.").unwrap();
        drop(conn);

        let mut stream = Box::pin(until_stream_must_end(
            t.state,
            t.session,
            t.world,
            forever_in_a_world(),
        ));
        tokio::time::timeout(LIVENESS_POLL * 2, async {
            for _ in 0..3 {
                assert!(matches!(stream.next().await, Some(Ok(1))));
            }
        })
        .await
        .expect("a stream into a world whose pause was lifted keeps delivering");
    }

    /// The session ending is still silent: no error item, only completion.
    #[tokio::test]
    async fn a_revoked_session_completes_without_an_error_item() {
        let t = a_table().await;
        let mut stream = Box::pin(until_stream_must_end(
            t.state.clone(),
            t.session,
            t.world,
            forever_in_a_world(),
        ));
        assert!(matches!(stream.next().await, Some(Ok(1))));

        end_session(&t.state, t.member, t.session)
            .await
            .expect("the session ends");

        let errors = drain(&mut stream, LIVENESS_POLL * 3)
            .await
            .expect("a revoked session's stream must still end on its own");
        assert!(
            errors.is_empty(),
            "a revoked session is owed no explanation"
        );
    }
}
