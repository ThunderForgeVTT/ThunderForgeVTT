//! The real-time delivery loop, and the pool sizing it runs against.
//!
//! # Why this is its own crate
//!
//! The loop that reads new world events and wakes the clients watching them
//! is the single point of failure for everything live in the product: token
//! moves, dice, chat, fog. When it stops, nothing errors. Rows keep
//! committing, HTTP keeps answering, `/readyz` keeps returning 200, and the
//! only symptom is that the game stops moving on everyone's screen.
//!
//! That loop used to live inside an async task in the server binary, wrapped
//! around a connection pool, a diesel query and a timer. Every interesting
//! question about it — what happens when a poll panics, when it hangs, when
//! it errors, when a burst arrives faster than one batch can carry — could
//! only be asked by running the entire stack under load and watching. One
//! such failure did occur, and pinning it took `pg_stat_activity`, per-thread
//! `wchan` sampling and a stack-trace attempt against a frozen process.
//!
//! None of those questions need a database. They are questions about a loop.
//! So the loop lives here, generic over where rows come from and where they
//! go, and the failure modes are ordinary test cases:
//!
//! - a source that panics ([`delivery::tests`] proves the loop survives it)
//! - a source that returns errors forever
//! - a source that hangs and never returns
//! - a burst deeper than one poll batch
//!
//! # What is deliberately not here
//!
//! `diesel`, `r2d2`, the schema, and the `world_events` table. The server
//! implements [`EventSource`] over its pool and passes it in. This crate has
//! no idea Postgres exists, which is exactly why its tests run in
//! milliseconds with nothing installed.
//!
//! # Relationship to `thunderforge-pg-sockets`
//!
//! That crate answers *who gets an event and when may the cursor move*
//! (the router, the cursor rule, the de-duplication relay). This one answers
//! *how do we keep asking, forever, without dying quietly*. It depends on
//! that crate and not the other way round.

pub mod delivery;
pub mod pool;

pub use delivery::{DeliveryConfig, DeliveryMetrics, EventSink, EventSource, run_delivery};
pub use pool::{PoolSizing, pool_sizing_from_env};
