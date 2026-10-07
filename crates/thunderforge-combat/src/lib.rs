//! The rules of a fight, once (spec 079, ADR-113).
//!
//! Spec 046 built a fight that resolves on the server: hit points that change,
//! a turn that holds a player to it, an attack aimed at something and the
//! offer its damage becomes. This crate is the part of that which is rules
//! rather than storage — what a hit is, what damage does, whose turn is next,
//! what a turn affords — so the server and the browser demo run one copy.
//!
//! The server loads what a rule needs, calls in here, and saves what it
//! decided. The demo does the same against what it holds in memory, through
//! the `wasm` feature's façade. Nothing here reads a database, opens a
//! socket, or owns entropy: a roll takes an injected RNG ([`dice`]).
//!
//! Shared code names no system's fields (spec 046 contract M5): everything a
//! rule needs to know about a system it reads from the pack's `combat` and
//! `turnStructure` blocks ([`manifest`]).

pub mod attack;
pub mod budget;
pub mod dice;
pub mod hit_points;
pub mod manifest;
pub mod order;
pub mod reach;
pub mod records;
pub mod size;
pub mod turn;
pub mod turn_structure;

#[cfg(feature = "wasm")]
pub mod wasm;
