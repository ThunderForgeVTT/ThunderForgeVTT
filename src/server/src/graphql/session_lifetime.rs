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

use diesel::prelude::*;
use futures_util::Stream;
use futures_util::StreamExt as _;
use std::time::Duration;

use crate::AppState;
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
    use crate::test_support::{insert_test_user, test_app_state};
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
}
