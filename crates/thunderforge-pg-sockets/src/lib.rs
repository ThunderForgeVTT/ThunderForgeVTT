//! Real-time world-event delivery: who gets woken, and when the cursor moves.
//!
//! Extracted from `src/server/src/network/listener.rs` and the global
//! broadcast channel it fed, for the same reason `thunderforge-opfs` was
//! extracted from the cache: the decisions worth testing were welded to I/O
//! that only exists at runtime, so they had no tests at all.
//!
//! # What is here
//!
//! - [`router`] — one `broadcast` channel per world instead of one for the
//!   whole process. This is the fix for a measured 20,000× delivery
//!   amplification at 100k connections; the numbers are on
//!   [`router::WorldRouter`].
//! - [`cursor`] — the rule for how far the "I have seen everything up to
//!   here" mark may advance, which is what stops an event that commits out of
//!   id order from being lost forever.
//! - [`relay`] — what one poll *means*: which of the rows it returned have
//!   already been broadcast, and where the cursor lands afterwards. The
//!   cursor rule and the de-duplication memory are only correct in agreement
//!   with each other, and that agreement is what this makes testable.
//!
//! # What is deliberately not here
//!
//! The Postgres I/O. No `diesel`, no `tokio-postgres`, no connection pool and
//! no schema: the poll query, the `LISTEN` connection and the `world_events`
//! table stay in the server, where the pool and the migrations live. This
//! crate takes what that machinery produces — ids, timestamps, a world id and
//! a payload — and decides what to do with it.
//!
//! That split is what makes `cargo test -p thunderforge_pg_sockets` a real
//! test rather than a compile check. Every rule in here runs against plain
//! values in microseconds, and the one thing that genuinely needs a database
//! — that two out-of-order commits are still both delivered — stays as an
//! integration test in the server, next to the pool it needs.
//!
//! # On keeping `LISTEN/NOTIFY`
//!
//! Worth writing down, because it is easy to benchmark the wrong thing.
//! Postgres struggling past ~100k *listening connections* says nothing about
//! this design: browsers never connect to Postgres. One app node needs a
//! single `LISTEN` connection, and the 100k-way fan-out happens in
//! [`router`], in our process. What NOTIFY buys is a wake at commit time
//! instead of a 100ms poll — lower latency, and commit-ordered by
//! construction.
//!
//! What it does not buy is delivery: a notification only reaches sessions
//! listening at that moment, so a dropped connection loses every event sent
//! during the gap. NOTIFY is therefore a *wake*, and the poll stays as the
//! reconciliation net behind it. [`cursor`] is what makes that pair safe.

pub mod cursor;
pub mod relay;
pub mod router;

pub use cursor::{COMMIT_GRACE, RowStamp, settled_cursor};
pub use relay::{Relay, Stamped};
pub use router::{SharedWorldRouter, WORLD_CHANNEL_CAPACITY, WorldRouter};
