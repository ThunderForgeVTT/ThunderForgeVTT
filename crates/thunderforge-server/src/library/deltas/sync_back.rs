//! Syncing a world's changes back to the collection they were made over
//! (spec 050 US6, FR-100 to FR-105, ADR-098).
//!
//! # Collections only, at any volume
//!
//! An imported book records what a document says, and it stays that however
//! much a world has changed it (FR-101, decision 5). There are three refusals,
//! so that no one of them has to be remembered by a route:
//!
//! * [`plan_sync_back`] and [`sync_back`] refuse a book read in before looking
//!   at a single delta, with the reason a person is shown ([`DeltaError::BookNeverSyncsBack`]);
//! * a sync back writes the collection's previous version **before** it
//!   writes anything else, and the database refuses a version of anything but
//!   a collection;
//! * nothing in this module takes a count or a selection, so no volume of
//!   change is a different path.
//!
//! # What is shown is what lands
//!
//! A sync back writes to something every other world reading the collection
//! reads (FR-103). [`plan_sync_back`] lays out every change it would make to
//! the shelf, and what stays in the world because it cannot attach, with a
//! [`SyncPlan::stamp`] naming the version and the deltas it was computed from.
//! [`sync_back`] recomputes the plan inside its transaction and refuses if the
//! stamp differs, so a person never confirms one set of changes and gets
//! another.
//!
//! # After it lands
//!
//! The collection holds the changes as a new version and the previous one is
//! kept (FR-104). This world no longer holds them as deltas (FR-105) — they
//! are the base now. Other worlds' deltas are not touched: they attach by kind
//! and name as they do after a re-read, and whatever no longer attaches is
//! reported to the person who synced, by [`super::unattached_after_reimport`].
//! Deltas that did not attach in this world are not synced and stay where
//! they are.
//!
//! # Whose shelf
//!
//! The collection is on the world owner's shelf (FR-010a), and a sync back
//! writes to that shelf rather than to the table, so only the world's owner
//! may do it. A Game Master or Trusted Player may change what the table
//! inherited; changing what the owner's other tables inherit is the owner's.

use diesel::prelude::*;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::compendium::ContentOrigin;
use crate::compendium::collections::next_version;
use crate::compendium::store::StoredEntry;
use crate::compendium::versions::{self, VersionEntry};
use crate::library::book_list::{self, BookListError};
use crate::play_pause::gate::refuse_if_paused;
use crate::schema::{compendium_entries, compendiums, world_entry_deltas, worlds};

use super::{
    Before, Delta, DeltaError, EntryState, Unattached, WorldUnattached, deltas_over, resolve,
    unattached_after_reimport,
};

/// What a sync back does to one entry on the shelf.
#[derive(Debug, Clone, PartialEq)]
pub enum ShelfChange {
    /// The collection's entry takes what this world changed it to.
    Rewritten {
        kind: String,
        name: String,
        before: Before,
        after: Before,
    },
    /// This world hid the entry, so the collection no longer holds it.
    TakenOut {
        kind: String,
        name: String,
        before: Before,
    },
    /// This world wrote the entry, so the collection gains it.
    Added {
        kind: String,
        name: String,
        after: Before,
    },
}

/// Everything a sync back would do, shown before it is confirmed (FR-103).
#[derive(Debug, Clone)]
pub struct SyncPlan {
    pub collection_id: Uuid,
    pub collection_title: String,
    pub world_name: String,
    /// The version in force now; landing makes the next one.
    pub base_version: i32,
    pub changes: Vec<ShelfChange>,
    /// This world's deltas that do not attach, which stay in the world.
    pub staying: Vec<(Delta, Unattached)>,
    /// Names the version and deltas this plan was made from. A sync back
    /// carrying a different stamp is refused.
    pub stamp: String,
}

/// What a landed sync back did.
#[derive(Debug, Clone)]
pub struct SyncOutcome {
    pub plan: SyncPlan,
    /// The version the collection is now at.
    pub base_version: i32,
    /// Every world's deltas that no longer attach to the new version, this
    /// world's included, named as a re-read names them.
    pub stranded: Vec<WorldUnattached>,
}

/// What a sync back of this world's changes over `compendium_id` would do,
/// without doing it.
pub fn plan_sync_back(
    conn: &mut PgConnection,
    caller: Uuid,
    world_id: Uuid,
    compendium_id: Uuid,
) -> Result<SyncPlan, DeltaError> {
    may_sync(conn, caller, world_id, compendium_id)?;
    plan(conn, world_id, compendium_id)
}

/// Sync this world's changes over a collection back to it (FR-100, FR-104,
/// FR-105), if they are still the ones `stamp` names.
pub fn sync_back(
    conn: &mut PgConnection,
    caller: Uuid,
    world_id: Uuid,
    compendium_id: Uuid,
    stamp: &str,
) -> Result<SyncOutcome, DeltaError> {
    // The pause before the book, as every write to a world's books has it: a
    // paused table is told it is paused, whatever else is true.
    book_list::require_book_manager(conn, caller, world_id)?;
    refuse_if_paused(conn, world_id).map_err(BookListError::Paused)?;
    may_sync(conn, caller, world_id, compendium_id)?;

    conn.transaction(|conn| {
        // Held for the transaction, so a second sync back, an entry written
        // on the shelf or a re-read cannot land between the plan and the
        // writes.
        compendiums::table
            .filter(compendiums::id.eq(compendium_id))
            .for_update()
            .select(compendiums::id)
            .first::<Uuid>(conn)?;

        let plan = plan(conn, world_id, compendium_id)?;
        if plan.stamp != stamp {
            return Err(DeltaError::SyncPlanChanged);
        }
        if plan.changes.is_empty() {
            return Err(DeltaError::NothingToSync);
        }

        // First, before anything is written: the version being replaced. The
        // database refuses it for anything but a collection (FR-101).
        versions::record(
            conn,
            compendium_id,
            &format!("Synced from {}", plan.world_name),
        )
        .map_err(|e| refused_as_a_book(e, &plan.collection_title))?;

        let base = base_entries(conn, compendium_id)?;
        let id_of = |kind: &str, name: &str| {
            base.iter()
                .find(|entry| entry.kind == kind && entry.name == name)
                .map(|entry| entry.id)
        };
        for change in &plan.changes {
            match change {
                ShelfChange::Rewritten {
                    kind, name, after, ..
                } => {
                    let id = id_of(kind, name).ok_or(DeltaError::SyncPlanChanged)?;
                    diesel::update(compendium_entries::table.filter(compendium_entries::id.eq(id)))
                        .set((
                            compendium_entries::field_values.eq(&after.field_values),
                            compendium_entries::prose_text.eq(&after.prose_text),
                        ))
                        .execute(conn)?;
                }
                ShelfChange::TakenOut { kind, name, .. } => {
                    let id = id_of(kind, name).ok_or(DeltaError::SyncPlanChanged)?;
                    diesel::delete(compendium_entries::table.filter(compendium_entries::id.eq(id)))
                        .execute(conn)?;
                }
                ShelfChange::Added { kind, name, after } => {
                    versions::insert_entry(
                        conn,
                        compendium_id,
                        VersionEntry {
                            kind: kind.clone(),
                            name: name.clone(),
                            name_uncertain: false,
                            field_values: after.field_values.clone(),
                            prose_text: after.prose_text.clone(),
                            suspect: false,
                            extras: None,
                        },
                    )?;
                }
            }
        }

        // FR-105: the changes are the base now, so the world stops holding
        // them. Only the synced ones; what did not attach stays.
        let synced: Vec<(String, String)> = plan
            .changes
            .iter()
            .map(|change| {
                let (kind, name) = change.identity();
                (kind.to_string(), name.to_string())
            })
            .collect();
        for (kind, name) in &synced {
            diesel::delete(
                world_entry_deltas::table
                    .filter(world_entry_deltas::world_id.eq(world_id))
                    .filter(world_entry_deltas::compendium_id.eq(compendium_id))
                    .filter(world_entry_deltas::kind.eq(kind))
                    .filter(world_entry_deltas::name.eq(name)),
            )
            .execute(conn)?;
        }

        next_version(conn, caller, compendium_id)
            .map_err(|e| DeltaError::Database(e.to_string()))?;
        let base_version: i32 = compendiums::table
            .filter(compendiums::id.eq(compendium_id))
            .select(compendiums::base_version)
            .first(conn)?;
        let stranded = unattached_after_reimport(conn, compendium_id)?;

        Ok(SyncOutcome {
            plan,
            base_version,
            stranded,
        })
    })
}

impl ShelfChange {
    /// The kind and name of the entry this change is to.
    pub fn identity(&self) -> (&str, &str) {
        match self {
            ShelfChange::Rewritten { kind, name, .. }
            | ShelfChange::TakenOut { kind, name, .. }
            | ShelfChange::Added { kind, name, .. } => (kind, name),
        }
    }
}

/// Who may sync, and to what: the world's owner, over a collection on this
/// world's list (see the module documentation).
fn may_sync(
    conn: &mut PgConnection,
    caller: Uuid,
    world_id: Uuid,
    compendium_id: Uuid,
) -> Result<(), DeltaError> {
    let owner = book_list::require_book_manager(conn, caller, world_id)?;
    book_list::require_served(conn, world_id, compendium_id)?;
    let (origin, title): (ContentOrigin, String) = compendiums::table
        .filter(compendiums::id.eq(compendium_id))
        .select((compendiums::origin, compendiums::book_title))
        .first(conn)?;
    // The book first, whoever asks: the answer for a book read in does not
    // depend on who is asking, and a Game Master looking for the sync on one
    // is owed the real reason rather than "ask the owner".
    if origin != ContentOrigin::Authored {
        return Err(DeltaError::BookNeverSyncsBack { title });
    }
    if caller != owner {
        return Err(DeltaError::OnlyTheOwnerSyncs);
    }
    Ok(())
}

fn plan(
    conn: &mut PgConnection,
    world_id: Uuid,
    compendium_id: Uuid,
) -> Result<SyncPlan, DeltaError> {
    let (collection_title, base_version): (String, i32) = compendiums::table
        .filter(compendiums::id.eq(compendium_id))
        .select((compendiums::book_title, compendiums::base_version))
        .first(conn)?;
    let world_name: String = worlds::table
        .filter(worlds::id.eq(world_id))
        .select(worlds::name)
        .first(conn)?;

    let base = base_entries(conn, compendium_id)?;
    let before_of = |kind: &str, name: &str| {
        base.iter()
            .find(|entry| entry.kind == kind && entry.name == name)
            .map(|entry| Before {
                field_values: entry.field_values.clone(),
                prose_text: entry.prose_text.clone(),
            })
    };
    let deltas = deltas_over(conn, world_id, compendium_id, None)?;
    let stamp = stamp_of(base_version, &deltas);
    let resolution = resolve(ContentOrigin::Authored, base.clone(), deltas, true);

    let mut changes = Vec::new();
    for entry in resolution.entries {
        let after = Before {
            field_values: entry.field_values,
            prose_text: entry.prose_text,
        };
        match entry.state {
            EntryState::Inherited => {}
            EntryState::Changed => {
                // A resolved change always has a before; the fallback reads
                // the base again rather than trusting that.
                let Some(before) = entry.before.or_else(|| before_of(&entry.kind, &entry.name))
                else {
                    continue;
                };
                changes.push(ShelfChange::Rewritten {
                    kind: entry.kind,
                    name: entry.name,
                    before,
                    after,
                });
            }
            EntryState::Hidden => changes.push(ShelfChange::TakenOut {
                kind: entry.kind,
                name: entry.name,
                before: after,
            }),
            EntryState::Added => changes.push(ShelfChange::Added {
                kind: entry.kind,
                name: entry.name,
                after,
            }),
        }
    }

    Ok(SyncPlan {
        collection_id: compendium_id,
        collection_title,
        world_name,
        base_version,
        changes,
        staying: resolution.unattached,
        stamp,
    })
}

fn base_entries(conn: &mut PgConnection, compendium_id: Uuid) -> QueryResult<Vec<StoredEntry>> {
    compendium_entries::table
        .filter(compendium_entries::compendium_id.eq(compendium_id))
        .order((
            compendium_entries::kind.asc(),
            compendium_entries::name.asc(),
        ))
        .select(StoredEntry::as_select())
        .load(conn)
}

/// The version in force and each delta as it last stood. Deltas come ordered
/// by kind and name, so the same state always stamps the same.
fn stamp_of(base_version: i32, deltas: &[Delta]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(base_version.to_be_bytes());
    for delta in deltas {
        hasher.update(delta.id.as_bytes());
        hasher.update(delta.updated_at.and_utc().timestamp_micros().to_be_bytes());
    }
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// The database's refusal of a version, in the words a person is shown.
fn refused_as_a_book(error: diesel::result::Error, title: &str) -> DeltaError {
    match &error {
        diesel::result::Error::DatabaseError(_, info)
            if info.message().contains("spec 050 FR-101") =>
        {
            DeltaError::BookNeverSyncsBack {
                title: title.to_string(),
            }
        }
        _ => error.into(),
    }
}

#[cfg(test)]
#[path = "sync_back_tests.rs"]
mod tests;
