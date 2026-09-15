//! A fight that resolves (spec 046).
//!
//! The rules of a fight live here rather than in `graphql/mutations_combat.rs`,
//! which is the initiative tracker: who is in the fight and whose turn it is.
//! This module is what those turns *do* — hit points that change, a turn that
//! holds a player to it, an attack aimed at something and the offer its damage
//! becomes — and the GraphQL resolvers call into it.
//!
//! Shared code names no system's fields (contract M5). Everything a rule needs
//! to know about a system it reads from the pack's `combat` block through
//! [`manifest`].

pub mod attack;
pub mod attack_fields;
pub mod budget;
pub mod controllers;
pub mod hit_points;
pub mod lair;
pub mod legendary;
pub mod manifest;
pub mod offers;
pub mod reach;
pub mod records;
pub mod redaction;
pub mod size;
pub mod turn;
pub mod weapon;

/// A table for the fight's tests. A `_tests.rs` file because it names a game
/// system, which only tests may (`check-system-registry.mjs`).
#[cfg(test)]
#[path = "fixtures_tests.rs"]
pub(crate) mod fixtures;
