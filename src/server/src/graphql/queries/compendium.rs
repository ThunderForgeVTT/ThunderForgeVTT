//! Reading a library: `myLibrary`, `compendium`, `compendiumEntries`
//! (spec 049 T046, US3; spec 050 FR-002, `contracts/import.md` — "Reading a
//! library").
//!
//! # Every read is account-scoped, and there is no world-scoped one
//!
//! A compendium belongs to an account and never to a world (049 FR-040), so
//! nothing here takes a `world_id` and nothing here is reachable through world
//! membership. A world cannot see a compendium at all until the book list
//! spec 050 FR-010 describes exists; research §1 records why that is a later
//! phase rather than an omission here. Adding a `worldId` argument to any of
//! these would be the retrofit that decision exists to avoid.
//!
//! The gate is [`crate::compendium::store`]'s, not this module's: every
//! resolver below calls a store function that takes the caller and goes
//! through `require_account_owner` itself. There is deliberately no query in
//! this file that reads `compendiums` directly, because a resolver that can
//! assemble its own SELECT is a resolver that can forget the filter.
//!
//! # Somebody else's shelf answers "no such thing"
//!
//! [`CompendiumQuery::compendium`] returns `null` for a compendium that does
//! not exist **and** for one belonging to another account, which is the
//! no-enumeration property `require_account_owner` is written for carried up
//! to the wire: a caller able to tell the two apart can walk an id space and
//! learn what other people have on their shelves.

use async_graphql::{Context, Enum, Json, Object, SimpleObject};
use base64::Engine as _;
use uuid::Uuid;

use crate::auth::account_ownership::AccountOwnershipError;
use crate::compendium::ContentOrigin;
use crate::compendium::store::{self, Compendium, StoredEntry};
use crate::graphql::{Error, GraphQLResult, app_state, authenticated_user};

/// How many entries one page carries when the caller does not say, and the
/// most it may ask for.
///
/// Bounded because a book produces thousands of entries and a single
/// unbounded read of a Monster Manual is megabytes of JSON for a screen that
/// shows twenty rows. The cap is on what crosses the wire, and the client
/// cannot raise it.
const ENTRIES_PER_PAGE: i32 = 50;
const MAX_ENTRIES_PER_PAGE: i32 = 200;

/// Where a compendium came from, on the wire.
///
/// Mirrors [`ContentOrigin`] rather than re-deriving it, and is read-only
/// here: origin is written by the server on import (FR-051) and has no update
/// path anywhere (FR-057). It is on the shelf because FR-002 asks the library
/// to show provenance, and because it is the field every later refusal will
/// name as its reason.
#[derive(Enum, Copy, Clone, Debug, PartialEq, Eq)]
#[graphql(name = "ContentOrigin")]
pub enum GraphQLContentOrigin {
    Authored,
    Uploaded,
}

impl From<ContentOrigin> for GraphQLContentOrigin {
    fn from(origin: ContentOrigin) -> Self {
        match origin {
            ContentOrigin::Authored => GraphQLContentOrigin::Authored,
            ContentOrigin::Uploaded => GraphQLContentOrigin::Uploaded,
        }
    }
}

/// How many of one kind a book produced.
///
/// A list of pairs rather than the stored JSON object, so the shelf renders
/// counts without a client parsing an untyped blob and inventing its own
/// ordering. The stored object is bookkeeping; the entries remain the truth.
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "CompendiumKindCount")]
pub struct GraphQLKindCount {
    pub kind: String,
    pub count: i32,
}

/// One book on an account's shelf (049 FR-041, 050 FR-002).
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "Compendium")]
pub struct GraphQLCompendium {
    pub id: Uuid,
    pub book_title: String,
    /// The file's SHA-256. Readable only by the account that holds it, and
    /// what lets a client recognise a file it already has before reading a
    /// second copy of it (FR-047). A hash is not content.
    pub source_hash: String,
    pub system_id: String,
    pub origin: GraphQLContentOrigin,
    pub parser_version: String,
    pub page_count: i32,
    /// Pages that yielded no text (FR-005). A book that was a third scans
    /// says so on the shelf, months later.
    pub silent_page_count: i32,
    pub entry_counts: Vec<GraphQLKindCount>,
    /// Everything the counts add up to, so a shelf need not sum them itself.
    pub entry_total: i32,
    pub imported_at: String,
    pub updated_at: String,
}

impl From<Compendium> for GraphQLCompendium {
    fn from(row: Compendium) -> Self {
        let entry_counts = kind_counts(&row.entry_counts);
        let entry_total = entry_counts.iter().map(|count| count.count).sum();
        Self {
            id: row.id,
            book_title: row.book_title,
            source_hash: row.source_hash,
            system_id: row.system_id,
            origin: row.origin.into(),
            parser_version: row.parser_version,
            page_count: row.page_count,
            silent_page_count: row.silent_page_count,
            entry_counts,
            entry_total,
            imported_at: row.created_at.and_utc().to_rfc3339(),
            updated_at: row.updated_at.and_utc().to_rfc3339(),
        }
    }
}

/// One entry, with the book it came from and the page it was found on
/// (FR-043).
///
/// `compendiumId` and `bookTitle` both travel with every entry deliberately:
/// FR-043 is a promise to the person reading an entry, and an entry that can
/// only say which book it came from by being looked up somewhere else is an
/// entry that will be shown without saying it.
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "CompendiumEntry")]
pub struct GraphQLCompendiumEntry {
    pub id: Uuid,
    pub compendium_id: Uuid,
    pub book_title: String,
    pub kind: String,
    pub name: String,
    /// The reader was unsure it had the name right. Shown, not hidden.
    pub name_uncertain: bool,
    pub page: i32,
    /// Each declared field with the certainty it was read at. A field looked
    /// for and not found carries no value at all, which is why this crosses
    /// as it was stored rather than being flattened into strings.
    pub field_values: Json<serde_json::Value>,
    pub prose_text: Option<String>,
    /// The reader distrusted this one (FR-023's third state, kept after the
    /// review so a suspect entry stays visibly suspect on the shelf).
    pub suspect: bool,
}

/// One page of entries, and where the next one starts.
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "CompendiumEntryPage")]
pub struct GraphQLCompendiumEntryPage {
    pub entries: Vec<GraphQLCompendiumEntry>,
    /// `null` when this page is the last one. A client that keeps asking
    /// until this is null cannot loop, because the cursor is the id of the
    /// last entry handed out and the order it indexes is total.
    pub next_cursor: Option<String>,
    /// How many entries match the filter in total, so a shelf can say "20 of
    /// 1,431" without fetching 1,431 of them.
    pub total: i32,
}

#[derive(Default)]
pub struct CompendiumQuery;

#[Object]
impl CompendiumQuery {
    /// This account's shelf, newest first (050 FR-002).
    ///
    /// There is no argument that could widen it to another account, and the
    /// store offers no listing that spans owners, so "whose library?" is
    /// answered by who is calling and by nothing else.
    async fn my_library(&self, ctx: &Context<'_>) -> GraphQLResult<Vec<GraphQLCompendium>> {
        let state = app_state(ctx)?;
        let owner = authenticated_user(ctx)?.user_id;
        let mut conn = connection(state)?;

        let rows = tokio::task::spawn_blocking(move || store::library_for(&mut conn, owner))
            .await
            .map_err(|_| Error::new("Failed to read your library"))?
            .map_err(|_| Error::new("Failed to read your library"))?;

        Ok(rows.into_iter().map(GraphQLCompendium::from).collect())
    }

    /// One book, if it is the caller's — and `null` if it is not, whether
    /// because there is no such compendium or because it is somebody else's.
    async fn compendium(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
    ) -> GraphQLResult<Option<GraphQLCompendium>> {
        let state = app_state(ctx)?;
        let caller = authenticated_user(ctx)?.user_id;
        let mut conn = connection(state)?;

        let found = tokio::task::spawn_blocking(move || store::load(&mut conn, caller, id))
            .await
            .map_err(|_| Error::new("Failed to read that compendium"))?;

        match found {
            Ok(row) => Ok(Some(GraphQLCompendium::from(row))),
            Err(AccountOwnershipError::NotTheOwner) => Ok(None),
            Err(AccountOwnershipError::Database(_)) => {
                Err(Error::new("Failed to read that compendium"))
            }
        }
    }

    /// What came out of one book, a page at a time, optionally of one kind
    /// (FR-042).
    ///
    /// `after` is the `nextCursor` of the previous page and nothing else; a
    /// cursor naming an entry that is no longer there — a re-import replaced
    /// the base underneath a browsing session — is refused rather than
    /// silently answered from the beginning, because a page of somebody's
    /// place in a book quietly becoming page one reads as lost data.
    async fn compendium_entries(
        &self,
        ctx: &Context<'_>,
        compendium_id: Uuid,
        kind: Option<String>,
        after: Option<String>,
        first: Option<i32>,
    ) -> GraphQLResult<GraphQLCompendiumEntryPage> {
        let state = app_state(ctx)?;
        let caller = authenticated_user(ctx)?.user_id;
        let limit = first
            .unwrap_or(ENTRIES_PER_PAGE)
            .clamp(1, MAX_ENTRIES_PER_PAGE) as usize;
        let after = after.map(|cursor| decode_cursor(&cursor)).transpose()?;
        let mut conn = connection(state)?;

        // The book and its entries are read under one ownership check each,
        // in one blocking hop: the title travels with every entry (FR-043),
        // so fetching it separately would be a second round trip for a field
        // that is never optional.
        let read = tokio::task::spawn_blocking(move || {
            let book = store::load(&mut conn, caller, compendium_id)?;
            let entries = store::entries_for(&mut conn, caller, compendium_id, kind.as_deref())?;
            Ok::<_, AccountOwnershipError>((book, entries))
        })
        .await
        .map_err(|_| Error::new("Failed to read that compendium"))?;

        let (book, mut entries) = match read {
            Ok(read) => read,
            // Somebody else's book reads as an empty one, for the reason
            // `compendium` returns null: the two must be indistinguishable.
            Err(AccountOwnershipError::NotTheOwner) => {
                return Ok(GraphQLCompendiumEntryPage {
                    entries: Vec::new(),
                    next_cursor: None,
                    total: 0,
                });
            }
            Err(AccountOwnershipError::Database(_)) => {
                return Err(Error::new("Failed to read that compendium"));
            }
        };

        // The store orders by kind then name, which two entries of the same
        // name in the same book do not distinguish. Paging over an order with
        // ties can repeat one entry and skip another, so the id breaks them:
        // a total order is what makes a cursor mean the same thing twice.
        entries.sort_by(|left, right| {
            (&left.kind, &left.name, left.id).cmp(&(&right.kind, &right.name, right.id))
        });

        let total = i32::try_from(entries.len()).unwrap_or(i32::MAX);
        let start = match after {
            None => 0,
            Some(cursor) => entries
                .iter()
                .position(|entry| entry.id == cursor)
                .map(|at| at + 1)
                .ok_or_else(|| {
                    Error::new("That page of this book is no longer there. Open it again.")
                })?,
        };

        let page: Vec<GraphQLCompendiumEntry> = entries
            .iter()
            .skip(start)
            .take(limit)
            .map(|entry| to_graphql_entry(entry, &book.book_title))
            .collect();

        let next_cursor = (start + page.len() < entries.len())
            .then(|| page.last().map(|entry| encode_cursor(entry.id)))
            .flatten();

        Ok(GraphQLCompendiumEntryPage {
            entries: page,
            next_cursor,
            total,
        })
    }
}

fn connection(
    state: &crate::state::AppState,
) -> GraphQLResult<
    diesel::r2d2::PooledConnection<diesel::r2d2::ConnectionManager<diesel::PgConnection>>,
> {
    state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))
}

fn to_graphql_entry(entry: &StoredEntry, book_title: &str) -> GraphQLCompendiumEntry {
    GraphQLCompendiumEntry {
        id: entry.id,
        compendium_id: entry.compendium_id,
        book_title: book_title.to_string(),
        kind: entry.kind.clone(),
        name: entry.name.clone(),
        name_uncertain: entry.name_uncertain,
        page: entry.page,
        field_values: Json(entry.field_values.clone()),
        prose_text: entry.prose_text.clone(),
        suspect: entry.suspect,
    }
}

/// The stored kind-to-count object, as a list a client can render.
///
/// A malformed or missing object reads as no counts rather than as an error:
/// the counts are bookkeeping written from the entries, and a shelf that
/// refuses to draw because one book's tally is unreadable would hide seven
/// books that are fine.
fn kind_counts(stored: &serde_json::Value) -> Vec<GraphQLKindCount> {
    let Some(object) = stored.as_object() else {
        return Vec::new();
    };
    let mut counts: Vec<GraphQLKindCount> = object
        .iter()
        .filter_map(|(kind, count)| {
            Some(GraphQLKindCount {
                kind: kind.clone(),
                count: i32::try_from(count.as_u64()?).ok()?,
            })
        })
        .collect();
    counts.sort_by(|left, right| left.kind.cmp(&right.kind));
    counts
}

/// Cursors are opaque on the wire so that what they encode stays this
/// module's business — a client that learned to build one would be a client
/// that breaks when the order changes.
fn encode_cursor(id: Uuid) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(id.as_bytes())
}

fn decode_cursor(cursor: &str) -> GraphQLResult<Uuid> {
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(cursor)
        .map_err(|_| Error::new("That is not a page of this book."))?;
    Uuid::from_slice(&bytes).map_err(|_| Error::new("That is not a page of this book."))
}

#[cfg(test)]
#[path = "compendium_tests.rs"]
mod compendium_tests;
