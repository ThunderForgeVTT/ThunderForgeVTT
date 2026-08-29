//! Who may do what to a world, and who may see which scenes.
//!
//! # Why this is its own crate
//!
//! These are decisions, not queries. A role, an ownership fact and a scene
//! flag go in; an allow or a deny comes out. But they lived inside the server
//! welded to diesel and a live Postgres, so the only way to ask "may a Player
//! delete a world" was to stand up a database and try it.
//!
//! Three holes survived exactly that gap, and none of them was hard to write:
//!
//! - `deleteWorld` accepted **any world member**, so a Player who had merely
//!   accepted an invite could destroy the world.
//! - `createScene` had **no membership check at all** — any signed-in user
//!   could add a scene to any world by id.
//! - `updateFogMask` likewise, and reveal is the dangerous direction: an
//!   attacker could uncover a map the GM was deliberately withholding.
//!
//! Each was a place where somebody had to remember a rule that was written
//! down nowhere, and no test could cheaply have caught any of them. Here the
//! rules are values, the permission matrix is enumerated in full, and adding
//! a capability fails to compile until every role has a stated answer for it.
//!
//! # What is deliberately not here
//!
//! Everything that needs a database: looking up the caller's `world_members`
//! row, the `worlds.created_by` fallback, the SQL that narrows a sync plan to
//! visible scenes. The server keeps those, resolves them into the plain values
//! below, and asks this crate the question.
//!
//! # Failing closed
//!
//! Every unknown answers no. An unparseable role string is not a member; an
//! actor with no role has no capabilities. A typo in a migration must not
//! become an authorization bypass.

pub mod capability;
pub mod role;
pub mod scene;

pub use capability::{Actor, Capability, role_allows};
pub use role::Role;
pub use scene::{Scene, asset_visible, scene_visible};
