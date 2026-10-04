//! What a re-import did to the worlds reading a book (spec 050 US5, FR-026,
//! FR-027).
//!
//! A re-import replaces a book's base and touches no delta. Deltas attach by
//! kind and name, so every one whose entry is still in the new reading goes on
//! applying without anything being done to it. The ones that do not — the
//! entry is gone, a rename made it a removal plus an addition, a second entry
//! of that name appeared, or the book now has an entry an addition was
//! standing in for — are still stored and still the world's. This names them,
//! per world, so the person who re-read the book is told what their re-read
//! stranded, rather than each table finding out on its own.
//!
//! The rule for *why* a delta does not attach is [`super::resolve`]'s and is
//! not restated here. This only runs it over every world at once.

use std::collections::BTreeMap;

use diesel::prelude::*;
use uuid::Uuid;

use crate::compendium::store::StoredEntry;
use crate::schema::{compendium_entries, world_entry_deltas, worlds};

use super::{Delta, Unattached, origin_of_book, resolve};

/// One world's deltas over a book that no longer attach to it.
#[derive(Debug, Clone)]
pub struct WorldUnattached {
    pub world_id: Uuid,
    pub world_name: String,
    /// Each delta, with why it is not being applied. Never empty: a world
    /// whose deltas all attach is not listed.
    pub deltas: Vec<(Delta, Unattached)>,
}

/// Every world's deltas over `compendium_id` that do not attach to the base in
/// force, ordered by world name.
///
/// Not gated: the caller is the one who decides who may ask. A book's deltas
/// are its owner's worlds' (a book list draws only on the world owner's
/// shelf), so the owner is who the resolver lets ask.
pub fn unattached_after_reimport(
    conn: &mut PgConnection,
    compendium_id: Uuid,
) -> QueryResult<Vec<WorldUnattached>> {
    let held: Vec<(Delta, String)> = world_entry_deltas::table
        .inner_join(worlds::table.on(worlds::id.eq(world_entry_deltas::world_id)))
        .filter(world_entry_deltas::compendium_id.eq(compendium_id))
        .select((Delta::as_select(), worlds::name))
        .load(conn)?;
    if held.is_empty() {
        return Ok(Vec::new());
    }

    let base: Vec<StoredEntry> = compendium_entries::table
        .filter(compendium_entries::compendium_id.eq(compendium_id))
        .select(StoredEntry::as_select())
        .load(conn)?;
    let book_origin = origin_of_book(conn, compendium_id)?;

    let mut by_world: BTreeMap<(String, Uuid), Vec<Delta>> = BTreeMap::new();
    for (delta, world_name) in held {
        by_world
            .entry((world_name, delta.world_id))
            .or_default()
            .push(delta);
    }

    Ok(by_world
        .into_iter()
        .filter_map(|((world_name, world_id), deltas)| {
            let unattached = resolve(book_origin, base.clone(), deltas, false).unattached;
            (!unattached.is_empty()).then_some(WorldUnattached {
                world_id,
                world_name,
                deltas: unattached,
            })
        })
        .collect())
}

#[cfg(test)]
#[path = "reimport_tests.rs"]
mod tests;
