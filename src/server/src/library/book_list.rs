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

use thunderforge_authz::Role;

use crate::auth::account_ownership::{AccountOwned, AccountOwnershipError, require_account_owner};
use crate::auth::world_membership::require_world_member;
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
    /// `None` once the person who switched it on has deleted their account:
    /// the book stays on, and the record says only that somebody did.
    pub switched_on_by: Option<Uuid>,
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
    /// The caller may not change this world's book list: a Player, a
    /// stranger, or no such world.
    ///
    /// Who *arranges* a table's books is a question of trust, and the Owner,
    /// Game Masters and Trusted Players are trusted with it (spec 050
    /// decision 8). Whose books they are is a separate question — see
    /// [`BookListError::NotOnTheOwnersShelf`].
    #[error("only this table's Owner, Game Masters and Trusted Players change its book list")]
    MayNotManageBooks,
    /// The book is not on the **world owner's** shelf (FR-010a, FR-014).
    ///
    /// Said in terms of the owner rather than "not yours", because the person
    /// most likely to see it is a Game Master or Trusted Player offering a
    /// book of their own, and "not yours" would be false. Managing a table's
    /// list is not bringing your own books to it.
    #[error("that book is not on this world owner's shelf")]
    NotOnTheOwnersShelf,
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

/// Whether the caller may arrange this world's books, and whose shelf they
/// are arranging from. Fail-closed.
///
/// Returns the **world owner's** id, and every caller uses that rather than
/// its own for the shelf, which is the whole of FR-010a: a Game Master or
/// Trusted Player switching a book on draws from the owner's library, and
/// there is no variable in reach that would let them draw from theirs.
///
/// `worlds.created_by` is this codebase's ownership source of truth, as
/// `require_world_member` records at length. Deliberately **no admin
/// bypass**, and no parameter to add one with: switching a book on reaches
/// into an account's library, an operator is another account, and
/// `require_account_owner` refuses one for exactly that reason. An operator
/// acting against imported content has the moderation route, which is a
/// different act from putting somebody's book on somebody's table.
///
/// Shared with [`super::deltas`] rather than restated there: changing what a
/// world inherited is book material too (FR-020a), and two copies of "who is
/// trusted with this table's books" are two answers waiting to differ.
pub(crate) fn require_book_manager(
    conn: &mut PgConnection,
    caller: Uuid,
    world_id: Uuid,
) -> Result<Uuid, BookListError> {
    let owner = worlds::table
        .filter(worlds::id.eq(world_id))
        .select(worlds::created_by)
        .first::<Uuid>(conn)
        .optional()?
        // No such world is refused exactly as somebody else's world is, for
        // the reason `require_account_owner` gives: a caller who can tell the
        // two apart can walk an id space.
        .ok_or(BookListError::MayNotManageBooks)?;

    // An unreadable membership, or a role string this build does not
    // recognise, is nobody.
    let manages = require_world_member(conn, caller, world_id)
        .ok()
        .and_then(|stored| Role::from_stored(&stored))
        .is_some_and(Role::manages_content);

    if manages {
        Ok(owner)
    } else {
        Err(BookListError::MayNotManageBooks)
    }
}

/// The owner's book, or the refusal that says so.
fn from_the_owners_shelf(
    conn: &mut PgConnection,
    owner: Uuid,
    compendium_id: Uuid,
) -> Result<Compendium, BookListError> {
    let own = |e| match e {
        AccountOwnershipError::NotTheOwner => BookListError::NotOnTheOwnersShelf,
        AccountOwnershipError::Database(message) => BookListError::Database(message),
    };
    // The account gate, through the one helper rather than by comparing the
    // owner column here. The lookup is the part that gets written differently
    // at each site, not the `==`.
    require_account_owner(conn, owner, AccountOwned::Compendium(compendium_id)).map_err(own)?;
    store::load(conn, owner, compendium_id).map_err(own)
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

/// Switch a book on for a world (FR-010, FR-010a, FR-031).
///
/// The caller may be the Owner, a Game Master or a Trusted Player; the book
/// must be the Owner's in every case. `switched_on_by` records the caller,
/// because FR-015 asks who did it, not whose book it was.
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
    let owner = require_book_manager(conn, caller, world_id)?;
    let book = from_the_owners_shelf(conn, owner, compendium_id)?;
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
/// The **owner's** shelf whoever asks (FR-010a), narrowed to the system this world runs and to books that
/// are not already on. A world with no system chosen is offered nothing,
/// because a book read as one system says nothing intelligible to a world
/// that has not said what it is.
pub fn offerable_to(
    conn: &mut PgConnection,
    caller: Uuid,
    world_id: Uuid,
) -> Result<Vec<Compendium>, BookListError> {
    let owner = require_book_manager(conn, caller, world_id)?;

    let Some(world_system) = system_of_world(conn, world_id)? else {
        return Ok(Vec::new());
    };

    let already_on: Vec<Uuid> = world_books::table
        .filter(world_books::world_id.eq(world_id))
        .select(world_books::compendium_id)
        .load(conn)?;

    Ok(store::library_for(conn, owner)?
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
/// `deltas` names every change and hide this world made over the book,
/// because those go with the link: they mean nothing without the entries they
/// modify. `additions_kept` names what this world added beside the book, and
/// says it **stays** (spec 050 decision 5) — authored writing that never
/// needed the book, which a person switching a book off is otherwise most
/// likely to assume they are about to lose.
#[derive(Debug, Clone)]
pub struct SwitchOffReport {
    pub compendium_id: Uuid,
    pub book_title: String,
    /// How many entries stop being served.
    pub entry_count: i64,
    /// Some of them by name, so a person recognises what they are turning
    /// off. Capped: a Monster Manual is not a confirmation dialogue.
    pub entry_names: Vec<String>,
    /// Changes and hides this world made over the book, which go with it,
    /// each named by form, kind and name (050 FR-013, 049 FR-046). Uncapped,
    /// unlike the entry names: a Monster Manual is not a confirmation
    /// dialogue, but a world's own work is exactly what a confirmation is for.
    pub deltas: Vec<String>,
    /// What this world added beside the book, which stays (decision 5).
    pub additions_kept: Vec<String>,
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
    require_book_manager(conn, caller, world_id)?;

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

    let (lost, kept) = super::deltas::deltas_named(conn, world_id, compendium_id)?;

    Ok(SwitchOffReport {
        compendium_id,
        book_title,
        entry_count,
        entry_names,
        deltas: lost,
        additions_kept: kept,
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
    require_book_manager(conn, caller, world_id)?;

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
    let listed = require_served(conn, world_id, compendium_id)?;

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

/// The book, if this world has it switched on **and** still runs the system
/// it was read as — the two conditions under which a world may read it, and
/// therefore the two under which it may change it (FR-042).
///
/// A delta over a mismatched book is refused for the reason the book is not
/// served: a world that has moved to another system is not reading this one,
/// and an edit to something it is not reading would be invisible work.
pub(crate) fn require_served(
    conn: &mut PgConnection,
    world_id: Uuid,
    compendium_id: Uuid,
) -> Result<ListedBook, BookListError> {
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
    Ok(listed)
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
