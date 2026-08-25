//! Spec 013: item ownership/permission enforcement — direct structural
//! mirror of `auth::actor_permissions` (spec 010), generalized to items.
//! The world's DM (Owner or GM role) always has implicit, un-removable
//! `Owner`-equivalent access to every item in their world; every other
//! member defaults to `Viewer` unless an explicit `world_item_permissions`
//! row says otherwise. See specs/013-items-inventory/research.md.



// Spec 027 (US5): the `effective_item_permission` / `require_item_permission` pair that
// lived here is now generated from the single declaration in
// `auth::permissioned_entities`, under the same names and signatures — so no
// caller changed. Three other modules carried a near-verbatim copy of the same
// logic; one of them shipped without its member-removal cleanup, which is the
// privilege leak that motivated consolidating them.
//
// Re-exported here so existing import paths keep working.
pub use crate::auth::permissioned_entities::{effective_item_permission, require_item_permission};

