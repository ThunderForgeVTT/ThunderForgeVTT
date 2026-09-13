//! The library, where it meets a world (spec 050 US3, T058 to T061).
//!
//! [`crate::compendium`] owns the shelf: a book belongs to an account and is
//! read, replaced and removed there. This module owns the one link by which a
//! world gets anything from it — the **book list** — and nothing else.
//!
//! # The list is the mechanism
//!
//! There is no seeding step, no import-into-world step and no copy step
//! (FR-031). A world switches a book on, and its content is *fetched* from the
//! account's shelf whenever the world reads it. That is why ticking books
//! while creating a world and ticking them afterwards are not two paths that
//! have to be kept agreeing (FR-033): there is one mutation, and world
//! creation calls it.
//!
//! # Two scopes meet here, and only here
//!
//! A compendium is account-scoped; a world is world-scoped. Switching a book
//! on is the single place in the arc where one reaches the other, so both
//! gates run, in this order:
//!
//! 1. the caller is **trusted with this table's books** — its Owner, a Game
//!    Master or a Trusted Player (spec 050 decision 8, ADR-099);
//! 2. the compendium is the **world owner's**, through
//!    [`crate::auth::account_ownership::require_account_owner`] — never the
//!    caller's, whoever the caller is (FR-010a);
//! 3. the book was read as the world's system (FR-041).
//!
//! The two ownership questions are kept apart on purpose. Who arranges a
//! table's books is a matter of trust; whose books they are is a matter of
//! ownership, and widening the first did not widen the second.
//!
//! The database asks the second and third again — is this book on the world
//! owner's shelf, and does it match? — in a trigger on `world_books`, so a
//! route that never calls this module still cannot write the row.

pub mod book_list;
