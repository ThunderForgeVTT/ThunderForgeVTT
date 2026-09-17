//! A book that has been read in, and everything that came out of it
//! (spec 049 US3, spec 050 US1).
//!
//! # What a compendium is, and whose it is
//!
//! One import, one compendium: read the Dungeon Master's Guide and get one
//! bucket holding everything found in it. The bucket belongs to the **account
//! that imported it** and never to a world (049 FR-040) — a Game Master with
//! eight worlds and one Monster Manual owns one Monster Manual, and a world
//! reaches it by inheriting rather than by holding a copy.
//!
//! That is also why nothing here takes a `world_id`. A world-scoped store
//! would have had to be migrated out from under every entry that referenced
//! it, and the arc was planned to avoid exactly that (research §7).
//!
//! # What is not here
//!
//! **The file.** Reading happens in the Game Master's own browser (FR-020)
//! and the PDF never leaves their machine. What arrives is what was read out
//! of it, plus the SHA-256 that lets a re-import be recognised (FR-047).
//! There is no column for the bytes and no code path that would fill one.
//!
//! **A way to make uploaded content authored.** [`origin::ContentOrigin`] is
//! written once, by [`store::import_book`], and there is no function in this
//! module that updates it. The database refuses the change as well, so the
//! absence here is a convenience rather than the whole guarantee — see
//! FR-057, and the trigger in the `2026-09-12-100000-0000_compendium`
//! migration.

pub mod collections;
pub mod origin;
pub mod store;

pub use origin::ContentOrigin;
