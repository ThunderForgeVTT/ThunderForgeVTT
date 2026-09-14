//! How a pack says which of its root GraphQL fields a pause refuses.
//!
//! # Why a pack has to say
//!
//! `graphql::play_pause_surface_tests` closes the list of root fields a pause
//! refuses, but it reads this crate's schema, and a system pack's fields are
//! merged in only by the app crate (`src/app/src/schema_roots.rs`). So a pack
//! classifies its own fields here, beside the code that defines them, and the
//! app crate's `play_pause_surface_tests` holds the **merged** schema to the
//! union: a pack root field in no table fails that build, and every field a
//! pack lists as gated is called against a paused world and must answer
//! `WORLD_PLAY_PAUSED`.
//!
//! # Why it is discovered
//!
//! For the reason `world_hooks` is: nothing in shared code may list packs by
//! name (FR-029). A pack submits one [`PackSurface`] through `inventory`, and
//! the test collects whatever is linked. A pack that contributes root fields
//! and submits nothing is not missed quietly — its fields are unclassified.
//!
//! The documents are data, not code: a few static strings per field, the same
//! shape as the server's own table.

/// A step that makes a row a gated request names, run by the world's Owner
/// through the merged schema before the world is paused.
pub struct SeedStep {
    /// The placeholder later documents use, `{key}`.
    pub key: &'static str,
    /// The root field the document calls.
    pub field: &'static str,
    /// The document. `{world}` and any earlier step's `{key}` are filled in.
    pub document: &'static str,
    /// A JSON pointer into the field's value, naming the id to keep.
    pub pick: &'static str,
}

/// One pack's classification of the root fields it contributes.
///
/// Every root `Query`, `Mutation` and `Subscription` field a pack merges in
/// must be in exactly one of `gated`, `not_world_scoped` and `reads`.
pub struct PackSurface {
    /// Matches the pack's manifest `id`. For messages only.
    pub system_id: &'static str,
    /// World-scoped: each calls `gate::refuse_world_if_paused` (or its
    /// siblings) beside its role check, before any write. `(field, document)`,
    /// run as the world's Game Master and as a site admin who is a member.
    pub gated: &'static [(&'static str, &'static str)],
    /// Touches no single world.
    pub not_world_scoped: &'static [&'static str],
    /// Read-only queries, answered while paused (FR-024, *readable, not
    /// editable*). A query that starts play belongs in `gated`.
    pub reads: &'static [&'static str],
    /// Rows the gated documents name, made in order before the pause.
    pub seed: &'static [SeedStep],
}

inventory::collect!(PackSurface);

/// Every pack surface linked into this binary.
pub fn pack_surfaces() -> impl Iterator<Item = &'static PackSurface> {
    inventory::iter::<PackSurface>.into_iter()
}
