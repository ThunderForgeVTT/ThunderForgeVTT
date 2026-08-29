//! Credential and session rules, with nothing they are applied to.
//!
//! Everything here is a pure function over values a handler already has in
//! hand. There is no database, no `AppState`, no request and no cookie jar,
//! which is why these rules can be hammered by proptest rather than only
//! reached through an HTTP round trip that needs Postgres running.
//!
//! The HTTP handlers in `src/server/src/auth/` stay where the state is and
//! call in here for every decision. See `docs/CLIENT_WORLD_CACHE.md` for the
//! precedent this split follows.

pub mod constant_time;
pub mod csrf;
pub mod password;
pub mod random;
pub mod session;
pub mod totp;
