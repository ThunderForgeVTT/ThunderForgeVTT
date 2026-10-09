//! Serving one GraphQL WebSocket: the connection behind `/api/ws`.
//!
//! The axum handler upgrades the request and hands the socket here. This is
//! where a connection is counted, given its caller, and given an idle timeout.

use std::time::Duration;

use async_graphql::{Data, Executor};
use async_graphql_axum::{GraphQLProtocol, GraphQLWebSocket};
use axum::extract::ws::Message;

use crate::auth_middleware::AuthenticatedUser;
use crate::graphql::subscription_metrics::OpenSocket;

/// How long a socket may send nothing before the server closes it.
///
/// A client that vanishes without closing (a laptop lid shut, a network that
/// drops without a FIN) leaves a socket the server cannot tell from a quiet
/// one. Nothing is written to a quiet world, so no write fails, and the socket
/// and every subscription on it would otherwise stay open indefinitely.
///
/// Any client message resets the clock, and the web client pings every 15 s
/// (`graphql-ws`'s `keepAlive`), so a minute is four missed pings: a slow or
/// dropped ping never costs a healthy table its connection. The close is
/// `graphql-ws`'s own 3008 "timeout", which its client retries, and a
/// reconnect replays what was missed through the catch-up query.
pub const IDLE_TIMEOUT: Duration = Duration::from_secs(60);

/// Serve one upgraded socket until it closes, times out, or its client goes.
///
/// Takes the socket as its two halves so a test can serve one over channels;
/// the handler passes `IDLE_TIMEOUT`.
pub async fn serve<Si, St, E>(
    sink: Si,
    stream: St,
    executor: E,
    protocol: GraphQLProtocol,
    caller: AuthenticatedUser,
    idle_timeout: Duration,
) where
    Si: futures_util::Sink<Message> + Unpin,
    St: futures_util::Stream<Item = Result<Message, axum::Error>> + Unpin,
    E: Executor,
{
    // Counted, never logged per connection: a reconnect storm is hundreds of
    // these in a second, and the point of the number is to say how many
    // sockets are attached *now*. A guard rather than an add and a subtract
    // around `serve`, so a connection task that panics is still uncounted.
    let _open = OpenSocket::begin();
    GraphQLWebSocket::new_with_pair(sink, stream, executor, protocol)
        .on_connection_init(move |_value| async move {
            let mut data = Data::default();
            data.insert(caller);
            Ok(data)
        })
        .keepalive_timeout(idle_timeout)
        .serve()
        .await;
}
