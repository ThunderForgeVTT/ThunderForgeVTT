//! The book list on the wire (spec 050 US3, FR-030 to FR-036, FR-041,
//! FR-042; spec 049 FR-042).
//!
//! Four reads and two writes, and the split between them is the whole of
//! FR-035. **Every member of a world may read the list** — players included,
//! because knowing what the table is running is the point of it — and only
//! the world's owner may write it. A player is not refused a mutation by a
//! check inside it; there is simply no mutation here that answers to anybody
//! but the owner, and `library::book_list` refuses again underneath.
//!
//! # What crosses to a player, and what does not
//!
//! FR-036: a book's **name**, its system, and how much is in it. Not an
//! entry, not a field, not a line of prose. [`GraphQLWorldBook`] has nowhere
//! to put content, which is a better guarantee than a resolver that
//! remembers to leave it out. Browsing what a book actually says — 049
//! FR-042 — is [`LibraryWorldQuery::world_compendium_entries`], and that one
//! answers to a Game Master.
//!
//! # Where the account boundary is
//!
//! Reading a compendium through `compendium::store` requires **owning** it.
//! Reading one through this module requires being **at the table it is
//! switched on for**, and that is the entire difference between using a book
//! and holding one. A co-Game Master reaches the owner's book here and
//! nowhere else, and cannot switch it on for a world of their own (FR-014).

use async_graphql::{Context, Object, SimpleObject};
use uuid::Uuid;

use crate::auth::world_membership::require_world_member;
use crate::graphql::queries::compendium::{
    GraphQLCompendium, GraphQLCompendiumEntryPage, GraphQLKindCount, decode_cursor,
    entries_per_page, kind_counts, page_of,
};
use crate::graphql::{Error, GraphQLResult, app_state, authenticated_user};
use crate::library::book_list::{self, BookListError, ListedBook, SwitchOffReport};
use crate::state::AppState;

/// One book this world is running (FR-030, FR-036).
///
/// Note what is absent, since this is the object a player is shown: no entry,
/// no prose, no field value, and not the file's hash either. Counts say how
/// big a book is; they do not say what is in it.
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "WorldBook")]
pub struct GraphQLWorldBook {
    pub compendium_id: Uuid,
    pub book_title: String,
    /// The system the book was read as — not necessarily the one this world
    /// runs today. See `system_matches`.
    pub system_id: String,
    pub entry_counts: Vec<GraphQLKindCount>,
    pub entry_total: i32,
    /// FR-042: `false` means the world changed system underneath this book.
    /// It is still listed, and it is no longer served.
    pub system_matches: bool,
    /// Which build of the reader produced the base in force when this was
    /// switched on (FR-015). The other half of that record, the file's hash,
    /// stays on the shelf: it is the owner's to see and a player has no use
    /// for it.
    pub base_parser_version: String,
    pub switched_on_at: String,
}

impl From<ListedBook> for GraphQLWorldBook {
    fn from(listed: ListedBook) -> Self {
        let entry_counts = kind_counts(&listed.entry_counts);
        Self {
            compendium_id: listed.row.compendium_id,
            book_title: listed.book_title,
            system_id: listed.system_id,
            entry_total: entry_counts.iter().map(|count| count.count).sum(),
            entry_counts,
            system_matches: listed.system_matches,
            base_parser_version: listed.row.base_parser_version,
            switched_on_at: listed.row.switched_on_at.and_utc().to_rfc3339(),
        }
    }
}

/// What switching a book off takes out of this world, said before it happens
/// (FR-013, FR-032).
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "BookSwitchOffReport")]
pub struct GraphQLSwitchOffReport {
    pub compendium_id: Uuid,
    pub book_title: String,
    /// How many entries stop being served the moment this is confirmed.
    pub entry_count: i32,
    /// A few of them by name, so this reads as a warning about *things* and
    /// not about a number.
    pub entry_names: Vec<String>,
    /// Changes this world made over the book, which would go with it. Always
    /// empty today: a world's delta is a later phase of spec 050 and has no
    /// table yet, so there is nothing to lose rather than nothing checked.
    pub deltas: Vec<String>,
    /// Whether this call switched anything off. False for the report, true
    /// for the confirmation, so a caller can never mistake one for the other.
    pub switched_off: bool,
}

impl GraphQLSwitchOffReport {
    fn from(report: SwitchOffReport, switched_off: bool) -> Self {
        Self {
            compendium_id: report.compendium_id,
            book_title: report.book_title,
            entry_count: i32::try_from(report.entry_count).unwrap_or(i32::MAX),
            entry_names: report.entry_names,
            deltas: report.deltas,
            switched_off,
        }
    }
}

/// Every refusal reaches a person as the sentence the store wrote for them.
///
/// The variants are not flattened into one message: a Game Master told "this
/// book was read as pathfinder2e and this world runs dnd5e" can act on it,
/// and one told "that failed" cannot.
fn refusal(e: BookListError) -> Error {
    Error::new(e.to_string())
}

fn connection(
    state: &AppState,
) -> GraphQLResult<
    diesel::r2d2::PooledConnection<diesel::r2d2::ConnectionManager<diesel::PgConnection>>,
> {
    state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))
}

/// Testable core of `worldBookList`.
pub async fn world_book_list_impl(
    state: &AppState,
    caller: Uuid,
    world_id: Uuid,
) -> GraphQLResult<Vec<GraphQLWorldBook>> {
    let mut conn = connection(state)?;

    tokio::task::spawn_blocking(move || {
        // Membership, not ownership: FR-035 is that a player sees this.
        require_world_member(&mut conn, caller, world_id)
            .map_err(|_| Error::new("You are not at this table."))?;

        book_list::books_on(&mut conn, world_id)
            .map(|books| books.into_iter().map(GraphQLWorldBook::from).collect())
            .map_err(refusal)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
}

/// Testable core of `compendiumsOfferedToWorld`.
pub async fn compendiums_offered_impl(
    state: &AppState,
    caller: Uuid,
    world_id: Uuid,
) -> GraphQLResult<Vec<GraphQLCompendium>> {
    let mut conn = connection(state)?;

    tokio::task::spawn_blocking(move || {
        book_list::offerable_to(&mut conn, caller, world_id)
            .map(|books| books.into_iter().map(GraphQLCompendium::from).collect())
            .map_err(refusal)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
}

/// Testable core of `worldCompendiumEntries` — the fetch (049 FR-042).
pub async fn world_compendium_entries_impl(
    state: &AppState,
    caller: Uuid,
    world_id: Uuid,
    compendium_id: Uuid,
    kind: Option<String>,
    after: Option<String>,
    first: Option<i32>,
) -> GraphQLResult<GraphQLCompendiumEntryPage> {
    let limit = entries_per_page(first);
    let after = after.map(|cursor| decode_cursor(&cursor)).transpose()?;
    let mut conn = connection(state)?;

    tokio::task::spawn_blocking(move || {
        // 049 FR-042 is a Game Master's browse. A player sees the list and
        // what is handed to them in play, which is a different surface with a
        // different rule (FR-035, FR-036).
        let role = require_world_member(&mut conn, caller, world_id)
            .map_err(|_| Error::new("You are not at this table."))?;
        // A role string this build does not recognise resolves to no role at
        // all rather than to a default one, so an unreadable membership row
        // denies rather than grants.
        if !thunderforge_authz::Role::from_stored(&role)
            .is_some_and(thunderforge_authz::Role::runs_the_world)
        {
            return Err(Error::new("Only a Game Master browses this world's books."));
        }

        let (book_title, mut entries) =
            book_list::entries_served_by(&mut conn, world_id, compendium_id, kind.as_deref())
                .map_err(refusal)?;

        page_of(&mut entries, &book_title, after, limit)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
}

/// Testable core of `switchOnCompendium`.
pub async fn switch_on_impl(
    state: &AppState,
    caller: Uuid,
    world_id: Uuid,
    compendium_id: Uuid,
) -> GraphQLResult<Vec<GraphQLWorldBook>> {
    let mut conn = connection(state)?;

    tokio::task::spawn_blocking(move || {
        book_list::switch_on(&mut conn, caller, world_id, compendium_id).map_err(refusal)?;
        // The whole list comes back rather than the one row, because that is
        // what the caller is showing and because a client that patches its
        // own copy of a list is a client whose copy can be wrong.
        book_list::books_on(&mut conn, world_id)
            .map(|books| books.into_iter().map(GraphQLWorldBook::from).collect())
            .map_err(refusal)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
}

/// Testable core of `switchOffCompendium`.
pub async fn switch_off_impl(
    state: &AppState,
    caller: Uuid,
    world_id: Uuid,
    compendium_id: Uuid,
    confirm: bool,
) -> GraphQLResult<GraphQLSwitchOffReport> {
    let mut conn = connection(state)?;

    tokio::task::spawn_blocking(move || {
        let report = book_list::switch_off_report(&mut conn, caller, world_id, compendium_id)
            .map_err(refusal)?;

        if !confirm {
            // FR-013: naming what goes *before* it goes is the reason there
            // are two calls, and this early return is what makes the first
            // one true. Because nothing was copied, switching off reaches a
            // live table — a creature standing on a scene, an item in a
            // player's hands — and that price is paid here, visibly.
            return Ok(GraphQLSwitchOffReport::from(report, false));
        }

        book_list::switch_off(&mut conn, caller, world_id, compendium_id).map_err(refusal)?;
        Ok(GraphQLSwitchOffReport::from(report, true))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
}

#[derive(Default)]
pub struct LibraryWorldQuery;

#[Object]
impl LibraryWorldQuery {
    /// Which books this table is running (FR-030, FR-035).
    ///
    /// Readable by every member, and it carries no content — see
    /// [`GraphQLWorldBook`].
    async fn world_book_list(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
    ) -> GraphQLResult<Vec<GraphQLWorldBook>> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        world_book_list_impl(state, user.user_id, world_id).await
    }

    /// What the owner of this world could switch on and has not (FR-030,
    /// FR-041): their own shelf, narrowed to this world's system.
    ///
    /// Answers to the world's owner alone. A co-Game Master asking gets the
    /// refusal, not an empty list — an empty list would read as "you have no
    /// books" and they may have plenty.
    async fn compendiums_offered_to_world(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
    ) -> GraphQLResult<Vec<GraphQLCompendium>> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        compendiums_offered_impl(state, user.user_id, world_id).await
    }

    /// Browse one switched-on book from inside the world (049 FR-042).
    ///
    /// This is the fetch: the entries come from the account's shelf every
    /// time they are asked for, and no copy of them exists in this world to
    /// read instead.
    async fn world_compendium_entries(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
        compendium_id: Uuid,
        kind: Option<String>,
        after: Option<String>,
        first: Option<i32>,
    ) -> GraphQLResult<GraphQLCompendiumEntryPage> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        world_compendium_entries_impl(
            state,
            user.user_id,
            world_id,
            compendium_id,
            kind,
            after,
            first,
        )
        .await
    }
}

#[derive(Default)]
pub struct LibraryWorldMutation;

#[Object]
impl LibraryWorldMutation {
    /// Switch a book on for a world (FR-010, FR-031, FR-033).
    ///
    /// The same call at world creation and a fortnight later. Nothing is
    /// copied: what this writes is a row saying the world reads that book.
    async fn switch_on_compendium(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
        compendium_id: Uuid,
    ) -> GraphQLResult<Vec<GraphQLWorldBook>> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        switch_on_impl(state, user.user_id, world_id, compendium_id).await
    }

    /// Switch a book off, or ask first what that takes (FR-013, FR-032).
    ///
    /// With `confirm: false` nothing changes and the reply is the warning.
    async fn switch_off_compendium(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
        compendium_id: Uuid,
        confirm: bool,
    ) -> GraphQLResult<GraphQLSwitchOffReport> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        switch_off_impl(state, user.user_id, world_id, compendium_id, confirm).await
    }
}

#[cfg(test)]
#[path = "mutations_library_tests.rs"]
mod tests;
