//! Writing and reading a compendium and its entries (spec 049 T045).
//!
//! # Origin is written here and nowhere else
//!
//! [`import_book`] sets [`ContentOrigin::Uploaded`] itself. It is not a field
//! on [`NewBook`], so a caller cannot supply it, a resolver cannot forward it
//! from a client, and a future caller cannot forget to. FR-051 says a
//! compendium's origin is recorded automatically with no question asked; the
//! way to make that true is to leave no question anywhere in the signature.
//!
//! There is also **no update path to origin at all** (FR-057) — no
//! `AsChangeset` struct over `compendiums`, so no struct that can grow an
//! `origin` field in a later edit and start writing it. The updates this
//! module performs name their columns one by one. The database refuses the
//! change as well (a trigger, in the migration), so going around this module
//! does not work either. A value that can be flipped is a value that will be
//! flipped, and this is the value every sharing rule is enforced against.
//!
//! # Every read is gated
//!
//! Reads take the caller and go through
//! [`crate::auth::account_ownership::require_account_owner`], rather than
//! trusting the caller to have checked. A compendium is one person's shelf
//! and FR-055a says it must not become reachable by any other account,
//! operators included.
//!
//! # All or nothing
//!
//! An import is a single transaction (FR-032). A failure partway leaves no
//! compendium, no entries and no import record — which is also what makes
//! FR-034's abandonment free: there is nothing partial to clean up, because
//! nothing partial is ever committed.

use std::collections::BTreeMap;

use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::account_ownership::{AccountOwned, AccountOwnershipError, require_account_owner};
use crate::compendium::origin::ContentOrigin;
use crate::content::Entry;
use crate::schema::{compendium_entries, compendiums};

/// A compendium as it is stored: one book, on one account's shelf.
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = compendiums)]
pub struct Compendium {
    pub id: Uuid,
    pub owner_user_id: Uuid,
    pub book_title: String,
    /// SHA-256 of the file, hex, hashed in the browser before any upload
    /// (spec 047 FR-070). The file itself is not stored and never was.
    /// `None` for a collection, which was read from no file; the database
    /// ties the two together.
    pub source_hash: Option<String>,
    pub system_id: String,
    pub origin: ContentOrigin,
    pub parser_version: String,
    pub page_count: i32,
    /// How many of those pages yielded no text (FR-005). A book that was a
    /// third scans must be able to say so on the shelf, months later.
    pub silent_page_count: i32,
    /// Kind to count, as JSON, so the library lists a shelf without opening
    /// every entry. The entries are the truth; this is bookkeeping.
    pub entry_counts: serde_json::Value,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
    /// Which reading of the book is in force (050 FR-006). 1 for the first
    /// import; each re-import is the next version, never an edit of this one.
    pub base_version: i32,
}

/// One entry as it is stored.
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = compendium_entries)]
pub struct StoredEntry {
    pub id: Uuid,
    pub compendium_id: Uuid,
    pub kind: String,
    pub name: String,
    pub name_uncertain: bool,
    /// The page of the book it was read from. `None` for an entry written in
    /// a collection, which is on no page of any book.
    pub page: Option<i32>,
    /// The declared fields, each with its certainty. A field that was looked
    /// for and not found is stored with no value at all — see [`Entry`].
    pub field_values: serde_json::Value,
    pub prose_text: Option<String>,
    pub suspect: bool,
    pub extras: Option<serde_json::Value>,
    pub created_at: chrono::NaiveDateTime,
}

/// What the server needs to record a book, and nothing more.
///
/// Note what is **absent**: `origin`, which [`import_book`] writes itself
/// (FR-051), and `entry_counts`, which it counts from the entries it was
/// given rather than believing a number a caller supplied. Both are fields a
/// client could otherwise have set, and the first is the one every sharing
/// rule is enforced against.
#[derive(Debug, Clone)]
pub struct NewBook {
    pub book_title: String,
    pub source_hash: String,
    pub system_id: String,
    /// Which build of the reader produced this, so a later read can tell "the
    /// book changed" from "the reader improved".
    pub parser_version: String,
    pub page_count: i32,
    pub silent_page_count: i32,
}

/// Why an import did not happen.
#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("database error: {0}")]
    Database(#[from] diesel::result::Error),
    /// An entry's read values could not be encoded for storage. Kept as its
    /// own variant rather than folded into a database error because it means
    /// the reader produced something this build cannot represent, which is a
    /// defect to fix and not a transient failure to retry.
    #[error("an entry could not be stored as it was read: {0}")]
    Unstorable(String),
    /// Someone else's shelf. Carried through rather than flattened into a
    /// database error so a refusal still reads as a refusal at the call site.
    #[error("{0}")]
    NotYours(#[from] AccountOwnershipError),
}

#[derive(Insertable)]
#[diesel(table_name = compendium_entries)]
struct NewStoredEntry {
    id: Uuid,
    compendium_id: Uuid,
    kind: String,
    name: String,
    name_uncertain: bool,
    page: Option<i32>,
    field_values: serde_json::Value,
    prose_text: Option<String>,
    suspect: bool,
    extras: Option<serde_json::Value>,
}

/// Read a book onto an account's shelf: the compendium and every entry, in
/// one transaction.
///
/// `owner` is both the shelf and the author of the record — there is no
/// second `imported_by` beside `created_by`, because two fields meaning the
/// same thing are two fields that can disagree.
///
/// Refuses a second import of the same file by the same account, by the
/// unique constraint on (owner, hash) rather than by a check here: FR-047's
/// "you already have this book" is only answerable if the answer cannot be
/// two rows. Overwriting an existing one is [`replace_entries`].
pub fn import_book(
    conn: &mut PgConnection,
    owner: Uuid,
    book: NewBook,
    entries: &[Entry],
) -> Result<Compendium, ImportError> {
    let id = Uuid::now_v7();
    let rows = encode_entries(id, entries)?;
    let counts = count_by_kind(entries);

    conn.transaction(|conn| {
        let compendium = diesel::insert_into(compendiums::table)
            .values((
                compendiums::id.eq(id),
                compendiums::owner_user_id.eq(owner),
                compendiums::book_title.eq(&book.book_title),
                compendiums::source_hash.eq(&book.source_hash),
                compendiums::system_id.eq(&book.system_id),
                // FR-051: everything this spec produces is uploaded, with
                // nobody asked and no answer to get wrong.
                compendiums::origin.eq(ContentOrigin::Uploaded),
                compendiums::parser_version.eq(&book.parser_version),
                compendiums::page_count.eq(book.page_count),
                compendiums::silent_page_count.eq(book.silent_page_count),
                compendiums::entry_counts.eq(&counts),
                compendiums::created_by.eq(owner),
                compendiums::updated_by.eq(owner),
            ))
            .returning(Compendium::as_select())
            .get_result::<Compendium>(conn)?;

        insert_entries(conn, rows)?;
        Ok(compendium)
    })
}

/// Replace everything one compendium holds with a fresh read of the same book
/// (FR-047's overwrite).
///
/// The base changes; the shelf entry does not. Note what this does **not**
/// touch: `origin` and `owner_user_id`. Neither is nameable here, and the
/// database refuses the first of them outright.
///
/// One transaction, so a failed re-read leaves the previous contents intact
/// rather than an emptied bucket (FR-032) — and the previous version number
/// with them, so a version is never claimed by a reading that did not land.
///
/// A new version, not an edit (050 FR-006): every entry is a new row with a
/// new id, and `base_version` moves on by one. Worlds' deltas are not touched
/// here at all. They attach by kind and name (FR-025), so they survive the
/// replacement by construction (FR-026), and any that no longer find their
/// entry are reported by [`crate::library::deltas::unattached_after_reimport`]
/// rather than removed (FR-027).
pub fn replace_entries(
    conn: &mut PgConnection,
    caller: Uuid,
    compendium_id: Uuid,
    parser_version: &str,
    entries: &[Entry],
) -> Result<(), ImportError> {
    require_account_owner(conn, caller, AccountOwned::Compendium(compendium_id))?;

    let rows = encode_entries(compendium_id, entries)?;
    let counts = count_by_kind(entries);

    conn.transaction(|conn| {
        diesel::delete(
            compendium_entries::table.filter(compendium_entries::compendium_id.eq(compendium_id)),
        )
        .execute(conn)?;

        insert_entries(conn, rows)?;

        // Columns named one at a time, deliberately. There is no changeset
        // struct over this table for a later edit to add an `origin` field to.
        diesel::update(compendiums::table.filter(compendiums::id.eq(compendium_id)))
            .set((
                compendiums::entry_counts.eq(&counts),
                compendiums::parser_version.eq(parser_version),
                compendiums::base_version.eq(compendiums::base_version + 1),
                compendiums::updated_by.eq(caller),
                compendiums::updated_at.eq(diesel::dsl::now),
            ))
            .execute(conn)?;
        Ok(())
    })
}

/// Rename a book on the shelf — the one thing about a compendium a person may
/// change, because the title is what *they* call it.
pub fn rename(
    conn: &mut PgConnection,
    caller: Uuid,
    compendium_id: Uuid,
    book_title: &str,
) -> Result<(), AccountOwnershipError> {
    require_account_owner(conn, caller, AccountOwned::Compendium(compendium_id))?;

    diesel::update(compendiums::table.filter(compendiums::id.eq(compendium_id)))
        .set((
            compendiums::book_title.eq(book_title),
            compendiums::updated_by.eq(caller),
            compendiums::updated_at.eq(diesel::dsl::now),
        ))
        .execute(conn)?;
    Ok(())
}

/// One account's shelf, newest first (050 FR-002).
///
/// Scoped to one owner by construction. There is deliberately no listing that
/// spans accounts — FR-055a says an uploaded book must not become reachable
/// by any other account, and a query shape that spans owners is the first
/// step towards one that forgets to filter.
pub fn library_for(conn: &mut PgConnection, owner: Uuid) -> QueryResult<Vec<Compendium>> {
    compendiums::table
        .filter(compendiums::owner_user_id.eq(owner))
        .order(compendiums::created_at.desc())
        .select(Compendium::as_select())
        .load(conn)
}

/// One compendium, if it is the caller's.
pub fn load(
    conn: &mut PgConnection,
    caller: Uuid,
    compendium_id: Uuid,
) -> Result<Compendium, AccountOwnershipError> {
    require_account_owner(conn, caller, AccountOwned::Compendium(compendium_id))?;

    compendiums::table
        .filter(compendiums::id.eq(compendium_id))
        .select(Compendium::as_select())
        .first(conn)
        .map_err(Into::into)
}

/// Whether this account has already read this file in (FR-047).
///
/// Against **the account's library**, never against a world and never across
/// accounts: importing a book a second time for a second world is the
/// duplication this arc exists to prevent, and matching across accounts is
/// the cross-account store decision 1 rejected.
///
/// A near-identical file — the same work re-saved — hashes differently and is
/// correctly not a match (spec 047 FR-075). That is a property of SHA-256,
/// and it is the behaviour that is wanted: claiming a false match would
/// overwrite a book with a different one.
pub fn find_by_source_hash(
    conn: &mut PgConnection,
    owner: Uuid,
    source_hash: &str,
) -> QueryResult<Option<Compendium>> {
    compendiums::table
        .filter(compendiums::owner_user_id.eq(owner))
        .filter(compendiums::source_hash.eq(source_hash))
        .select(Compendium::as_select())
        .first(conn)
        .optional()
}

/// What came out of one book, optionally of one kind (FR-042).
pub fn entries_for(
    conn: &mut PgConnection,
    caller: Uuid,
    compendium_id: Uuid,
    kind: Option<&str>,
) -> Result<Vec<StoredEntry>, AccountOwnershipError> {
    require_account_owner(conn, caller, AccountOwned::Compendium(compendium_id))?;

    let mut query = compendium_entries::table
        .filter(compendium_entries::compendium_id.eq(compendium_id))
        .into_boxed();
    if let Some(kind) = kind {
        query = query.filter(compendium_entries::kind.eq(kind.to_string()));
    }

    query
        .order((
            compendium_entries::kind.asc(),
            compendium_entries::name.asc(),
        ))
        .select(StoredEntry::as_select())
        .load(conn)
        .map_err(Into::into)
}

/// Take a compendium off the shelf, and everything it contributed with it
/// (FR-044).
///
/// The entries go by the cascade on their foreign key, so removal takes that
/// import's contribution and nothing else — an entry from another book is not
/// reachable from here to delete by accident.
///
/// Naming what is in use **before** this runs (FR-045) is the caller's, and it
/// cannot be folded in: a removal that reports what would break and then does
/// it anyway is not a confirmation.
pub fn remove(
    conn: &mut PgConnection,
    caller: Uuid,
    compendium_id: Uuid,
) -> Result<(), AccountOwnershipError> {
    require_account_owner(conn, caller, AccountOwned::Compendium(compendium_id))?;

    diesel::delete(compendiums::table.filter(compendiums::id.eq(compendium_id))).execute(conn)?;
    Ok(())
}

/// How many of each kind this read produced (FR-041).
///
/// Counted from the entries actually being written, never taken from a
/// caller: a count that disagrees with the rows is a library that lies about
/// what a book holds, and FR-027 says what is committed must be what the
/// review showed.
fn count_by_kind(entries: &[Entry]) -> serde_json::Value {
    let mut counts: BTreeMap<&str, u64> = BTreeMap::new();
    for entry in entries {
        *counts.entry(entry.kind.as_str()).or_default() += 1;
    }
    serde_json::json!(counts)
}

/// Turn what the reader produced into rows, without deciding anything.
///
/// `values` is serialised as it stands, which is what carries the certainty
/// of each field into storage intact: a field the reader looked for and did
/// not find is `ReadValue::Unread`, which has nowhere to put a value, so no
/// serialisation of it can invent one (FR-002). No empty string, no zero, no
/// "not found" masquerading as a reading.
fn encode_entries(
    compendium_id: Uuid,
    entries: &[Entry],
) -> Result<Vec<NewStoredEntry>, ImportError> {
    entries
        .iter()
        .map(|entry| {
            let field_values = serde_json::to_value(&entry.values)
                .map_err(|e| ImportError::Unstorable(e.to_string()))?;
            let page = i32::try_from(entry.page).map_err(|_| {
                ImportError::Unstorable(format!("page {} is not a page number", entry.page))
            })?;
            Ok(NewStoredEntry {
                id: Uuid::now_v7(),
                compendium_id,
                kind: entry.kind.clone(),
                name: entry.name.clone(),
                name_uncertain: matches!(entry.name_state, crate::content::NameState::Uncertain),
                page: Some(page),
                field_values,
                prose_text: entry.text.clone(),
                suspect: entry.suspect,
                extras: entry.extras.clone(),
            })
        })
        .collect()
}

/// Write the rows, in batches.
///
/// Chunked because a book can produce a couple of thousand entries and
/// Postgres binds a limited number of parameters per statement; ten columns
/// times a whole Monster Manual is past it. Inside the caller's transaction
/// either way, so a batch that fails takes the whole import with it.
fn insert_entries(conn: &mut PgConnection, rows: Vec<NewStoredEntry>) -> QueryResult<()> {
    for chunk in rows.chunks(500) {
        diesel::insert_into(compendium_entries::table)
            .values(chunk)
            .execute(conn)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
