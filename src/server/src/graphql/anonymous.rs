//! The caller identity behind every share read that does not authenticate.
//!
//! Four resolvers now resolve without a session — `sharedCollection`
//! (ADR-070) and `sharedAbility`, `sharedItem`, `sharedActor` (ADR-071) — and
//! each must rate-limit before its lookup, because an unguessable code is
//! unguessable only while the number of guesses is bounded.
//!
//! This lived in `mutations_collection_shares` while collections were the only
//! anonymous path. It moved here when the other three joined them: a newtype
//! four resolvers depend on is not owned by one of them, and leaving it there
//! would have made three modules import from a fourth for a reason unrelated to
//! collections.

use async_graphql::Context;

/// The caller's identity for rate-limiting purposes, put into the GraphQL
/// context by the public transport handler.
///
/// A newtype rather than a bare `String` so nothing else in the context can be
/// mistaken for it.
#[derive(Clone, Debug)]
pub struct AnonymousCaller(pub String);

/// The caller identity to rate-limit against, for a resolver that does not
/// authenticate.
///
/// An absent identity means the transport did not supply one. Falling back to a
/// shared bucket is the safe way to be wrong: it rate-limits such callers
/// together rather than exempting them.
///
/// Written once rather than four times so the four anonymous reads cannot come
/// to disagree about what an unidentified caller is — the same reasoning that
/// makes each module's refusal a constant instead of a repeated literal.
pub fn caller_id(ctx: &Context<'_>) -> String {
    ctx.data_opt::<AnonymousCaller>()
        .map(|c| c.0.clone())
        .unwrap_or_else(|| "unknown".to_string())
}
