//! Which books a world is running: switching one on, switching one off, and
//! reading what the list is serving (spec 050 FR-010 to FR-015, FR-030 to
//! FR-036, FR-041, FR-042).
//!
//! # What is not here
//!
//! A copy. [`switch_on`] writes one row naming a book; it does not read the
//! book, does not duplicate an entry, and has no path that could. Everything
//! a world shows from a compendium is read from `compendium_entries` at the
//! moment it is asked for, through [`entries_served_by`]. That is the whole
//! of FR-011 and FR-031, and it is also why [`switch_off`] takes content out
//! from under a live table — a consequence the caller must pay visibly
//! (FR-032, [`switch_off_report`]), never one a player discovers when a sword
//! vanishes.

use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::account_ownership::{AccountOwned, AccountOwnershipError, require_account_owner};
use crate::compendium::store::{self, Compendium, StoredEntry};
use crate::schema::{compendium_entries, compendiums, world_books, worlds};

/// One row of the list, as it is stored.
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = world_books)]
pub struct BookOnList {
    pub id: Uuid,
    pub world_id: Uuid,
    pub compendium_id: Uuid,
    /// The base that was in force when this was switched on (FR-015): the
    /// file's hash and the build of the reader that read it, which is the
    /// pair that changes when a re-import replaces a base.
    pub base_source_hash: String,
    pub base_parser_version: String,
    pub switched_on_by: Uuid,
    pub switched_on_at: chrono::NaiveDateTime,
}

/// One row of the list as a person reads it (FR-030, FR-036).
///
/// The book's **name** and the counts that say how big it is — and nothing
/// that amounts to its content. Naming a book is not reproducing it, and this
/// struct is what a player is shown, so the line matters: there is no entry,
/// no prose and no field value anywhere in it.
#[derive(Debug, Clone)]
pub struct ListedBook {
    pub row: BookOnList,
    pub book_title: String,
    /// The system the book was read as, which is not always the system the
    /// world runs any more — see `system_matches`.
    pub system_id: String,
    pub entry_counts: serde_json::Value,
    /// Whether this world still runs the system the book was read as
    /// (FR-042).
    ///
    /// `false` is reachable only one way: the world changed system after the
    /// book was switched on. A mismatch cannot be switched on in the first
    /// place — the database refuses it — so this flag always means "your
    /// world moved", which is what makes it worth reporting rather than
    /// silently dropping the row. A mismatched book is listed and **not
    /// served** (FR-042's second half, which [`entries_served_by`] enforces).
    pub system_matches: bool,
}

/// Why a book did not go on or come off a list.
///
/// Every variant is a refusal. They are separate so a server log can tell a
/// wrong caller from a broken database and so a person can be told which of
/// their assumptions was wrong — not so a call site can decide one of them is
/// survivable.
#[derive(Debug, thiserror::Error)]
pub enum BookListError {
    /// The caller does not own this world.
    ///
    /// Co-running a world is not owning it: a co-Game Master uses what is
    /// switched on and does not change the list, because the list draws on
    /// the **owner's** shelf and a co-Game Master cannot see it. Being able
    /// to use a book is not being able to take it home (FR-014).
    #[error("this world is not yours to change the book list of")]
    NotYourWorld,
    /// Somebody else's book. Carried through rather than flattened so a
    /// refusal still reads as a refusal at the call site.
    #[error("{0}")]
    NotYourBook(#[from] AccountOwnershipError),
    /// FR-041: a book read as one system cannot be switched on in a world
    /// running another. Both are named because a person seeing this needs to
    /// know which end is wrong.
    #[error("this book was read as {book_system} and this world runs {}", world_system.as_deref().unwrap_or("no system"))]
    SystemMismatch {
        book_system: String,
        world_system: Option<String>,
    },
    /// The book is not on this world's list at all.
    #[error("that book is not switched on for this world")]
    NotOnTheList,
    #[error("database error: {0}")]
    Database(String),
}

impl From<diesel::result::Error> for BookListError {
    fn from(e: diesel::result::Error) -> Self {
        BookListError::Database(e.to_string())
    }
}

/// Which account owns this world, fail-closed.
///
/// `worlds.created_by` is this codebase's ownership source of truth, as
/// `require_world_member` records at length. Deliberately **no admin
/// bypass**, and no parameter to add one with: switching a book on reaches
/// into an account's library, an operator is another account, and
/// `require_account_owner` refuses one for exactly that reason. An operator
/// acting against imported content has the moderation route, which is a
/// different act from putting somebody's book on somebody's table.
fn require_world_owner(
    conn: &mut PgConnection,
    caller: Uuid,
    world_id: Uuid,
) -> Result<(), BookListError> {
    let owner = worlds::table
        .filter(worlds::id.eq(world_id))
        .select(worlds::created_by)
        .first::<Uuid>(conn)
        .optional()?;

    match owner {
        Some(owner) if owner == caller => Ok(()),
        // Somebody else's world, or no such world. The same refusal, for the
        // reason `require_account_owner` gives: a caller who can tell the two
        // apart can walk an id space.
        _ => Err(BookListError::NotYourWorld),
    }
}

/// What system this world runs, if it has said (FR-041).
fn system_of_world(
    conn: &mut PgConnection,
    world_id: Uuid,
) -> Result<Option<String>, BookListError> {
    worlds::table
        .filter(worlds::id.eq(world_id))
        .select(worlds::game_system_id)
        .first::<Option<String>>(conn)
        .optional()
        .map(Option::flatten)
        .map_err(Into::into)
}

/// Switch a book on for a world (FR-010, FR-031).
///
/// **This is the only way content reaches a world from a library**, and it is
/// the same call whether a Game Master ticks a book while creating the world
/// or a fortnight later (FR-033). There is no second path to keep agreeing
/// with this one, which is the entire reason the spec asked for a list rather
/// than a seeding step.
///
/// Switching on a book that is already on succeeds and changes nothing. That
/// is deliberate: the caller asked for a state, the state holds, and a
/// creation form that ticks three books should not fail because one of them
/// was ticked a moment ago by a retried request.
pub fn switch_on(
    conn: &mut PgConnection,
    caller: Uuid,
    world_id: Uuid,
    compendium_id: Uuid,
) -> Result<BookOnList, BookListError> {
    require_world_owner(conn, caller, world_id)?;
    // The account gate, through the one helper rather than by comparing the
    // owner column here. The lookup is the part that gets written differently
    // at each site, not the `==`.
    require_account_owner(conn, caller, AccountOwned::Compendium(compendium_id))?;

    let book: Compendium = store::load(conn, caller, compendium_id)?;
    let world_system = system_of_world(conn, world_id)?;
    if world_system.as_deref() != Some(book.system_id.as_str()) {
        return Err(BookListError::SystemMismatch {
            book_system: book.system_id,
            world_system,
        });
    }

    diesel::insert_into(world_books::table)
        .values((
            world_books::id.eq(Uuid::now_v7()),
            world_books::world_id.eq(world_id),
            world_books::compendium_id.eq(compendium_id),
            // FR-015: the base in force, recorded rather than joined, so the
            // answer survives the base being replaced underneath it.
            world_books::base_source_hash.eq(&book.source_hash),
            world_books::base_parser_version.eq(&book.parser_version),
            world_books::switched_on_by.eq(caller),
        ))
        .on_conflict((world_books::world_id, world_books::compendium_id))
        .do_nothing()
        .execute(conn)?;

    world_books::table
        .filter(world_books::world_id.eq(world_id))
        .filter(world_books::compendium_id.eq(compendium_id))
        .select(BookOnList::as_select())
        .first(conn)
        .map_err(Into::into)
}

/// What is on this world's list (FR-030, FR-035).
///
/// Not gated here: **every member of the world may read this**, players
/// included, because knowing what the table is running is the point of the
/// list. The caller checks membership; what it must not do is hand this to a
/// stranger. Nothing in [`ListedBook`] is content, so the read-only half of
/// FR-035 is the absence of a mutation a player can call rather than a
/// narrower version of this function.
pub fn books_on(conn: &mut PgConnection, world_id: Uuid) -> Result<Vec<ListedBook>, BookListError> {
    let world_system = system_of_world(conn, world_id)?;

    let rows: Vec<(BookOnList, String, String, serde_json::Value)> = world_books::table
        .inner_join(compendiums::table.on(compendiums::id.eq(world_books::compendium_id)))
        .filter(world_books::world_id.eq(world_id))
        .order(compendiums::book_title.asc())
        .select((
            BookOnList::as_select(),
            compendiums::book_title,
            compendiums::system_id,
            compendiums::entry_counts,
        ))
        .load(conn)?;

    Ok(rows
        .into_iter()
        .map(|(row, book_title, system_id, entry_counts)| ListedBook {
            system_matches: world_system.as_deref() == Some(system_id.as_str()),
            row,
            book_title,
            system_id,
            entry_counts,
        })
        .collect())
}

/// What this world's owner could switch on and has not (FR-030, FR-041).
///
/// Their own shelf, narrowed to the system this world runs and to books that
/// are not already on. A world with no system chosen is offered nothing,
/// because a book read as one system says nothing intelligible to a world
/// that has not said what it is.
pub fn offerable_to(
    conn: &mut PgConnection,
    caller: Uuid,
    world_id: Uuid,
) -> Result<Vec<Compendium>, BookListError> {
    require_world_owner(conn, caller, world_id)?;

    let Some(world_system) = system_of_world(conn, world_id)? else {
        return Ok(Vec::new());
    };

    let already_on: Vec<Uuid> = world_books::table
        .filter(world_books::world_id.eq(world_id))
        .select(world_books::compendium_id)
        .load(conn)?;

    Ok(store::library_for(conn, caller)?
        .into_iter()
        .filter(|book| book.system_id == world_system)
        .filter(|book| !already_on.contains(&book.id))
        .collect())
}

/// What switching a book off would take out of this world, named before
/// anything is taken (FR-013, FR-032).
///
/// The honest shape of "what is in use" under a model where nothing was
/// copied: **everything the book is serving**. A world holds no copy of an
/// entry to enumerate, so what a Game Master loses is the book's whole
/// contribution to this table, and that is what this counts and names.
///
/// `deltas` is empty, and empty as a fact rather than as a gap: a world's
/// changes over a base are spec 050's delta model, which is a later phase and
/// has no table yet. When it arrives this is the function that learns it, and
/// both callers already go through here.
#[derive(Debug, Clone)]
pub struct SwitchOffReport {
    pub compendium_id: Uuid,
    pub book_title: String,
    /// How many entries stop being served.
    pub entry_count: i64,
    /// Some of them by name, so a person recognises what they are turning
    /// off. Capped: a Monster Manual is not a confirmation dialogue.
    pub entry_names: Vec<String>,
    /// Changes this world made over the book, which go with it. None can
    /// exist yet; see above.
    pub deltas: Vec<String>,
}

/// How many entry names a confirmation may carry.
const NAMES_IN_A_REPORT: i64 = 12;

/// Say what switching this book off takes, and change nothing.
pub fn switch_off_report(
    conn: &mut PgConnection,
    caller: Uuid,
    world_id: Uuid,
    compendium_id: Uuid,
) -> Result<SwitchOffReport, BookListError> {
    require_world_owner(conn, caller, world_id)?;

    let on_the_list = world_books::table
        .filter(world_books::world_id.eq(world_id))
        .filter(world_books::compendium_id.eq(compendium_id))
        .select(world_books::id)
        .first::<Uuid>(conn)
        .optional()?;
    if on_the_list.is_none() {
        return Err(BookListError::NotOnTheList);
    }

    let book_title = compendiums::table
        .filter(compendiums::id.eq(compendium_id))
        .select(compendiums::book_title)
        .first::<String>(conn)?;

    let entry_count = compendium_entries::table
        .filter(compendium_entries::compendium_id.eq(compendium_id))
        .count()
        .get_result::<i64>(conn)?;

    let entry_names = compendium_entries::table
        .filter(compendium_entries::compendium_id.eq(compendium_id))
        .order(compendium_entries::name.asc())
        .limit(NAMES_IN_A_REPORT)
        .select(compendium_entries::name)
        .load::<String>(conn)?;

    Ok(SwitchOffReport {
        compendium_id,
        book_title,
        entry_count,
        entry_names,
        deltas: Vec::new(),
    })
}

/// Switch a book off (FR-013, FR-032).
///
/// One row goes and nothing else does. There is no copy in the world to
/// delete afterwards, which is the claim this whole architecture rests on and
/// the reason the end-to-end test looks for the absence rather than trusting
/// it: what a world showed came from the shelf every time it showed it, so
/// removing the link removes the content.
pub fn switch_off(
    conn: &mut PgConnection,
    caller: Uuid,
    world_id: Uuid,
    compendium_id: Uuid,
) -> Result<(), BookListError> {
    require_world_owner(conn, caller, world_id)?;

    let removed = diesel::delete(
        world_books::table
            .filter(world_books::world_id.eq(world_id))
            .filter(world_books::compendium_id.eq(compendium_id)),
    )
    .execute(conn)?;

    if removed == 0 {
        return Err(BookListError::NotOnTheList);
    }
    Ok(())
}

/// The fetch (FR-031): what one switched-on book is serving this world right
/// now, optionally of one kind.
///
/// Authorised by the **world**, not by the account — that is the point of the
/// list. A co-Game Master at this table reads the owner's book through here
/// and cannot reach it through [`crate::compendium::store`], which refuses
/// everyone but the owner.
///
/// A book the world's system no longer matches serves **nothing** (FR-042).
/// It stays on the list and says so; continuing to hand out its entries is
/// precisely the "quietly continuing to serve it" the requirement forbids.
pub fn entries_served_by(
    conn: &mut PgConnection,
    world_id: Uuid,
    compendium_id: Uuid,
    kind: Option<&str>,
) -> Result<(String, Vec<StoredEntry>), BookListError> {
    let listed = books_on(conn, world_id)?
        .into_iter()
        .find(|listed| listed.row.compendium_id == compendium_id)
        .ok_or(BookListError::NotOnTheList)?;

    if !listed.system_matches {
        let world_system = system_of_world(conn, world_id)?;
        return Err(BookListError::SystemMismatch {
            book_system: listed.system_id,
            world_system,
        });
    }

    let mut query = compendium_entries::table
        .filter(compendium_entries::compendium_id.eq(compendium_id))
        .into_boxed();
    if let Some(kind) = kind {
        query = query.filter(compendium_entries::kind.eq(kind.to_string()));
    }

    let entries = query
        .order((
            compendium_entries::kind.asc(),
            compendium_entries::name.asc(),
        ))
        .select(StoredEntry::as_select())
        .load(conn)?;

    Ok((listed.book_title, entries))
}

/// Which of this account's worlds have this book switched on (050 FR-060,
/// 049 FR-045).
///
/// What a removal from the library must name before it takes anything. Every
/// world here loses the book's content the moment the compendium goes, and
/// the cascade on `world_books.compendium_id` is what makes that true rather
/// than dangling.
pub fn worlds_with_book(
    conn: &mut PgConnection,
    compendium_id: Uuid,
) -> Result<Vec<(Uuid, String)>, BookListError> {
    world_books::table
        .inner_join(worlds::table.on(worlds::id.eq(world_books::world_id)))
        .filter(world_books::compendium_id.eq(compendium_id))
        .order(worlds::name.asc())
        .select((worlds::id, worlds::name))
        .load(conn)
        .map_err(Into::into)
}

#[cfg(test)]
#[path = "book_list_tests.rs"]
mod tests;
