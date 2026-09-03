//! Links the bundled system packs into this crate's **test** binary only.
//!
//! # Why this exists, separately from `src/app/src/system_packs.rs`
//!
//! Same load-bearing fact, different binary. A statically linked Rust crate
//! that nothing references is never linked, and its `inventory` submissions
//! go with it. The application's linkage lives in `src/app` because that is
//! the composition root; this crate's *tests* are their own binary and link
//! nothing, so six tests that assert what a pack contributes — a Genie actor's
//! derived Wish Points, a 5e actor's modifiers, the Fate and Cypher packs
//! validating against their own systems — collected an empty set the moment
//! the server became a library.
//!
//! These are `[dev-dependencies]`, which is what makes the arrangement legal:
//! Cargo permits a dev-dependency cycle, so a pack may depend on this crate
//! while this crate's tests depend on the pack. A normal build has neither.
//!
//! A test asserting "a Genie actor derives its Wish Points" has to name Genie;
//! `scripts/check-system-registry.mjs` exempts tests for exactly that reason.

use blades_server as _;
use cypher_server as _;
use dnd5e_server as _;
use fate_server as _;
use genie_server as _;
use pathfinder2e_server as _;
use yze_server as _;
