//! The book list on the wire (spec 050 US3, FR-030 to FR-036, FR-041,
//! FR-042; spec 049 FR-042).
//!
//! Four reads and two writes, and the split between them is the whole of
//! FR-035. **Every member of a world may read the list** — players included,
//! because knowing what the table is running is the point of it — and its
//! Owner, Game Masters and Trusted Players may write it (spec 050 decision 8).
//! The rule lives in `library::book_list`, not here, so there is one place
//! it can be got wrong rather than one per resolver.
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
//!
//! A Game Master or Trusted Player arranging the list is shown the owner's
//! shelf, narrowed to what fits — as [`GraphQLOfferedBook`], which carries a
//! book's name and size and not the file hash the shelf itself shows its
//! owner. Knowing which books a table could run is arranging it; knowing the
//! owner's files is not.

use std::collections::BTreeMap;

use async_graphql::{Context, Enum, Json, Object, SimpleObject};
use uuid::Uuid;

use crate::auth::world_membership::require_world_member;
use crate::content::ReadValue;
use crate::graphql::queries::compendium::{
    GraphQLContentOrigin, GraphQLKindCount, decode_cursor, encode_cursor, entries_per_page,
    kind_counts,
};
use crate::graphql::{Error, GraphQLResult, app_state, authenticated_user};
use crate::library::book_list::{self, BookListError, ListedBook, SwitchOffReport};
use crate::library::deltas::{
    self, Content, DeltaError, EntryState, Resolution, Unattached, WorldEntry,
};
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

/// A book on the world owner's shelf that this table could switch on
/// (FR-030, FR-010a).
///
/// Narrower than `Compendium` on purpose. The shelf view carries the file's
/// hash and page counts because it is shown to the account that holds them;
/// this is shown to whoever arranges the table, who may be somebody else.
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "OfferedBook")]
pub struct GraphQLOfferedBook {
    pub id: Uuid,
    pub book_title: String,
    pub system_id: String,
    pub entry_total: i32,
    pub imported_at: String,
}

impl From<crate::compendium::store::Compendium> for GraphQLOfferedBook {
    fn from(book: crate::compendium::store::Compendium) -> Self {
        Self {
            id: book.id,
            book_title: book.book_title,
            system_id: book.system_id,
            entry_total: kind_counts(&book.entry_counts)
                .iter()
                .map(|count| count.count)
                .sum(),
            imported_at: book.created_at.and_utc().to_rfc3339(),
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
    /// Changes and hides this world made over the book, each named, because
    /// they go with it (050 FR-013, 049 FR-046).
    pub deltas: Vec<String>,
    /// What this world added beside the book, each named, because it
    /// **stays** (spec 050 decision 5): the world's own writing never needed
    /// the book.
    pub additions_kept: Vec<String>,
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
            additions_kept: report.additions_kept,
            switched_off,
        }
    }
}

/// How an entry stands in this world (FR-021, SC-009).
#[derive(Enum, Copy, Clone, Debug, PartialEq, Eq)]
#[graphql(name = "WorldEntryState")]
pub enum GraphQLEntryState {
    Inherited,
    Changed,
    Hidden,
    Added,
}

impl From<EntryState> for GraphQLEntryState {
    fn from(state: EntryState) -> Self {
        match state {
            EntryState::Inherited => GraphQLEntryState::Inherited,
            EntryState::Changed => GraphQLEntryState::Changed,
            EntryState::Hidden => GraphQLEntryState::Hidden,
            EntryState::Added => GraphQLEntryState::Added,
        }
    }
}

/// What an entry said before this world changed it (FR-024).
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "WorldEntryBefore")]
pub struct GraphQLEntryBefore {
    pub field_values: Json<serde_json::Value>,
    pub prose_text: Option<String>,
}

/// One entry as this world reads it: the book with this world's delta applied
/// (FR-022).
///
/// Carries the same fields a shelf entry does, so the browser that showed a
/// book before deltas existed reads this without learning a second shape, and
/// four more that only a world has: how the entry stands here, where it came
/// from, what it was, and whether it can be changed at all.
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "WorldEntry")]
pub struct GraphQLWorldEntry {
    /// The book entry's id, or the addition's own for an entry the book does
    /// not have.
    pub id: Uuid,
    pub compendium_id: Uuid,
    pub book_title: String,
    pub kind: String,
    pub name: String,
    pub name_uncertain: bool,
    /// `null` for an addition: it is on no page of the book (049 FR-043
    /// promises a page a person can look up, and there is none to look up).
    pub page: Option<i32>,
    pub field_values: Json<serde_json::Value>,
    pub prose_text: Option<String>,
    pub suspect: bool,
    pub state: GraphQLEntryState,
    /// Per entry, not per book (FR-052, FR-052a). A changed entry of an
    /// uploaded book says `UPLOADED`; an addition beside it says `AUTHORED`.
    pub origin: GraphQLContentOrigin,
    /// `origin`, answered as the sharing rule answers it — so no client
    /// re-derives "may this leave?" and gets it subtly different.
    pub may_be_shared: bool,
    /// Why not, in the words the refusal uses (049 FR-053). `null` when it
    /// may.
    pub not_shareable_because: Option<String>,
    /// What the book says, for a changed entry (FR-024).
    pub before: Option<GraphQLEntryBefore>,
    /// Another entry in this book has the same kind and name, so no change
    /// can attach to this one (FR-025a).
    pub ambiguous: bool,
}

impl GraphQLWorldEntry {
    fn from(entry: WorldEntry, book_title: &str) -> Self {
        Self {
            id: entry.id,
            compendium_id: entry.compendium_id,
            book_title: book_title.to_string(),
            kind: entry.kind,
            name: entry.name,
            name_uncertain: entry.name_uncertain,
            page: entry.page,
            field_values: Json(entry.field_values),
            prose_text: entry.prose_text,
            suspect: entry.suspect,
            state: entry.state.into(),
            origin: entry.origin.into(),
            may_be_shared: entry.origin.may_be_shared(),
            not_shareable_because: entry.origin.refusal_reason().map(str::to_string),
            before: entry.before.map(|before| GraphQLEntryBefore {
                field_values: Json(before.field_values),
                prose_text: before.prose_text,
            }),
            ambiguous: entry.ambiguous,
        }
    }
}

/// A change this world holds and is not applying (FR-025a, FR-027).
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "UnattachedWorldDelta")]
pub struct GraphQLUnattachedDelta {
    pub kind: String,
    pub name: String,
    /// `changed`, `hidden` or `added`.
    pub form: String,
    /// Said for a person, because the person reading it has to decide
    /// whether to restore it or wait for a better reading of the book.
    pub reason: String,
}

impl GraphQLUnattachedDelta {
    fn from((delta, why): (deltas::Delta, Unattached)) -> Self {
        let reason = match why {
            Unattached::Ambiguous { count } => format!(
                "This book has {count} {} entries named \"{}\", so this change cannot tell which one it belongs to and is applied to neither.",
                delta.kind, delta.name
            ),
            Unattached::NoSuchEntry => format!(
                "This book no longer has a {} named \"{}\".",
                delta.kind, delta.name
            ),
            Unattached::ShadowsTheBook => format!(
                "This book now has its own {} named \"{}\", so the one added here is not shown beside it.",
                delta.kind, delta.name
            ),
        };
        Self {
            kind: delta.kind,
            name: delta.name,
            form: delta.form.word().to_string(),
            reason,
        }
    }
}

/// One page of what a world reads from a book.
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "WorldEntryPage")]
pub struct GraphQLWorldEntryPage {
    pub entries: Vec<GraphQLWorldEntry>,
    pub next_cursor: Option<String>,
    pub total: i32,
    /// Every held change that could not be applied, on every page — it is
    /// about the book, not about the entries this page happens to show.
    pub unattached: Vec<GraphQLUnattachedDelta>,
}

/// Cut a resolved book at the cursor. The resolution is already in the total
/// order `page_of` uses for the shelf, so a cursor means the same place on
/// both surfaces.
fn page_of_world(
    resolution: Resolution,
    book_title: &str,
    after: Option<Uuid>,
    limit: usize,
) -> GraphQLResult<GraphQLWorldEntryPage> {
    let Resolution {
        entries,
        unattached,
    } = resolution;
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
    let remaining = entries.len().saturating_sub(start);

    let page: Vec<GraphQLWorldEntry> = entries
        .into_iter()
        .skip(start)
        .take(limit)
        .map(|entry| GraphQLWorldEntry::from(entry, book_title))
        .collect();
    let next_cursor = (page.len() < remaining)
        .then(|| page.last().map(|entry| encode_cursor(entry.id)))
        .flatten();

    Ok(GraphQLWorldEntryPage {
        entries: page,
        next_cursor,
        total,
        unattached: unattached
            .into_iter()
            .map(GraphQLUnattachedDelta::from)
            .collect(),
    })
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
) -> GraphQLResult<Vec<GraphQLOfferedBook>> {
    let mut conn = connection(state)?;

    tokio::task::spawn_blocking(move || {
        book_list::offerable_to(&mut conn, caller, world_id)
            .map(|books| books.into_iter().map(GraphQLOfferedBook::from).collect())
            .map_err(refusal)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
}

/// Testable core of `worldCompendiumEntries` — the fetch (049 FR-042), with
/// this world's delta applied (050 FR-022).
#[allow(clippy::too_many_arguments)]
pub async fn world_compendium_entries_impl(
    state: &AppState,
    caller: Uuid,
    world_id: Uuid,
    compendium_id: Uuid,
    kind: Option<String>,
    after: Option<String>,
    first: Option<i32>,
    show_hidden: bool,
) -> GraphQLResult<GraphQLWorldEntryPage> {
    let limit = entries_per_page(first);
    let after = after.map(|cursor| decode_cursor(&cursor)).transpose()?;
    let mut conn = connection(state)?;

    tokio::task::spawn_blocking(move || {
        require_content_manager(&mut conn, caller, world_id)?;

        let (book_title, resolution) = deltas::world_reads(
            &mut conn,
            world_id,
            compendium_id,
            kind.as_deref(),
            show_hidden,
        )
        .map_err(delta_refusal)?;

        page_of_world(resolution, &book_title, after, limit)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
}

/// Testable core of `worldAdditionsWithoutBook` (spec 050 decision 5).
///
/// Each entry's `bookTitle` is the book it was written beside, which is how it
/// rejoins that book's page if the book is switched back on.
pub async fn additions_without_book_impl(
    state: &AppState,
    caller: Uuid,
    world_id: Uuid,
) -> GraphQLResult<Vec<GraphQLWorldEntry>> {
    let mut conn = connection(state)?;

    tokio::task::spawn_blocking(move || {
        // The same readers as the book pages these additions were written on.
        require_content_manager(&mut conn, caller, world_id)?;
        deltas::additions_without_their_book(&mut conn, world_id)
            .map(|found| {
                found
                    .into_iter()
                    .map(|(title, entry)| GraphQLWorldEntry::from(entry, &title))
                    .collect()
            })
            .map_err(|_| Error::new("This world's own additions could not be read."))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
}

/// 049 FR-042: browsing a world's books belongs to the people who manage its
/// book material, which includes a Trusted Player (owner, 2026-09-13).
/// FR-020a already let them change what a world inherited, and nobody can
/// change entries they cannot read. A Player sees the list and what is handed
/// to them in play, which is a different surface with a different rule
/// (FR-035, FR-036).
fn require_content_manager(
    conn: &mut diesel::PgConnection,
    caller: Uuid,
    world_id: Uuid,
) -> GraphQLResult<()> {
    let role = require_world_member(conn, caller, world_id)
        .map_err(|_| Error::new("You are not at this table."))?;
    // A role string this build does not recognise resolves to no role at
    // all rather than to a default one, so an unreadable membership row
    // denies rather than grants.
    if !thunderforge_authz::Role::from_stored(&role)
        .is_some_and(thunderforge_authz::Role::manages_content)
    {
        return Err(Error::new(
            "Only a Game Master or Trusted Player browses this world's books.",
        ));
    }
    Ok(())
}

fn delta_refusal(e: DeltaError) -> Error {
    Error::new(e.to_string())
}

/// What a person sent as an entry's content: fields or prose, and exactly one.
///
/// The wire has two optional arguments because GraphQL has no input union;
/// this is where that looseness ends; past here, [`Content`] cannot hold both.
fn content_from(
    field_values: Option<Json<BTreeMap<String, ReadValue>>>,
    prose_text: Option<String>,
) -> GraphQLResult<Content> {
    match (field_values, prose_text) {
        (Some(Json(fields)), None) => Ok(Content::Fields(fields)),
        (None, Some(text)) => Ok(Content::Prose(text)),
        _ => Err(Error::new(
            "Send either fields or prose for an entry, not both and not neither.",
        )),
    }
}

/// Which change to a world's entry a mutation makes. One enum, so the four
/// resolvers share one gate, one blocking hop and one reply shape rather than
/// four copies of each that can drift.
enum EntryChange {
    Change(Content),
    Hide,
    Add(Content),
    Restore,
}

/// Testable core of `changeWorldEntry`, `hideWorldEntry`, `addWorldEntry` and
/// `restoreWorldEntry`.
///
/// Replies with the entry as the world now reads it — hidden entries
/// included, marked — or `null` where there is no longer one to read: an
/// addition that was taken out.
pub async fn change_world_entry_impl(
    state: &AppState,
    caller: Uuid,
    world_id: Uuid,
    compendium_id: Uuid,
    kind: String,
    name: String,
    change: EntryChangeRequest,
) -> GraphQLResult<Option<GraphQLWorldEntry>> {
    let change = match change {
        EntryChangeRequest::Change {
            field_values,
            prose_text,
        } => EntryChange::Change(content_from(field_values, prose_text)?),
        EntryChangeRequest::Add {
            field_values,
            prose_text,
        } => EntryChange::Add(content_from(field_values, prose_text)?),
        EntryChangeRequest::Hide => EntryChange::Hide,
        EntryChangeRequest::Restore => EntryChange::Restore,
    };
    let is_restore = matches!(change, EntryChange::Restore);
    let mut conn = connection(state)?;

    tokio::task::spawn_blocking(move || {
        // The rule for who may change a world's books lives in
        // `library::deltas`, which asks the book list's own gate. Nothing
        // here decides it.
        let c = &mut conn;
        match change {
            EntryChange::Change(content) => {
                deltas::change_entry(c, caller, world_id, compendium_id, &kind, &name, content)
                    .map(|_| ())
            }
            EntryChange::Hide => {
                deltas::hide_entry(c, caller, world_id, compendium_id, &kind, &name).map(|_| ())
            }
            EntryChange::Add(content) => {
                deltas::add_entry(c, caller, world_id, compendium_id, &kind, &name, content)
                    .map(|_| ())
            }
            EntryChange::Restore => {
                deltas::restore_entry(c, caller, world_id, compendium_id, &kind, &name).map(|_| ())
            }
        }
        .map_err(delta_refusal)?;

        // Removing an addition whose book is switched off leaves nothing to
        // read back (decision 5 kept it; the person has now let it go).
        let book_title = match book_list::require_served(c, world_id, compendium_id) {
            Ok(listed) => listed.book_title,
            Err(BookListError::NotOnTheList) if is_restore => return Ok(None),
            Err(e) => return Err(refusal(e)),
        };
        deltas::world_entry(c, world_id, compendium_id, &kind, &name)
            .map(|entry| entry.map(|entry| GraphQLWorldEntry::from(entry, &book_title)))
            .map_err(delta_refusal)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
}

/// The four changes as a caller names them, before their content is checked.
pub enum EntryChangeRequest {
    Change {
        field_values: Option<Json<BTreeMap<String, ReadValue>>>,
        prose_text: Option<String>,
    },
    Hide,
    Add {
        field_values: Option<Json<BTreeMap<String, ReadValue>>>,
        prose_text: Option<String>,
    },
    Restore,
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
    /// FR-041): the owner's shelf, narrowed to this world's system.
    ///
    /// Answers to the Owner, Game Masters and Trusted Players, and it is the
    /// **owner's** shelf for all of them (FR-010a). A Player asking gets the
    /// refusal, not an empty list — an empty list would read as "the owner
    /// has no books" when it means "this is not yours to ask".
    async fn compendiums_offered_to_world(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
    ) -> GraphQLResult<Vec<GraphQLOfferedBook>> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        compendiums_offered_impl(state, user.user_id, world_id).await
    }

    /// Browse one switched-on book from inside the world (049 FR-042), as
    /// this world reads it (050 FR-022).
    ///
    /// This is the fetch: the entries come from the account's shelf every
    /// time they are asked for, with this world's changes laid over them, and
    /// no copy of either exists in this world to read instead. `showHidden`
    /// includes what this world hides, marked, so it can be put back.
    #[allow(clippy::too_many_arguments)]
    async fn world_compendium_entries(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
        compendium_id: Uuid,
        kind: Option<String>,
        after: Option<String>,
        first: Option<i32>,
        show_hidden: Option<bool>,
    ) -> GraphQLResult<GraphQLWorldEntryPage> {
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
            show_hidden.unwrap_or(false),
        )
        .await
    }
    /// What this world added beside books it has since switched off (spec
    /// 050 decision 5). Kept, authored, and readable here until somebody
    /// removes it with `restoreWorldEntry`.
    async fn world_additions_without_book(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
    ) -> GraphQLResult<Vec<GraphQLWorldEntry>> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        additions_without_book_impl(state, user.user_id, world_id).await
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

    /// Change what an entry says in this world, and in no other world and not
    /// in the book (050 FR-020, FR-023). Send `fieldValues` for an entry read
    /// as fields — only the ones named are touched — or `proseText` for prose.
    #[allow(clippy::too_many_arguments)]
    async fn change_world_entry(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
        compendium_id: Uuid,
        kind: String,
        name: String,
        field_values: Option<Json<BTreeMap<String, ReadValue>>>,
        prose_text: Option<String>,
    ) -> GraphQLResult<Option<GraphQLWorldEntry>> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        change_world_entry_impl(
            state,
            user.user_id,
            world_id,
            compendium_id,
            kind,
            name,
            EntryChangeRequest::Change {
                field_values,
                prose_text,
            },
        )
        .await
    }

    /// Stop showing an entry in this world. Every other world still has it.
    async fn hide_world_entry(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
        compendium_id: Uuid,
        kind: String,
        name: String,
    ) -> GraphQLResult<Option<GraphQLWorldEntry>> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        change_world_entry_impl(
            state,
            user.user_id,
            world_id,
            compendium_id,
            kind,
            name,
            EntryChangeRequest::Hide,
        )
        .await
    }

    /// Write an entry into this world beside the book. Authored, and so
    /// shareable, however uploaded the book beside it is (050 FR-052a).
    #[allow(clippy::too_many_arguments)]
    async fn add_world_entry(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
        compendium_id: Uuid,
        kind: String,
        name: String,
        field_values: Option<Json<BTreeMap<String, ReadValue>>>,
        prose_text: Option<String>,
    ) -> GraphQLResult<Option<GraphQLWorldEntry>> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        change_world_entry_impl(
            state,
            user.user_id,
            world_id,
            compendium_id,
            kind,
            name,
            EntryChangeRequest::Add {
                field_values,
                prose_text,
            },
        )
        .await
    }

    /// Put an entry back as the book has it, or take an addition out
    /// (050 FR-024).
    async fn restore_world_entry(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
        compendium_id: Uuid,
        kind: String,
        name: String,
    ) -> GraphQLResult<Option<GraphQLWorldEntry>> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        change_world_entry_impl(
            state,
            user.user_id,
            world_id,
            compendium_id,
            kind,
            name,
            EntryChangeRequest::Restore,
        )
        .await
    }
}

#[cfg(test)]
#[path = "mutations_library_tests.rs"]
mod tests;
