//! Committing a reviewed book, and taking one back off the shelf (spec 049
//! US2 and US3, `contracts/import.md`).
//!
//! The book itself never arrives here. It was read in the Game Master's own
//! browser (FR-020) and what crosses the wire is the entries a person looked
//! at and approved — so this module is an ordinary GraphQL mutation with a
//! structured payload, and deliberately not another upload route.
//!
//! # A review that happened in a browser is not a permission
//!
//! FR-036 and constitution Principle III. Everything the review did, the
//! server does again on arrival, before a row is written:
//!
//! 1. the compendium belongs to the caller — by construction for a fresh
//!    import (there is no owner field for a payload to name somebody else in)
//!    and through [`require_account_owner`] for an overwrite;
//! 2. the system exists and declares content patterns, **re-read from the
//!    manifest** and never taken from the payload (FR-015);
//! 3. the entry count is within [`MAX_ENTRIES_PER_IMPORT`] (FR-035);
//! 4. every entry's kind is one that system declared — a kind the manifest
//!    does not name is a payload this flow did not produce.
//!
//! The order matters slightly: all four are answered before the transaction
//! opens, so a refusal costs a manifest read rather than a rolled-back write.
//!
//! # Origin is not an input
//!
//! FR-051. It is written by [`crate::compendium::store::import_book`], is not
//! a field on anything a client can send, and there is no resolver here that
//! could forward one. A field the client can set is a field an attacker can
//! set, and this is the field every sharing rule in the arc is enforced
//! against.
//!
//! # All or nothing
//!
//! The store applies an import in a single transaction (FR-032), and this
//! module deliberately does not open a second one around it: either the whole
//! compendium and all its entries exist afterwards, or none of it does. That
//! is also what makes FR-034's abandonment free — a dropped request leaves
//! nothing behind because nothing partial was ever committed.

use std::collections::BTreeMap;

use async_graphql::{Context, Error, InputObject, Json, Object, SimpleObject};
use diesel::prelude::*;
use thunderforge_canvas_core::content_patterns::ContentPatterns;
use uuid::Uuid;

use crate::auth::account_ownership::{AccountOwned, require_account_owner};
use crate::compendium::store::{self, ImportError, NewBook};
use crate::content::{Entry, NameState, ReadValue};
use crate::content_patterns::content_patterns_for_system;
use crate::graphql::mutations_library::GraphQLUnattachedDelta;
use crate::graphql::queries::compendium::GraphQLCompendium;
use crate::graphql::{GraphQLResult, app_state, authenticated_user};
use crate::schema::compendium_entries;
use crate::state::AppState;

/// The most entries one import may carry (FR-035).
///
/// Ten thousand, and the number is chosen against measurement rather than
/// picked for roundness. The largest single books in the 246-book sample
/// yielded creatures in the high hundreds — 717 across six books — so a real
/// sourcebook is an order of magnitude under this and no Game Master will
/// ever meet the bound with a genuine one.
///
/// What a payload past it actually means is a declaration that matches too
/// much: the measured failure was a prose pattern with no discriminator
/// returning 72,974 "magic items" from a library, including tables of
/// contents. That is the case this refuses, and refusing it early is what
/// keeps one transaction to twenty chunked inserts and the JSON body inside
/// the 50 MB the GraphQL route accepts.
pub const MAX_ENTRIES_PER_IMPORT: usize = 10_000;

/// One entry, as the review approved it.
#[derive(InputObject, Debug, Clone)]
pub struct ImportedEntryInput {
    /// The system's own word for what this is. Checked against the manifest,
    /// never switched on.
    pub kind: String,
    pub name: String,
    pub name_uncertain: bool,
    /// One-based, as a person would cite it.
    pub page: i32,
    /// The declared fields, each carrying its own certainty.
    ///
    /// Carried in the shared [`ReadValue`] shape rather than re-described as
    /// GraphQL input objects, and that is what keeps FR-002 true across the
    /// wire: `unread` is a variant with nowhere to put a value, so a payload
    /// claiming a field was not found *and* supplying its value does not
    /// deserialise at all. A pair of `{ state, value }` fields would have
    /// admitted exactly that, and anything admissible eventually arrives.
    pub values: Option<Json<BTreeMap<String, ReadValue>>>,
    /// Prose kinds only: what the entry says.
    pub text: Option<String>,
    /// The reader distrusted the lines this was built from (FR-004).
    pub suspect: bool,
    /// Whatever the system's own pack read that a declaration could not
    /// express. Opaque here exactly as it is everywhere else.
    pub extras: Option<Json<serde_json::Value>>,
}

/// What a reviewed book asks the server to record.
///
/// **There is no origin field, and no owner field.** The first is written by
/// the store (FR-051); the second is the authenticated caller, so a payload
/// has no way to name somebody else's shelf. Both absences are the check
/// rather than a comment about one.
#[derive(InputObject, Debug, Clone)]
pub struct CreateCompendiumFromImportInput {
    pub book_title: String,
    /// SHA-256 of the file, hex, taken in the browser before anything was
    /// sent (spec 047 FR-072).
    pub source_hash: String,
    /// The system the book was read as. Re-read from the manifest here; what
    /// the payload says is only which manifest to open.
    pub system_id: String,
    /// Which build of the reader produced this, so a later read can tell "the
    /// book changed" from "the reader improved".
    pub parser_version: String,
    pub page_count: i32,
    pub silent_page_count: i32,
    /// FR-047's overwrite: the compendium this re-read replaces. Absent for a
    /// first import, which is the ordinary case.
    pub replaces_compendium_id: Option<Uuid>,
    pub entries: Vec<ImportedEntryInput>,
}

/// A world that is using something this compendium contributed (FR-045).
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLCompendiumUsage {
    pub world_id: Uuid,
    pub world_name: String,
    /// What is in use there, named so a person recognises it — a creature on
    /// a scene, an item in an inventory.
    pub entry_names: Vec<String>,
}

/// One world's share of a book's removal (spec 050 decision 5).
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "RemovalWorldDeltas")]
pub struct GraphQLRemovalWorldDeltas {
    pub world_id: Uuid,
    pub world_name: String,
    /// Changes and hides, which go with the book.
    pub lost: Vec<String>,
    /// Additions, which stay in the world as its own writing, labelled with
    /// this book's title.
    pub kept: Vec<String>,
}

/// What removing this compendium would take with it, and what it would leave
/// (FR-045, FR-046).
///
/// Returned by both calls. With `confirm: false` it is the whole answer and
/// nothing has changed; with `confirm: true` it describes what was removed.
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLRemovalReport {
    pub compendium_id: Uuid,
    pub book_title: String,
    /// How many entries this import contributed, all of which go.
    pub entry_count: i32,
    /// Where those entries are in use today, per world.
    ///
    /// One row per world with this book switched on (050 FR-060). Because
    /// nothing was copied into a world, "in use" is not a subset to compute:
    /// a table running the book loses all of it, and that is what this says.
    pub in_use: Vec<GraphQLCompendiumUsage>,
    /// Entries a table has changed or hidden over this book, which go with it
    /// (FR-046), each prefixed with the world it is in.
    ///
    /// Under spec 050 a hand edit is a world's delta over the base, never a
    /// change to `compendium_entries`, which is also why a re-import cannot
    /// silently overwrite one. `deltasByWorld` carries the same, per world,
    /// beside what stays.
    pub hand_edited: Vec<String>,
    /// Per world, what removing the book does to that table's own work:
    /// changes and hides are lost, additions are **kept** (spec 050 decision
    /// 5). Includes worlds that have switched the book off and kept an
    /// addition, which `inUse` does not name.
    pub deltas_by_world: Vec<GraphQLRemovalWorldDeltas>,
    /// Whether this call removed anything. False for the report, true for the
    /// confirmation — so a caller can never mistake one reply for the other.
    pub removed: bool,
}

/// One world's changes over a book that its reading in force no longer takes
/// (050 FR-027).
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "WorldUnattachedDeltas")]
pub struct GraphQLWorldUnattachedDeltas {
    pub world_id: Uuid,
    pub world_name: String,
    /// Never empty: a world whose changes all still attach is not listed.
    pub deltas: Vec<GraphQLUnattachedDelta>,
}

/// Everything the four arrival checks can refuse, phrased for the person
/// refused (FR-033).
fn refusal(reason: impl Into<String>) -> Error {
    Error::new(reason.into())
}

/// Turn one reviewed entry into what the reader produces, refusing anything
/// that is not a reading.
fn entry_from(input: ImportedEntryInput) -> GraphQLResult<Entry> {
    let page = u32::try_from(input.page)
        .map_err(|_| refusal(format!("'{}' has no page number", input.name)))?;
    if page == 0 {
        // Pages are one-based everywhere in this feature, because the point
        // of keeping one is that a Game Master can look it up in the book.
        return Err(refusal(format!(
            "'{}' claims page 0, and books start at page 1",
            input.name
        )));
    }

    Ok(Entry {
        kind: input.kind,
        name: input.name,
        name_state: if input.name_uncertain {
            NameState::Uncertain
        } else {
            NameState::Clear
        },
        page,
        values: input.values.map(|v| v.0).unwrap_or_default(),
        text: input.text,
        suspect: input.suspect,
        extras: input.extras.map(|e| e.0),
    })
}

/// Checks three and four, over the entries a payload carries.
///
/// Separate from the resolver and free of a connection, so the two refusals
/// it owns can be tested for what they are rather than through a database.
fn checked_entries(
    patterns: &ContentPatterns,
    system_id: &str,
    entries: Vec<ImportedEntryInput>,
) -> GraphQLResult<Vec<Entry>> {
    if entries.len() > MAX_ENTRIES_PER_IMPORT {
        // The bound is named, because FR-035 asks for it to be and because a
        // number a person can see is a number they can argue with.
        return Err(refusal(format!(
            "This import carries {} entries, and an import may carry at most {MAX_ENTRIES_PER_IMPORT}. \
             A book producing more than that is almost always a pattern matching far more than it \
             should rather than a very large book.",
            entries.len()
        )));
    }

    entries
        .into_iter()
        .map(|input| {
            if patterns.for_kind(&input.kind).is_none() {
                return Err(refusal(format!(
                    "'{}' is not a kind of content {system_id} declares, so it cannot have come \
                     from reading a book as that system.",
                    input.kind
                )));
            }
            entry_from(input)
        })
        .collect()
}

/// Where a compendium's entries are in use, per world (FR-045, 050 FR-060).
///
/// Spec 050's book list is what taught this function, as the note that stood
/// here predicted. Every world with the book switched on loses the whole of
/// it the moment the compendium goes — not some entries but all of them,
/// because nothing was copied and what those tables were reading was this.
///
/// A lookup that fails names nothing rather than refusing the report, and the
/// choice is deliberate in one direction only: a report that cannot be
/// assembled must not block a removal a person has asked for twice, and the
/// removal itself is still gated by ownership.
fn usage_of(conn: &mut PgConnection, compendium_id: Uuid) -> Vec<GraphQLCompendiumUsage> {
    let Ok(worlds) = crate::library::book_list::worlds_with_book(conn, compendium_id) else {
        return Vec::new();
    };

    // A few entries by name, so a Game Master recognises what leaves each
    // table rather than reading a count. The same list for every world,
    // because the same book leaves every one of them.
    let names: Vec<String> = compendium_entries::table
        .filter(compendium_entries::compendium_id.eq(compendium_id))
        .order(compendium_entries::name.asc())
        .limit(12)
        .select(compendium_entries::name)
        .load::<String>(conn)
        .unwrap_or_default();

    worlds
        .into_iter()
        .map(|(world_id, world_name)| GraphQLCompendiumUsage {
            world_id,
            world_name,
            entry_names: names.clone(),
        })
        .collect()
}

/// What removing this book does to every world's deltas over it (FR-046,
/// spec 050 decision 5).
///
/// A report that cannot be read names nothing rather than refusing, for the
/// reason `usage_of` gives: the removal itself is still gated by ownership.
fn deltas_by_world(conn: &mut PgConnection, compendium_id: Uuid) -> Vec<GraphQLRemovalWorldDeltas> {
    crate::library::deltas::removal_consequences(conn, compendium_id)
        .unwrap_or_default()
        .into_iter()
        .map(|each| GraphQLRemovalWorldDeltas {
            world_id: each.world_id,
            world_name: each.world_name,
            lost: each.lost,
            kept: each.kept,
        })
        .collect()
}

/// Testable core of `createCompendiumFromImport`.
///
/// `systems_dir` is a parameter rather than read from `state` so a test can
/// stand a system up of its own and prove the manifest is what is consulted.
/// The resolver always passes the instance's.
pub async fn create_compendium_from_import_impl(
    state: &AppState,
    systems_dir: &str,
    owner: Uuid,
    input: CreateCompendiumFromImportInput,
) -> GraphQLResult<GraphQLCompendium> {
    // Check two. The manifest is the authority on what this system declares;
    // the payload only says which manifest to open. A system that declares
    // nothing is refused with a reason rather than defaulted to anything —
    // reading a book with another system's vocabulary produces entries that
    // look plausible and are wrong (FR-015).
    let patterns = content_patterns_for_system(systems_dir, &input.system_id);
    if patterns.is_empty() {
        return Err(refusal(
            "This game system does not say what its content looks like in a book, so nothing can \
             be read into it. A system declares that in its own manifest.",
        ));
    }

    // Checks three and four, before a connection is even taken.
    let entries = checked_entries(&patterns, &input.system_id, input.entries)?;

    let book = NewBook {
        book_title: input.book_title,
        source_hash: input.source_hash,
        system_id: input.system_id,
        parser_version: input.parser_version,
        page_count: input.page_count,
        silent_page_count: input.silent_page_count,
    };
    let replaces = input.replaces_compendium_id;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        let Some(existing_id) = replaces else {
            // A first import. Check one holds by construction: the owner is
            // the authenticated caller and the payload had nowhere to name
            // anybody else.
            return match store::import_book(&mut conn, owner, book, &entries) {
                Ok(compendium) => Ok(GraphQLCompendium::from(compendium)),
                Err(ImportError::Database(diesel::result::Error::DatabaseError(
                    diesel::result::DatabaseErrorKind::UniqueViolation,
                    _,
                ))) => Err(refusal(
                    "You have already imported this file. Open the book you have and re-read it \
                     if you want to replace what is in it.",
                )),
                Err(e) => Err(refusal(format!("The import did not happen: {e}"))),
            };
        };

        // Check one, for an overwrite: somebody else's shelf is refused here
        // and not compared inline. `replace_entries` asks again — this gate
        // is the one that runs before the hash below is read.
        require_account_owner(&mut conn, owner, AccountOwned::Compendium(existing_id))
            .map_err(|e| refusal(e.to_string()))?;

        let existing =
            store::load(&mut conn, owner, existing_id).map_err(|e| refusal(e.to_string()))?;

        // An overwrite is a re-read of *this* file. A different hash is a
        // different file — a later printing, a re-save, a scan of the same
        // book — and claiming it as a match would overwrite one book with
        // another while the shelf went on naming the first one's hash (spec
        // 047 FR-075).
        // A collection has no file, so no re-read can match it: it is
        // written, not read, and never replaced by an import.
        if existing.source_hash.as_deref() != Some(book.source_hash.as_str()) {
            return Err(refusal(
                "This is not the file that book was read from. It may be a different printing or \
                 a re-saved copy; import it as its own book rather than over this one.",
            ));
        }

        store::replace_entries(
            &mut conn,
            owner,
            existing_id,
            &book.parser_version,
            &entries,
        )
        .map_err(|e| refusal(format!("The re-import did not happen: {e}")))?;

        store::load(&mut conn, owner, existing_id)
            .map(GraphQLCompendium::from)
            .map_err(|e| refusal(e.to_string()))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
}

/// Testable core of `removeCompendium`.
pub async fn remove_compendium_impl(
    state: &AppState,
    caller: Uuid,
    compendium_id: Uuid,
    confirm: bool,
) -> GraphQLResult<GraphQLRemovalReport> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        // Ownership first, and through the one helper. A stranger learns
        // nothing from this call, including whether the id exists.
        let compendium =
            store::load(&mut conn, caller, compendium_id).map_err(|e| refusal(e.to_string()))?;

        let entry_count: i64 = compendium_entries::table
            .filter(compendium_entries::compendium_id.eq(compendium_id))
            .count()
            .get_result(&mut conn)
            .map_err(|e| refusal(format!("Failed to read the book: {e}")))?;

        let by_world = deltas_by_world(&mut conn, compendium_id);
        let report = GraphQLRemovalReport {
            compendium_id,
            book_title: compendium.book_title,
            entry_count: i32::try_from(entry_count).unwrap_or(i32::MAX),
            in_use: usage_of(&mut conn, compendium_id),
            hand_edited: by_world
                .iter()
                .flat_map(|world| {
                    world
                        .lost
                        .iter()
                        .map(move |lost| format!("{}: {lost}", world.world_name))
                })
                .collect(),
            deltas_by_world: by_world,
            removed: false,
        };

        if !confirm {
            // FR-045: naming what is in use *before* it is confirmed is the
            // whole reason there are two calls. This one changes nothing, and
            // the early return is what says so.
            return Ok(report);
        }

        store::remove(&mut conn, caller, compendium_id).map_err(|e| refusal(e.to_string()))?;

        Ok(GraphQLRemovalReport {
            removed: true,
            ..report
        })
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
}

/// Testable core of `compendiumUnattachedDeltas`.
///
/// The owner only, through the same load every other shelf call uses: a book
/// list draws on its world owner's shelf, so the worlds named here are the
/// owner's own, and a stranger learns nothing, including whether the id
/// exists.
pub async fn compendium_unattached_deltas_impl(
    state: &AppState,
    caller: Uuid,
    compendium_id: Uuid,
) -> GraphQLResult<Vec<GraphQLWorldUnattachedDeltas>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        store::load(&mut conn, caller, compendium_id).map_err(|e| refusal(e.to_string()))?;
        let worlds = crate::library::deltas::unattached_after_reimport(&mut conn, compendium_id)
            .map_err(|e| refusal(format!("Failed to read this book's changes: {e}")))?;
        Ok(worlds
            .into_iter()
            .map(|world| GraphQLWorldUnattachedDeltas {
                world_id: world.world_id,
                world_name: world.world_name,
                deltas: world
                    .deltas
                    .into_iter()
                    .map(GraphQLUnattachedDelta::from)
                    .collect(),
            })
            .collect())
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
}

/// Whether this account already holds the book in this file (FR-047).
pub async fn compendium_for_file_hash_impl(
    state: &AppState,
    owner: Uuid,
    source_hash: String,
) -> GraphQLResult<Option<GraphQLCompendium>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        store::find_by_source_hash(&mut conn, owner, &source_hash)
            .map(|found| found.map(GraphQLCompendium::from))
            .map_err(|e| Error::new(format!("Failed to look for this book: {e}")))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
}

#[derive(Default)]
pub struct CompendiumMutation;

#[Object]
impl CompendiumMutation {
    /// Record a reviewed book as a compendium on the caller's own shelf.
    async fn create_compendium_from_import(
        &self,
        ctx: &Context<'_>,
        input: CreateCompendiumFromImportInput,
    ) -> GraphQLResult<GraphQLCompendium> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        let systems_dir = state.directories.systems_dir.clone();
        create_compendium_from_import_impl(state, &systems_dir, user.user_id, input).await
    }

    /// Take a book off the shelf, or ask first what that would take with it.
    ///
    /// With `confirm: false` this reports and changes nothing; with
    /// `confirm: true` it removes that import's contribution and nothing
    /// else (FR-044, FR-045).
    async fn remove_compendium(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        confirm: bool,
    ) -> GraphQLResult<GraphQLRemovalReport> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        remove_compendium_impl(state, user.user_id, id, confirm).await
    }
}

#[derive(Default)]
pub struct CompendiumImportQuery;

#[Object]
impl CompendiumImportQuery {
    /// Does this account already hold the book in this file? (FR-047.)
    ///
    /// Asked **before** the upload, with a hash and nothing else: a hash is
    /// not content, and catching a book the account already has is worth far
    /// more before some megabytes of entries are sent than after.
    ///
    /// Scoped to the caller's own library, never across accounts — importing
    /// a book a second time for a second world is the duplication this arc
    /// exists to prevent, and matching across accounts would be a different
    /// product with a different consent story.
    async fn compendium_for_file_hash(
        &self,
        ctx: &Context<'_>,
        source_hash: String,
    ) -> GraphQLResult<Option<GraphQLCompendium>> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        compendium_for_file_hash_impl(state, user.user_id, source_hash).await
    }

    /// Changes the caller's worlds hold over this book that its reading in
    /// force no longer takes, per world, each with why (050 FR-027).
    ///
    /// Asked after a re-import, so the person who re-read the book is told
    /// what the re-read stranded. Nothing is removed: each change stays with
    /// its world until somebody there restores it.
    async fn compendium_unattached_deltas(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
    ) -> GraphQLResult<Vec<GraphQLWorldUnattachedDeltas>> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        compendium_unattached_deltas_impl(state, user.user_id, id).await
    }
}

#[cfg(test)]
#[path = "mutations_compendium_tests.rs"]
mod tests;
