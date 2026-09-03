//! The GraphQL schema, core plus whatever the packs contribute.
//!
//! # Why this is in the binary
//!
//! `async-graphql` composes a schema from types named at compile time, so
//! *something* has to name a pack's query and mutation types. That looked
//! like the registry FR-029 forbids, and it is not: a `MergedObject` entry
//! carries no information that can drift. It says a type exists and should be
//! merged, and nothing about that system's data shapes, validators or rules.
//! If the pack changes its mutations the entry does not; if the type goes
//! away the build fails loudly rather than drifting quietly.
//!
//! That is the same argument that exempts `system_packs.rs`'s
//! `use <pack> as _;` lines, and it is why both live here, in the crate whose
//! whole job is composition. The distinction worth holding is between shared
//! code *deciding* something per system at runtime — which is the violation —
//! and shared code *composing* at build time, which is a dependency.
//!
//! See `specs/032-pack-architecture/research.md` § F-5 and ADR-063.

use async_graphql::MergedObject;

/// Everything queryable: the product's own roots, then each pack's.
#[derive(MergedObject, Default)]
pub struct AppQueryRoot(
    thunderforge_server::graphql::QueryRoot,
    genie_server::GenieSessionQuery,
);

/// Everything mutable, same shape.
#[derive(MergedObject, Default)]
pub struct AppMutationRoot(
    thunderforge_server::graphql::MutationRoot,
    genie_server::GenieSessionMutation,
);

pub type AppSchema = async_graphql::Schema<
    AppQueryRoot,
    AppMutationRoot,
    thunderforge_server::graphql::SubscriptionRoot,
>;
