//! The one table this pack owns.
//!
//! Declared here rather than in `crates/thunderforge-server/src/schema.rs`, per ADR-063: a
//! pack owns the tables it writes. Both `diesel.toml` files list this table
//! under `except_tables`, so `diesel print_schema` never adds a second
//! declaration of it to the server's generated schema.
//!
//! The migration that creates it still lives in `crates/thunderforge-server/migrations/` —
//! Diesel reads one migrations directory per invocation, and splitting that is
//! a separate decision from owning the declaration.

diesel::table! {
    /// A world's answers to the five optional rules Roll for Shoes leaves to
    /// the table (spec 062).
    ///
    /// There is no row until a Game Master changes something. A missing row
    /// reads as every default, which is what makes the feature opt-in.
    world_roll_for_shoes_settings (world_id) {
        world_id -> Uuid,
        #[max_length = 16]
        difficulty_mode -> Text,
        tie_succeeds -> Bool,
        statuses_enabled -> Bool,
        skill_slots_enabled -> Bool,
        starting_skills -> Jsonb,
        updated_by -> Nullable<Uuid>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}
