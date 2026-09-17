//! A collection's earlier versions (spec 050 FR-104, ADR-098).
//!
//! Every change to a collection moves `base_version` on: an entry written or
//! taken out, a world's changes synced back, an earlier version restored.
//! [`record`] keeps the version being moved on *from*, in the same
//! transaction as the change, so a regretted change can be undone and a
//! failed one leaves nothing behind.
//!
//! # The conditions this module is written against
//!
//! ADR-098 accepted versioned collections on three conditions. How each holds
//! here:
//!
//! 1. **An earlier version is readable by its owner alone.** Every public
//!    function here asks [`require_account_owner`] first. A shelf collection
//!    has no share link and no adoption, so there is no second audience to
//!    leak history to — and nothing here takes a link or a world.
//! 2. **Sync-back never reaches an adopted copy.** There are no adopted copies
//!    of a shelf collection. A world reads one through its book list, live,
//!    which is how it meets a new version at all.
//! 3. **A takedown withholds every version.** Every read of an earlier
//!    version goes through [`read_at`], and nothing else in the crate reads
//!    this table's `entries`. No notice can name a shelf collection's entry
//!    today, the version in force included — the owner's reading, recorded in
//!    ADR-098 on 2026-09-16. When one can, the check goes in [`read_at`], which
//!    covers every earlier version at once, and in the two reads of the
//!    version in force: the download and a world's book list.
//!
//! # Collections only
//!
//! An imported book is re-read, and its previous reading is not kept. The
//! database refuses a version of one, which is also what makes a sync back to
//! a book impossible by any route: a sync back writes its version first.

use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::account_ownership::{AccountOwned, require_account_owner};
use crate::compendium::collections::{CollectionError, next_version};
use crate::compendium::origin::ContentOrigin;
use crate::compendium::store::{Compendium, StoredEntry};
use crate::schema::{compendium_entries, compendiums, shelf_collection_versions};

/// One entry as a version holds it: its identity and what it said. Not its
/// id, which a restore gives afresh.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionEntry {
    pub kind: String,
    pub name: String,
    pub name_uncertain: bool,
    pub field_values: serde_json::Value,
    pub prose_text: Option<String>,
    pub suspect: bool,
    pub extras: Option<serde_json::Value>,
}

impl From<StoredEntry> for VersionEntry {
    fn from(entry: StoredEntry) -> Self {
        VersionEntry {
            kind: entry.kind,
            name: entry.name,
            name_uncertain: entry.name_uncertain,
            field_values: entry.field_values,
            prose_text: entry.prose_text,
            suspect: entry.suspect,
            extras: entry.extras,
        }
    }
}

/// An earlier version, named without its content.
#[derive(Debug, Clone, PartialEq)]
pub struct PastVersion {
    pub version: i32,
    pub book_title: String,
    pub entry_counts: serde_json::Value,
    pub entry_total: i32,
    /// What moved the collection on from this version, in its owner's words.
    pub replaced_by: String,
    pub replaced_at: chrono::NaiveDateTime,
}

/// Which version of a collection to read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum At {
    /// The version in force.
    Current,
    /// An earlier one, by number.
    Version(i32),
}

/// Keep the collection's version in force before something replaces it.
///
/// Call inside the transaction that makes the change, **before** writing
/// anything else, so the change cannot land without it. Refused by the
/// database for anything but a collection.
pub(crate) fn record(
    conn: &mut PgConnection,
    collection_id: Uuid,
    replaced_by: &str,
) -> QueryResult<()> {
    let collection: Compendium = compendiums::table
        .filter(compendiums::id.eq(collection_id))
        // Two changes to one collection at once would both record the same
        // version and the second would fail on the unique key. Waiting is
        // the better answer.
        .for_update()
        .select(Compendium::as_select())
        .first(conn)?;
    let entries = entries_in_force(conn, collection_id)?;
    let entries = serde_json::to_value(&entries)
        .map_err(|e| diesel::result::Error::SerializationError(Box::new(e)))?;
    diesel::insert_into(shelf_collection_versions::table)
        .values((
            shelf_collection_versions::id.eq(Uuid::now_v7()),
            shelf_collection_versions::compendium_id.eq(collection_id),
            shelf_collection_versions::version.eq(collection.base_version),
            shelf_collection_versions::book_title.eq(&collection.book_title),
            shelf_collection_versions::entries.eq(entries),
            shelf_collection_versions::entry_counts.eq(&collection.entry_counts),
            shelf_collection_versions::replaced_by.eq(truncated(replaced_by)),
        ))
        .execute(conn)?;
    Ok(())
}

/// The collection's earlier versions, newest first (FR-104). Its owner's
/// alone.
pub fn history(
    conn: &mut PgConnection,
    caller: Uuid,
    collection_id: Uuid,
) -> Result<Vec<PastVersion>, CollectionError> {
    require_account_owner(conn, caller, AccountOwned::Compendium(collection_id))?;
    let rows: Vec<(
        i32,
        String,
        serde_json::Value,
        String,
        chrono::NaiveDateTime,
    )> = shelf_collection_versions::table
        .filter(shelf_collection_versions::compendium_id.eq(collection_id))
        .order(shelf_collection_versions::version.desc())
        .select((
            shelf_collection_versions::version,
            shelf_collection_versions::book_title,
            shelf_collection_versions::entry_counts,
            shelf_collection_versions::replaced_by,
            shelf_collection_versions::replaced_at,
        ))
        .load(conn)?;
    Ok(rows
        .into_iter()
        .map(
            |(version, book_title, entry_counts, replaced_by, replaced_at)| PastVersion {
                version,
                book_title,
                entry_total: total_of(&entry_counts),
                entry_counts,
                replaced_by,
                replaced_at,
            },
        )
        .collect())
}

/// What a collection held at a version, ordered by kind and name. Its
/// owner's alone.
///
/// **The one read of an earlier version** (ADR-098 condition 3), so whatever
/// withholds content here withholds it from every version at once.
pub fn read_at(
    conn: &mut PgConnection,
    caller: Uuid,
    collection_id: Uuid,
    at: At,
) -> Result<Vec<VersionEntry>, CollectionError> {
    require_account_owner(conn, caller, AccountOwned::Compendium(collection_id))?;
    let mut entries = match at {
        At::Current => entries_in_force(conn, collection_id)?,
        At::Version(version) => {
            let held: serde_json::Value = shelf_collection_versions::table
                .filter(shelf_collection_versions::compendium_id.eq(collection_id))
                .filter(shelf_collection_versions::version.eq(version))
                .select(shelf_collection_versions::entries)
                .first(conn)
                .optional()?
                .ok_or(CollectionError::NoSuchVersion { version })?;
            serde_json::from_value(held).map_err(|e| CollectionError::Unstorable(e.to_string()))?
        }
    };
    entries.sort_by(|left, right| (&left.kind, &left.name).cmp(&(&right.kind, &right.name)));
    Ok(entries)
}

/// Put an earlier version back (FR-104, US6 scenario 6).
///
/// A restore is itself a change, so it is a **new** version holding the old
/// one's entries, and the version it replaced is kept like any other. Nothing
/// is rewound, so a restore can be regretted too. Worlds reading the
/// collection meet it as they meet a re-read book: their deltas attach by kind
/// and name, and what no longer attaches is reported, not dropped.
pub fn restore(
    conn: &mut PgConnection,
    caller: Uuid,
    collection_id: Uuid,
    version: i32,
) -> Result<Compendium, CollectionError> {
    let entries = read_at(conn, caller, collection_id, At::Version(version))?;
    conn.transaction(|conn| {
        refuse_unless_authored(conn, collection_id)?;
        record(
            conn,
            collection_id,
            &format!("Went back to version {version}"),
        )?;
        diesel::delete(
            compendium_entries::table.filter(compendium_entries::compendium_id.eq(collection_id)),
        )
        .execute(conn)?;
        for entry in entries {
            insert_entry(conn, collection_id, entry)?;
        }
        next_version(conn, caller, collection_id)?;
        Ok(compendiums::table
            .filter(compendiums::id.eq(collection_id))
            .select(Compendium::as_select())
            .first(conn)?)
    })
}

/// Write one entry into a collection as a new row. The caller has checked the
/// collection is authored and holds no entry of this kind and name.
pub(crate) fn insert_entry(
    conn: &mut PgConnection,
    collection_id: Uuid,
    entry: VersionEntry,
) -> QueryResult<StoredEntry> {
    diesel::insert_into(compendium_entries::table)
        .values((
            compendium_entries::id.eq(Uuid::now_v7()),
            compendium_entries::compendium_id.eq(collection_id),
            compendium_entries::kind.eq(entry.kind),
            compendium_entries::name.eq(entry.name),
            compendium_entries::name_uncertain.eq(entry.name_uncertain),
            compendium_entries::page.eq(None::<i32>),
            compendium_entries::field_values.eq(entry.field_values),
            compendium_entries::prose_text.eq(entry.prose_text),
            compendium_entries::suspect.eq(entry.suspect),
            compendium_entries::extras.eq(entry.extras),
        ))
        .returning(StoredEntry::as_select())
        .get_result(conn)
}

fn refuse_unless_authored(
    conn: &mut PgConnection,
    collection_id: Uuid,
) -> Result<(), CollectionError> {
    let (origin, title): (ContentOrigin, String) = compendiums::table
        .filter(compendiums::id.eq(collection_id))
        .select((compendiums::origin, compendiums::book_title))
        .first(conn)?;
    if origin != ContentOrigin::Authored {
        return Err(CollectionError::ImportedBook { title });
    }
    Ok(())
}

fn entries_in_force(
    conn: &mut PgConnection,
    collection_id: Uuid,
) -> QueryResult<Vec<VersionEntry>> {
    Ok(compendium_entries::table
        .filter(compendium_entries::compendium_id.eq(collection_id))
        .order((
            compendium_entries::kind.asc(),
            compendium_entries::name.asc(),
        ))
        .select(StoredEntry::as_select())
        .load(conn)?
        .into_iter()
        .map(VersionEntry::from)
        .collect())
}

fn total_of(counts: &serde_json::Value) -> i32 {
    counts
        .as_object()
        .map(|counts| counts.values().filter_map(serde_json::Value::as_i64).sum())
        .and_then(|total: i64| i32::try_from(total).ok())
        .unwrap_or(0)
}

/// The column holds 300 characters; a world name can make the sentence
/// longer.
fn truncated(text: &str) -> String {
    text.chars().take(300).collect()
}

#[cfg(test)]
#[path = "versions_tests.rs"]
mod tests;
