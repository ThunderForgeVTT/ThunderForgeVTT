//! Spec 026: content collections.
//!
//! A collection is a named set of one world's artifacts, shared by one
//! unguessable link and copied whole into another world as independent
//! records. Governed by ADR-069 (the DMCA determination, which accepts a
//! stated risk) and ADR-070 (the anonymous read path).
//!
//! # Why this is a module rather than a fourth `mutations_*_shares.rs`
//!
//! Three single-artifact share implementations already exist — ability, item
//! and actor — totalling ~1,800 lines of near-duplicate code that already
//! diverge in small ways (the ability copy re-validates effect formulas and
//! preserves `gm_only`; the item copy does neither). A fourth copy would make
//! four. The GraphQL layer stays thin and delegates here, so the logic a test
//! can exhaust lives in one place.
//!
//! # The two invariants everything here rests on
//!
//! **Nothing is enumerable.** FR-020 forbids any query that lists collections
//! by world, by user, or globally, beyond a caller's own. ADR-069's
//! determination is conditional on it — it is the property that keeps a
//! link-shared collection from being a repository. Do not add a listing path.
//!
//! **Nothing is cached about a member's status.** Moderation and restriction
//! are asked fresh on every read, which is why `world_collection_members`
//! carries no `disabled` or `restricted` column. See [`resolve`].

pub mod copy;
pub mod membership;
pub mod rate_limit;
pub mod resolve;
pub mod scene_copy;

/// The five member types a collection may hold (FR-002).
///
/// Stored as the string in `world_collection_members.member_type`.
pub const MEMBER_TYPES: &[&str] = &["actor", "item", "ability", "lore", "scene"];

/// FR-005a: a collection holds at most this many members.
///
/// A count rather than a byte ceiling. It is what a person can reason about,
/// and what decides whether copying stays a single action the recipient waits
/// out (SC-002a) rather than a background job with progress and resumption.
pub const MAX_MEMBERS: i64 = 100;

/// Whether `member_type` is one this feature knows.
pub fn is_known_member_type(member_type: &str) -> bool {
    MEMBER_TYPES.contains(&member_type)
}

/// The moderation entity type for a member type, as
/// `moderation::effective_status` expects it.
///
/// `None` for a type spec 015's moderation does not track. A member whose type
/// has no moderation entity is not thereby un-moderatable — it is a gap, and
/// naming it here is what makes the gap visible rather than silent.
pub fn moderation_entity_type(member_type: &str) -> Option<&'static str> {
    match member_type {
        "actor" => Some("world_actor"),
        "item" => Some("world_item"),
        "ability" => Some("world_ability"),
        "lore" => Some("world_lore_entry"),
        // Scenes are not a moderated entity type in spec 015 today. A takedown
        // against a scene is filed against its images, which are separate
        // entities. Recorded rather than assumed away.
        "scene" => None,
        _ => None,
    }
}
