//! Collections on the shelf: authored content beside imported books (spec 050
//! FR-007 to FR-009c, spec 049 Phase 13).
//!
//! # A collection is a compendium whose origin is authored
//!
//! Same table, same id space, same entries table. FR-008 says a collection
//! behaves identically to a compendium everywhere origin does not decide the
//! answer, and the shelf, the book list, a world's read and a world's deltas
//! all take a compendium id — so one table makes "identically" true by
//! construction, where two would be two paths to keep in step.
//!
//! What origin **does** decide is decided here, and the database agrees:
//!
//! * a collection has no file hash and its entries no page (the migration
//!   ties both to origin, so neither can be invented or left out);
//! * only a collection is written to entry by entry — an imported book's base
//!   is what the book says, and changes only by being read again;
//! * only a collection downloads (FR-009a, FR-009b), and a download carries
//!   only authored content, naming anything it left out (FR-009c).
//!
//! # Every change is a new version
//!
//! Writing or removing an entry moves `base_version` on, the same counter a
//! re-import moves (FR-006). A world reading the collection meets a new
//! version exactly as it meets a re-read book: its deltas attach by kind and
//! name and anything that no longer does is reported, not dropped.

use std::collections::BTreeMap;

use diesel::prelude::*;
use serde::Serialize;
use uuid::Uuid;

use crate::auth::account_ownership::{AccountOwned, AccountOwnershipError, require_account_owner};
use crate::compendium::origin::ContentOrigin;
use crate::compendium::store::{Compendium, StoredEntry};
use crate::library::deltas::Content;
use crate::schema::{compendium_entries, compendiums};

/// Why a collection was not written or downloaded.
#[derive(Debug, thiserror::Error)]
pub enum CollectionError {
    #[error("{0}")]
    NotYours(#[from] AccountOwnershipError),
    /// An imported book's base is what the book says. It changes by being
    /// read again (FR-006), never an entry at a time.
    #[error(
        "\"{title}\" was read in from a book, so its entries are what the book says; read the book again to change them"
    )]
    ImportedBook { title: String },
    /// FR-009b, which is spec 049 FR-052 and not a new rule.
    #[error(
        "\"{title}\" was read in from a book, and a book read in stays with this account; it cannot be downloaded"
    )]
    NotDownloadable { title: String },
    /// Two entries sharing a kind and a name are the ambiguity FR-025a makes a
    /// world's changes refuse, so a collection does not manufacture one.
    #[error("this collection already has a {kind} named \"{name}\"")]
    AlreadyHas { kind: String, name: String },
    #[error("an entry needs a kind and a name")]
    Unnamed,
    #[error("a collection needs a title")]
    Untitled,
    #[error("this collection has no such entry")]
    NoSuchEntry,
    #[error("an entry could not be stored as written: {0}")]
    Unstorable(String),
    #[error("database error: {0}")]
    Database(String),
}

impl From<diesel::result::Error> for CollectionError {
    fn from(e: diesel::result::Error) -> Self {
        CollectionError::Database(e.to_string())
    }
}

/// Start a collection on this account's shelf (FR-007, FR-009).
///
/// `system_id` is taken as given; the resolver checks it against the
/// manifest, as an import's is.
pub fn create_collection(
    conn: &mut PgConnection,
    owner: Uuid,
    title: &str,
    system_id: &str,
) -> Result<Compendium, CollectionError> {
    let title = title.trim();
    if title.is_empty() {
        return Err(CollectionError::Untitled);
    }
    Ok(diesel::insert_into(compendiums::table)
        .values((
            compendiums::id.eq(Uuid::now_v7()),
            compendiums::owner_user_id.eq(owner),
            compendiums::book_title.eq(title),
            compendiums::source_hash.eq(None::<String>),
            compendiums::system_id.eq(system_id),
            // Written here, for the reason `import_book` writes Uploaded: no
            // caller names origin, so no caller can name the wrong one.
            compendiums::origin.eq(ContentOrigin::Authored),
            compendiums::parser_version.eq(""),
            compendiums::page_count.eq(0),
            compendiums::silent_page_count.eq(0),
            compendiums::created_by.eq(owner),
            compendiums::updated_by.eq(owner),
        ))
        .returning(Compendium::as_select())
        .get_result(conn)?)
}

/// Write an entry into a collection (FR-007).
///
/// Refused for an imported book, for a kind and name the collection already
/// holds, and for an entry with no kind or name. The collection moves on a
/// version.
pub fn write_entry(
    conn: &mut PgConnection,
    caller: Uuid,
    collection_id: Uuid,
    kind: &str,
    name: &str,
    content: Content,
) -> Result<StoredEntry, CollectionError> {
    let collection = writable(conn, caller, collection_id)?;
    let (kind, name) = (kind.trim(), name.trim());
    if kind.is_empty() || name.is_empty() {
        return Err(CollectionError::Unnamed);
    }
    let (field_values, prose_text) = match content {
        Content::Fields(fields) => (
            serde_json::to_value(&fields)
                .map_err(|e| CollectionError::Unstorable(e.to_string()))?,
            None,
        ),
        Content::Prose(text) => (serde_json::json!({}), Some(text)),
    };

    conn.transaction(|conn| {
        let held: i64 = compendium_entries::table
            .filter(compendium_entries::compendium_id.eq(collection.id))
            .filter(compendium_entries::kind.eq(kind))
            .filter(compendium_entries::name.eq(name))
            .count()
            .get_result(conn)?;
        if held > 0 {
            return Err(CollectionError::AlreadyHas {
                kind: kind.to_string(),
                name: name.to_string(),
            });
        }

        let entry = diesel::insert_into(compendium_entries::table)
            .values((
                compendium_entries::id.eq(Uuid::now_v7()),
                compendium_entries::compendium_id.eq(collection.id),
                compendium_entries::kind.eq(kind),
                compendium_entries::name.eq(name),
                compendium_entries::page.eq(None::<i32>),
                compendium_entries::field_values.eq(field_values),
                compendium_entries::prose_text.eq(prose_text),
            ))
            .returning(StoredEntry::as_select())
            .get_result(conn)?;
        next_version(conn, caller, collection.id)?;
        Ok(entry)
    })
}

/// Take an entry out of a collection. The collection moves on a version.
pub fn remove_entry(
    conn: &mut PgConnection,
    caller: Uuid,
    collection_id: Uuid,
    entry_id: Uuid,
) -> Result<(), CollectionError> {
    let collection = writable(conn, caller, collection_id)?;
    conn.transaction(|conn| {
        let removed = diesel::delete(
            compendium_entries::table
                .filter(compendium_entries::id.eq(entry_id))
                .filter(compendium_entries::compendium_id.eq(collection.id)),
        )
        .execute(conn)?;
        if removed == 0 {
            return Err(CollectionError::NoSuchEntry);
        }
        next_version(conn, caller, collection.id)
    })
}

/// A collection as a file its owner can take away (FR-009a).
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CollectionFile {
    /// What this file is, so something reading it later can tell.
    pub format: &'static str,
    pub format_version: u32,
    pub title: String,
    pub system_id: String,
    /// Which version of the collection this is a copy of.
    pub version: i32,
    pub entries: Vec<FileEntry>,
    /// FR-009c: anything left out because it was not authored, and why.
    /// Present even when empty, so "nothing was left out" is said rather than
    /// inferred from a missing key.
    pub excluded: Vec<ExcludedEntry>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub kind: String,
    pub name: String,
    pub field_values: serde_json::Value,
    pub prose_text: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExcludedEntry {
    pub kind: String,
    pub name: String,
    pub reason: String,
}

/// The name this format goes by inside the file.
pub const FILE_FORMAT: &str = "thunderforge.collection";

/// What the owner's download of `collection_id` holds (FR-009a to FR-009c).
///
/// Refuses an imported book outright (FR-009b) — before any entry is read, so
/// a refused download reads nothing out of the book.
pub fn download(
    conn: &mut PgConnection,
    caller: Uuid,
    collection_id: Uuid,
) -> Result<CollectionFile, CollectionError> {
    require_account_owner(conn, caller, AccountOwned::Compendium(collection_id))?;
    let collection: Compendium = compendiums::table
        .filter(compendiums::id.eq(collection_id))
        .select(Compendium::as_select())
        .first(conn)?;
    if !collection.origin.may_be_shared() {
        return Err(CollectionError::NotDownloadable {
            title: collection.book_title,
        });
    }

    // Each entry with its own origin, asked of the database rather than
    // assumed from the collection's: the rule FR-009c states is per entry, and
    // it is enforced per entry so it still holds on the day an entry's origin
    // can differ from its collection's.
    let entries: Vec<(StoredEntry, ContentOrigin)> = compendium_entries::table
        .inner_join(compendiums::table)
        .filter(compendium_entries::compendium_id.eq(collection_id))
        .order((
            compendium_entries::kind.asc(),
            compendium_entries::name.asc(),
        ))
        .select((StoredEntry::as_select(), compendiums::origin))
        .load(conn)?;

    Ok(compose_file(&collection, entries))
}

/// Lay a collection and its entries out as a file, keeping only what may leave
/// the account (FR-009c). Pure, so the exclusion is testable for an entry the
/// database would not let exist today.
pub fn compose_file(
    collection: &Compendium,
    entries: Vec<(StoredEntry, ContentOrigin)>,
) -> CollectionFile {
    let (mut kept, mut excluded) = (Vec::new(), Vec::new());
    for (entry, origin) in entries {
        if origin.may_be_shared() {
            kept.push(FileEntry {
                kind: entry.kind,
                name: entry.name,
                field_values: entry.field_values,
                prose_text: entry.prose_text,
            });
        } else {
            excluded.push(ExcludedEntry {
                kind: entry.kind,
                name: entry.name,
                reason: "It came from a book read in, and a book read in stays with this account."
                    .to_string(),
            });
        }
    }
    CollectionFile {
        format: FILE_FORMAT,
        format_version: 1,
        title: collection.book_title.clone(),
        system_id: collection.system_id.clone(),
        version: collection.base_version,
        entries: kept,
        excluded,
    }
}

/// The caller's collection, refused if it is an imported book.
fn writable(
    conn: &mut PgConnection,
    caller: Uuid,
    collection_id: Uuid,
) -> Result<Compendium, CollectionError> {
    require_account_owner(conn, caller, AccountOwned::Compendium(collection_id))?;
    let collection: Compendium = compendiums::table
        .filter(compendiums::id.eq(collection_id))
        .select(Compendium::as_select())
        .first(conn)?;
    if collection.origin != ContentOrigin::Authored {
        return Err(CollectionError::ImportedBook {
            title: collection.book_title,
        });
    }
    Ok(collection)
}

/// Recount the collection's entries and move it on a version.
fn next_version(
    conn: &mut PgConnection,
    caller: Uuid,
    collection_id: Uuid,
) -> Result<(), CollectionError> {
    let kinds: Vec<String> = compendium_entries::table
        .filter(compendium_entries::compendium_id.eq(collection_id))
        .select(compendium_entries::kind)
        .load(conn)?;
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    for kind in kinds {
        *counts.entry(kind).or_default() += 1;
    }
    diesel::update(compendiums::table.filter(compendiums::id.eq(collection_id)))
        .set((
            compendiums::entry_counts.eq(serde_json::json!(counts)),
            compendiums::base_version.eq(compendiums::base_version + 1),
            compendiums::updated_by.eq(caller),
            compendiums::updated_at.eq(diesel::dsl::now),
        ))
        .execute(conn)?;
    Ok(())
}

#[cfg(test)]
#[path = "collections_tests.rs"]
mod tests;
