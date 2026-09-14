//! Prints the merged GraphQL schema as SDL, with no database and no server.
//!
//!     cargo run -q -p thunderforge --bin thunderforge-schema --features schema-sdl
//!
//! # Why this exists
//!
//! `apps/web` speaks to the server in hand-written query strings, and nothing
//! compiled the two against each other: a renamed field or argument on the
//! Rust side compiled, every Rust test passed, and the first thing to notice
//! was a person clicking a button. `scripts/check-graphql-contract.mjs` closes
//! that gap by validating every operation the web sends against this output,
//! which is committed as `src/app/schema.graphql` so a schema change is also a
//! line in the review diff.
//!
//! # Why a binary of its own, behind a feature
//!
//! The schema is composed here, in the app crate, because this is the one
//! place that names the packs' roots (`schema_roots.rs`). Printing it needs
//! only the types: `Schema::build(..).finish()` registers them and touches no
//! state, which is why nothing below builds an `AppState`.
//!
//! The feature keeps `cargo build -p thunderforge` — which the dev server, the
//! e2e runner and the release build all use — from linking a second
//! executable that none of them run. Enabling a feature on this package alone
//! changes no dependency's feature set, so the check reuses the dev build's
//! compiled server rather than compiling another copy of it.

// The same type-layout depth `main.rs` raises its limit for: instantiating
// the merged schema is what needs it, and this binary does exactly that.
#![recursion_limit = "512"]

#[path = "../schema_roots.rs"]
mod schema_roots;

use schema_roots::{AppMutationRoot, AppQueryRoot, AppSchema};
use std::io::Write;

fn main() {
    // Typed as the server's own alias, so this cannot print a schema of a
    // different shape from the one `main.rs` serves.
    let schema: AppSchema = async_graphql::Schema::build(
        AppQueryRoot::default(),
        AppMutationRoot::default(),
        thunderforge_server::graphql::SubscriptionRoot,
    )
    .finish();
    // Written, not `print!`ed: stdout *is* this binary's output, and a failed
    // write must fail the check rather than hand it a truncated schema.
    std::io::stdout()
        .lock()
        .write_all(schema.sdl().as_bytes())
        .expect("could not write the schema to stdout");
}
