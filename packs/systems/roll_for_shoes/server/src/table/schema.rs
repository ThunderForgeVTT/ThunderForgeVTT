//! The table that holds what the Game Master has said has to be beaten.
//!
//! Declared here rather than in `src/server/src/schema.rs`, per ADR-063, for
//! the reason `settings/schema.rs` gives: a pack owns the tables it writes.
//! Both `diesel.toml` files keep it out of the server's generated schema.

diesel::table! {
    /// One world's standing difficulty.
    ///
    /// A row means the Game Master has set one; no row means they have not,
    /// and every sheet falls back to the free entry it has always had.
    world_roll_for_shoes_difficulty (world_id) {
        world_id -> Uuid,
        target -> Int4,
        band -> Nullable<Text>,
        gm_dice -> Nullable<Jsonb>,
        set_by -> Nullable<Uuid>,
        set_at -> Timestamp,
    }
}
