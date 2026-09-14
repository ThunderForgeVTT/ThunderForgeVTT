//! A fight that resolves (spec 046).
//!
//! The rules of a fight live here rather than in `graphql/mutations_combat.rs`,
//! which is the initiative tracker: who is in the fight and whose turn it is.
//! This module is what those turns *do* — hit points that change, a turn that
//! holds a player to it — and the GraphQL resolvers call into it.
//!
//! Shared code names no system's fields (contract M5). Everything a rule needs
//! to know about a system it reads from the pack's `combat` block through
//! [`manifest`].

pub mod manifest;
