//! User Story 3: changes made in the repository, brought back under a decision.
//!
//! # This is the module that removes the guarantee
//!
//! Everything else in `lore_sync` is unable to damage a world *by
//! construction*: it reads every lore table and writes none, so "a
//! synchronisation cannot alter lore" is a property of the code's shape rather
//! than of anyone's care. This module writes lore. The module docs upstairs
//! say that a change adding a write path has left the first delivery — this is
//! that change, arriving deliberately and with the guarantee it removes
//! replaced piece by piece rather than dropped.
//!
//! What replaces it:
//!
//! * **FR-022 is a type.** [`IncomingEnabled`] cannot be constructed from a
//!   connection that has not enabled incoming acceptance, and every function
//!   here that detects, records, or applies anything demands one. A world that
//!   never enabled it is not protected by a forgotten `if`; there is no
//!   spelling of the call that reaches it.
//! * **FR-023 is a column.** A detected change is a row in
//!   `lore_pending_incoming_changes` with `status = 'pending'`, and nothing
//!   reads a pending row as an instruction. [`accept`] is the only function in
//!   this module that writes to a lore table, and it takes a user id because
//!   there is no acceptance without an accepter.
//! * **FR-024 is an absence.** [`DetectedChange::Update`] carries the incoming
//!   text and the identity of the app-side revision — never a combination of
//!   the two. There is no merge function here, no "prefer theirs" flag, and no
//!   column in the table to put a merged body in. The reviewer chooses a whole
//!   text; the system never composes one.
//! * **FR-026 is a `kind`.** A file that disappeared becomes a
//!   [`DetectedChange::Deletion`] proposal, and accepting one returns
//!   [`Acceptance::DeletionConfirmed`] rather than deleting anything — see that
//!   variant for why the deletion itself is not performed here. Declining is
//!   reversed by the very next export pass, because the exported-entry record
//!   still names a file that is no longer on disk and `apply` writes it back.
//! * **FR-027 is a `HashMap` lookup and nothing else.** The only thing that
//!   associates a file with an entry is the durable identifier in its header.
//!   No function in this module reads a path or a title in order to decide
//!   what a file is about; paths appear only in what a human is shown.
//!
//! # Why detection is pure
//!
//! [`detect`] takes the repository's files and what the world believes it
//! exported, and returns proposals. No network, no clone, no database, no
//! clock. That is what lets "a world without incoming enabled is never
//! modified" and "a file with a stranger's front matter is never matched to an
//! existing entry" be tested as claims about a `Vec` rather than as claims
//! about an integration that somebody has to stand up first — the same
//! argument `plan.rs` makes for the outbound direction, and worth more here,
//! because here being wrong costs a world its words.
//!
//! # No inbound endpoint, ever
//!
//! FR-034a. Detection runs on the polling pass, from remote state that pass
//! already fetched. Nothing in this module is reachable from an HTTP route a
//! repository host could call, and nothing here should ever become so: an
//! endpoint that a third party can make fire is a write path into a world's
//! lore whose trigger the platform does not control.

use std::collections::{HashMap, HashSet};

use chrono::Utc;
use diesel::prelude::*;
use uuid::Uuid;

use crate::lore_sync::{document, paths};
use crate::models::{
    LorePendingIncomingChange, LoreRepositoryConnection, NewLoreLink, NewLoreRevision,
};
use crate::schema::{
    lore_pending_incoming_changes, world_lore_entries, world_lore_links, world_lore_revisions,
};

/// Proof that a connection has enabled acceptance of incoming changes.
///
/// FR-022 in the type system. Export must be usable without import, and a
/// world that never turned acceptance on must never be modified by anything in
/// its repository — so rather than every function here beginning with the same
/// `if connection.incoming_enabled` that one of them will eventually be
/// written without, the check happens once, in the only place that can make
/// one of these.
///
/// A deactivated connection (FR-041a) cannot produce one either. An
/// enforcement deactivation that a repository could still write through would
/// not be a deactivation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IncomingEnabled {
    connection_id: Uuid,
    world_id: Uuid,
}

impl IncomingEnabled {
    /// `None` unless this connection accepts incoming changes and is not
    /// deactivated. The fields are private and there is no other constructor:
    /// a caller cannot assemble one for a world that has not opted in.
    pub fn for_connection(connection: &LoreRepositoryConnection) -> Option<Self> {
        if !connection.incoming_enabled || connection.state == "deactivated" {
            return None;
        }
        Some(Self {
            connection_id: connection.id,
            world_id: connection.world_id,
        })
    }

    pub fn connection_id(&self) -> Uuid {
        self.connection_id
    }

    pub fn world_id(&self) -> Uuid {
        self.world_id
    }
}

/// A file as the polling pass found it in the working clone.
#[derive(Debug, Clone)]
pub struct RepositoryFile {
    /// Relative to the world's subtree. Shown to a reviewer; never matched on.
    pub path: String,
    /// The file's bytes as text, front matter included.
    pub contents: String,
}

/// What the world believes it exported for one entry, and what that entry says
/// now.
///
/// Assembled by the caller from `lore_exported_entries` joined to the entry and
/// its revisions. Both bodies are in **authored** form — the app's own link
/// syntax — because that is the form an incoming body is compared against and
/// eventually stored in.
#[derive(Debug, Clone)]
pub struct ExportedEntry {
    pub lore_entry_id: Uuid,
    pub title: String,
    /// Where we last wrote this entry's file, relative to the subtree.
    pub current_path: String,
    /// The revision the exported file was built from — the common ancestor.
    pub exported_revision_id: Option<Uuid>,
    /// The markdown that revision held.
    pub exported_body: String,
    /// The entry's current revision in the app.
    pub current_revision_id: Option<Uuid>,
    /// The markdown the entry holds now.
    pub current_body: String,
}

/// One thing the repository proposes.
///
/// A proposal, in every variant. Nothing in this enum has happened to a world.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetectedChange {
    /// A file matched to an entry by the identifier in its header, whose text
    /// differs from what we exported.
    Update {
        lore_entry_id: Uuid,
        repository_path: String,
        /// The repository's text, in authored form, whole. Never combined with
        /// the app's text (FR-024).
        incoming_body: String,
        base_revision_id: Option<Uuid>,
        app_revision_id: Option<Uuid>,
        /// FR-024: the entry moved on in the app too, so a reviewer is choosing
        /// between two texts rather than confirming one.
        also_changed_in_app: bool,
    },
    /// A file carrying no identifier this world recognises (FR-027).
    ///
    /// It names no entry, because it was matched to none. The title is a
    /// suggestion for the entry that would be created, taken from the file's
    /// own header or its filename — a label on a proposal, not a match.
    NewEntry {
        repository_path: String,
        proposed_title: String,
        incoming_body: String,
    },
    /// An entry whose file is no longer in the repository (FR-026).
    ///
    /// A proposal to delete. Never a deletion.
    Deletion {
        lore_entry_id: Uuid,
        repository_path: String,
    },
}

/// Compare a repository against a world, and say what the repository proposes.
///
/// Pure: no clock, no network, no database. Given the same inputs it returns
/// the same proposals, which is what makes the rules below testable as
/// arithmetic rather than as integration.
///
/// `excluded_from_export` is the set of entries the export plan is currently
/// withholding — moderation-disabled ones (FR-015). A file bearing one of
/// those identifiers produces **nothing at all**: not an update, because the
/// entry is disabled; not a proposed new entry, because that would let text a
/// takedown removed re-enter the world under a fresh id; and not a deletion,
/// because its file's absence is our own doing rather than the user's.
///
/// `_gate` is unread on purpose. It is not a parameter this function consults,
/// it is a parameter this function *requires*, so that FR-022 is enforced by
/// the signature and cannot be forgotten at a call site.
/// The entries a takedown is currently withholding from export.
///
/// # Why this is a type rather than a slice
///
/// Because the slice version had a hole with a very bad shape. `detect` needs
/// to know which entries moderation is withholding, or a disabled entry's file
/// — still sitting in the repository, because the platform cannot delete what
/// it does not control — comes back as a *proposed new entry* and a Game
/// Master can accept the takedown straight back into their world.
///
/// As a `&[Uuid]` that safety depended on every caller remembering to fill it,
/// and `&[]` compiled. This is the one property in this module that a caller
/// could silently get wrong, and the consequence is undoing a legal obligation
/// — so it is worth a type.
///
/// The only constructor asks moderation. There is no `empty()`, and a caller
/// that has no database connection cannot produce one at all.
pub struct WithheldByModeration(HashSet<Uuid>);

impl WithheldByModeration {
    /// Ask moderation which of a world's entries are disabled right now.
    ///
    /// Deliberately re-asked per detection rather than cached: a takedown that
    /// lands between two passes must take effect on the next one, and a cached
    /// answer would let a disabled entry be proposed for however long the cache
    /// lived.
    pub async fn for_world(state: &crate::AppState, world_id: Uuid) -> Result<Self, IncomingError> {
        use crate::schema::world_lore_entries;

        let mut conn = state.db_pool.get().map_err(|e| {
            IncomingError::Database(diesel::result::Error::QueryBuilderError(
                e.to_string().into(),
            ))
        })?;

        let ids: Vec<Uuid> = world_lore_entries::table
            .filter(world_lore_entries::world_id.eq(world_id))
            .select(world_lore_entries::id)
            .load(&mut conn)
            .map_err(IncomingError::Database)?;

        let visible = crate::moderation::filter_visible(state, "lore_entry", ids.clone(), |id| *id)
            .await
            .map_err(|_| {
                IncomingError::Database(diesel::result::Error::QueryBuilderError(
                    "moderation filter failed".into(),
                ))
            })?;

        let visible: HashSet<Uuid> = visible.into_iter().collect();
        Ok(Self(
            ids.into_iter().filter(|id| !visible.contains(id)).collect(),
        ))
    }

    /// For tests that are about detection rather than about moderation.
    ///
    /// `#[cfg(test)]` on purpose: in a build that ships, the only way to get
    /// one of these is to ask moderation.
    #[cfg(test)]
    pub fn exactly(ids: &[Uuid]) -> Self {
        Self(ids.iter().copied().collect())
    }

    fn contains(&self, id: &Uuid) -> bool {
        self.0.contains(id)
    }
}

pub fn detect(
    _gate: &IncomingEnabled,
    files: &[RepositoryFile],
    exported: &[ExportedEntry],
    excluded_from_export: &WithheldByModeration,
) -> Vec<DetectedChange> {
    let excluded = excluded_from_export;

    // The ONLY association between a file and an entry (FR-027). There is
    // deliberately no map keyed by path and none keyed by title; a reader
    // checking that FR-027 holds should be able to check it by looking for
    // such a map and not finding one.
    let by_id: HashMap<Uuid, &ExportedEntry> = exported
        .iter()
        .map(|entry| (entry.lore_entry_id, entry))
        .collect();

    let mut accounted_for: HashSet<Uuid> = HashSet::new();
    let mut changes = Vec::new();

    for file in files {
        let parsed = match document::parse(&file.contents) {
            Ok(parsed) => parsed,
            Err(_) => {
                // No readable front matter is FR-027's central case: a Game
                // Master wrote a new file in their editor. It is offered as a
                // new entry, and no attempt whatever is made to guess which
                // existing entry it "really" is.
                //
                // The body is the whole file, front matter attempt and all: a
                // header we could not parse is prose the author wrote, and
                // eating it would lose their words to a parser's opinion.
                changes.push(DetectedChange::NewEntry {
                    repository_path: file.path.clone(),
                    proposed_title: title_from_filename(&file.path),
                    incoming_body: file.contents.clone(),
                });
                continue;
            }
        };

        if excluded.contains(&parsed.header.id) {
            continue;
        }

        let Some(entry) = by_id.get(&parsed.header.id) else {
            // An identifier we do not recognise is no identifier at all. It may
            // be another world's file, or an id a user invented; either way it
            // matches nothing here, and a file that matches nothing is a
            // proposal for a new entry.
            let title = parsed.header.title.trim();
            changes.push(DetectedChange::NewEntry {
                repository_path: file.path.clone(),
                proposed_title: if title.is_empty() {
                    title_from_filename(&file.path)
                } else {
                    title.to_string()
                },
                incoming_body: restore_authored_links(&parsed.body, &file.path, exported),
            });
            continue;
        };

        accounted_for.insert(entry.lore_entry_id);

        let incoming = restore_authored_links(&parsed.body, &file.path, exported);

        // Two ways for a file to propose nothing: it still holds what we wrote,
        // or it already holds what the app now says. The second matters because
        // a pass that exported an edit and then re-read it must not offer the
        // Game Master their own edit back.
        if incoming == entry.exported_body || incoming == entry.current_body {
            continue;
        }

        changes.push(DetectedChange::Update {
            lore_entry_id: entry.lore_entry_id,
            repository_path: file.path.clone(),
            incoming_body: incoming,
            base_revision_id: entry.exported_revision_id,
            app_revision_id: entry.current_revision_id,
            // FR-024. Answered from recorded revisions, not from timestamps:
            // the entry has moved on in the app exactly when its current
            // revision is not the one the file was built from.
            also_changed_in_app: entry.current_revision_id != entry.exported_revision_id,
        });
    }

    for entry in exported {
        if accounted_for.contains(&entry.lore_entry_id) || excluded.contains(&entry.lore_entry_id) {
            continue;
        }
        changes.push(DetectedChange::Deletion {
            lore_entry_id: entry.lore_entry_id,
            repository_path: entry.current_path.clone(),
        });
    }

    changes
}

/// A readable name for a file that carries none.
///
/// This is **not** matching by filename: nothing is looked up with it. It is
/// the placeholder title of an entry that does not exist yet, so a reviewer
/// sees "The Salt Road" rather than a path, and it is theirs to change.
fn title_from_filename(path: &str) -> String {
    let leaf = path.rsplit('/').next().unwrap_or(path);
    let stem = leaf.strip_suffix(".md").unwrap_or(leaf);
    if stem.trim().is_empty() {
        "Untitled".to_string()
    } else {
        stem.to_string()
    }
}

/// Repository link form back to authored form.
///
/// The inverse of what export did, computed the same way export computed it —
/// from each exported entry's recorded path — so that a round trip with no
/// edit produces byte-identical markdown (SC-008) and therefore proposes
/// nothing. A destination that is not one of this world's exported files is
/// left exactly as the author wrote it.
fn restore_authored_links(body: &str, self_path: &str, exported: &[ExportedEntry]) -> String {
    let mut by_destination: HashMap<String, String> = HashMap::new();
    for entry in exported {
        if entry.current_path == self_path {
            continue;
        }
        by_destination.insert(
            paths::relative_link(self_path, &entry.current_path),
            entry.title.clone(),
        );
    }

    document::restore_links(body, |destination| by_destination.get(destination).cloned())
}

/// Why an acceptance or a refusal could not be carried out.
#[derive(Debug)]
pub enum IncomingError {
    /// No such undecided change for this connection. Covers a change already
    /// decided, one belonging to another connection, and one that never
    /// existed — deliberately one variant, because distinguishing them for a
    /// caller who supplied an id they should not have would be answering a
    /// question about another world's data.
    NotPending,
    /// The row is internally inconsistent — a kind whose required field is
    /// absent. The database's CHECK constraints make this unreachable through
    /// this module; it exists so that a row written by something else fails
    /// loudly rather than being interpreted generously.
    Malformed(&'static str),
    Database(diesel::result::Error),
}

impl std::fmt::Display for IncomingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotPending => write!(formatter, "no such pending incoming change"),
            Self::Malformed(what) => write!(formatter, "incoming change is malformed: {what}"),
            Self::Database(error) => write!(formatter, "database error: {error}"),
        }
    }
}

impl std::error::Error for IncomingError {}

impl From<diesel::result::Error> for IncomingError {
    fn from(error: diesel::result::Error) -> Self {
        Self::Database(error)
    }
}

/// Persist detected proposals for review, replacing any earlier undecided
/// proposal for the same entry.
///
/// Replacing rather than appending is what the partial unique indexes in the
/// migration require, and the reason is a user-facing one: two pending rows for
/// one entry is two accept buttons for one entry, and pressing both writes the
/// older text last.
///
/// A decided row is never touched. The history of what the repository has
/// proposed for an entry, and what was done about it, is the audit trail
/// FR-023 is worth having.
pub fn record(
    conn: &mut PgConnection,
    gate: &IncomingEnabled,
    changes: &[DetectedChange],
) -> Result<Vec<Uuid>, IncomingError> {
    let now = Utc::now().naive_utc();
    let mut ids = Vec::with_capacity(changes.len());

    for change in changes {
        let row = row_for(gate, change, now);

        // Matched by entry, or — for a proposal that names no entry — by the
        // file it came from, which is the only identity a thing that does not
        // exist yet has.
        let existing: Option<Uuid> = match row.lore_entry_id {
            Some(entry_id) => lore_pending_incoming_changes::table
                .filter(lore_pending_incoming_changes::connection_id.eq(gate.connection_id))
                .filter(lore_pending_incoming_changes::status.eq(STATUS_PENDING))
                .filter(lore_pending_incoming_changes::lore_entry_id.eq(entry_id))
                .select(lore_pending_incoming_changes::id)
                .first(conn)
                .optional()?,
            None => lore_pending_incoming_changes::table
                .filter(lore_pending_incoming_changes::connection_id.eq(gate.connection_id))
                .filter(lore_pending_incoming_changes::status.eq(STATUS_PENDING))
                .filter(lore_pending_incoming_changes::lore_entry_id.is_null())
                .filter(lore_pending_incoming_changes::repository_path.eq(&row.repository_path))
                .select(lore_pending_incoming_changes::id)
                .first(conn)
                .optional()?,
        };

        match existing {
            Some(id) => {
                diesel::update(
                    lore_pending_incoming_changes::table
                        .filter(lore_pending_incoming_changes::id.eq(id)),
                )
                .set((
                    lore_pending_incoming_changes::kind.eq(&row.kind),
                    lore_pending_incoming_changes::repository_path.eq(&row.repository_path),
                    lore_pending_incoming_changes::proposed_title.eq(&row.proposed_title),
                    lore_pending_incoming_changes::incoming_body.eq(&row.incoming_body),
                    lore_pending_incoming_changes::base_revision_id.eq(row.base_revision_id),
                    lore_pending_incoming_changes::app_revision_id.eq(row.app_revision_id),
                    lore_pending_incoming_changes::also_changed_in_app.eq(row.also_changed_in_app),
                    lore_pending_incoming_changes::detected_at.eq(now),
                ))
                .execute(conn)?;
                ids.push(id);
            }
            None => {
                let id = row.id;
                diesel::insert_into(lore_pending_incoming_changes::table)
                    .values(row)
                    .execute(conn)?;
                ids.push(id);
            }
        }
    }

    Ok(ids)
}

const STATUS_PENDING: &str = "pending";
const STATUS_ACCEPTED: &str = "accepted";
const STATUS_DECLINED: &str = "declined";

const KIND_UPDATE: &str = "update";
const KIND_NEW_ENTRY: &str = "new_entry";
const KIND_DELETION: &str = "deletion";

fn row_for(
    gate: &IncomingEnabled,
    change: &DetectedChange,
    now: chrono::NaiveDateTime,
) -> LorePendingIncomingChange {
    let base = LorePendingIncomingChange {
        id: Uuid::now_v7(),
        connection_id: gate.connection_id,
        lore_entry_id: None,
        kind: KIND_UPDATE.to_string(),
        repository_path: String::new(),
        proposed_title: None,
        incoming_body: None,
        base_revision_id: None,
        app_revision_id: None,
        also_changed_in_app: false,
        status: STATUS_PENDING.to_string(),
        detected_at: now,
        decided_at: None,
        decided_by: None,
        applied_revision_id: None,
        created_entry_id: None,
    };

    match change {
        DetectedChange::Update {
            lore_entry_id,
            repository_path,
            incoming_body,
            base_revision_id,
            app_revision_id,
            also_changed_in_app,
        } => LorePendingIncomingChange {
            lore_entry_id: Some(*lore_entry_id),
            kind: KIND_UPDATE.to_string(),
            repository_path: repository_path.clone(),
            incoming_body: Some(incoming_body.clone()),
            base_revision_id: *base_revision_id,
            app_revision_id: *app_revision_id,
            also_changed_in_app: *also_changed_in_app,
            ..base
        },
        DetectedChange::NewEntry {
            repository_path,
            proposed_title,
            incoming_body,
        } => LorePendingIncomingChange {
            lore_entry_id: None,
            kind: KIND_NEW_ENTRY.to_string(),
            repository_path: repository_path.clone(),
            proposed_title: Some(proposed_title.clone()),
            incoming_body: Some(incoming_body.clone()),
            ..base
        },
        DetectedChange::Deletion {
            lore_entry_id,
            repository_path,
        } => LorePendingIncomingChange {
            lore_entry_id: Some(*lore_entry_id),
            kind: KIND_DELETION.to_string(),
            repository_path: repository_path.clone(),
            incoming_body: None,
            ..base
        },
    }
}

/// Everything awaiting a decision for one connection, newest first.
///
/// The review surface's read. It returns rows and nothing else — reading this
/// list has no effect on a world, which is the property that lets it be called
/// from anywhere without thinking about it.
pub fn pending(
    conn: &mut PgConnection,
    gate: &IncomingEnabled,
) -> Result<Vec<LorePendingIncomingChange>, IncomingError> {
    Ok(lore_pending_incoming_changes::table
        .filter(lore_pending_incoming_changes::connection_id.eq(gate.connection_id))
        .filter(lore_pending_incoming_changes::status.eq(STATUS_PENDING))
        .order(lore_pending_incoming_changes::detected_at.desc())
        .select(LorePendingIncomingChange::as_select())
        .load(conn)?)
}

/// What accepting a change did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Acceptance {
    /// An existing entry gained an ordinary revision carrying the incoming
    /// text (FR-025).
    Updated {
        lore_entry_id: Uuid,
        revision_id: Uuid,
    },
    /// A proposed new entry became an entry (FR-027).
    Created {
        lore_entry_id: Uuid,
        revision_id: Uuid,
    },
    /// A deletion was confirmed, and **nothing has been deleted yet**.
    ///
    /// FR-026 is satisfied at the point the confirmation is recorded; the
    /// deletion itself is left to the world's own lore-deletion path, which
    /// re-parents the entry's children to their grandparent rather than
    /// orphaning them (031/T072) and cleans up permissions, links and images.
    /// Reimplementing that here would be a second definition of what deleting
    /// a lore entry means, and the two would drift — with the copy that drifts
    /// being the one nobody uses day to day.
    ///
    /// The caller is therefore obliged to perform the deletion. It is a
    /// deliberate obligation rather than an oversight: a mutation that
    /// confirms and does not delete leaves an entry the user asked to remove,
    /// which is visible and recoverable, where the alternative — this module
    /// growing its own deletion — is a second way to destroy lore.
    DeletionConfirmed { lore_entry_id: Uuid },
}

/// Apply an accepted change as an ordinary revision, attributed to the
/// accepting user (FR-025).
///
/// "Ordinary" is the requirement and it is meant literally: the row goes into
/// `world_lore_revisions` alongside every revision a person typed, so the
/// entry's history stays complete, restoring to it works, and nothing that
/// reads lore history has to learn about repositories. What marks it as having
/// come from the repository is `applied_revision_id` on the accepted proposal
/// pointing back at it — see the migration for why that mark lives there and
/// not as a column on the revisions table.
///
/// The whole of it is one transaction. A revision written without the proposal
/// being marked accepted would be a world change with no record of who agreed
/// to it, and the proposal would still be offered — twice-accepting the same
/// text.
pub fn accept(
    conn: &mut PgConnection,
    gate: &IncomingEnabled,
    change_id: Uuid,
    accepting_user: Uuid,
) -> Result<Acceptance, IncomingError> {
    conn.transaction(|conn| {
        // Scoped to the gate's connection, so an id from another world's
        // review surface finds nothing. Locked for the duration, so two
        // reviewers pressing accept at the same moment produce one revision
        // rather than two.
        let change: LorePendingIncomingChange = lore_pending_incoming_changes::table
            .filter(lore_pending_incoming_changes::id.eq(change_id))
            .filter(lore_pending_incoming_changes::connection_id.eq(gate.connection_id))
            .filter(lore_pending_incoming_changes::status.eq(STATUS_PENDING))
            .select(LorePendingIncomingChange::as_select())
            .for_update()
            .first(conn)
            .optional()?
            .ok_or(IncomingError::NotPending)?;

        let now = Utc::now().naive_utc();

        match change.kind.as_str() {
            KIND_DELETION => {
                let lore_entry_id = change
                    .lore_entry_id
                    .ok_or(IncomingError::Malformed("a deletion names no entry"))?;
                decide(conn, change.id, STATUS_ACCEPTED, accepting_user, now)?;
                Ok(Acceptance::DeletionConfirmed { lore_entry_id })
            }
            KIND_UPDATE => {
                let lore_entry_id = change
                    .lore_entry_id
                    .ok_or(IncomingError::Malformed("an update names no entry"))?;
                let body = change
                    .incoming_body
                    .clone()
                    .ok_or(IncomingError::Malformed("an update carries no text"))?;

                let revision_id = write_revision(conn, lore_entry_id, &body, accepting_user, now)?;

                diesel::update(
                    world_lore_entries::table.filter(world_lore_entries::id.eq(lore_entry_id)),
                )
                .set((
                    world_lore_entries::content.eq(&body),
                    world_lore_entries::current_revision_id.eq(revision_id),
                    world_lore_entries::updated_at.eq(now),
                ))
                .execute(conn)?;

                reindex_links(conn, gate.world_id, lore_entry_id, &body)?;
                decide(conn, change.id, STATUS_ACCEPTED, accepting_user, now)?;
                diesel::update(
                    lore_pending_incoming_changes::table
                        .filter(lore_pending_incoming_changes::id.eq(change.id)),
                )
                .set(lore_pending_incoming_changes::applied_revision_id.eq(revision_id))
                .execute(conn)?;

                Ok(Acceptance::Updated {
                    lore_entry_id,
                    revision_id,
                })
            }
            KIND_NEW_ENTRY => {
                let body = change
                    .incoming_body
                    .clone()
                    .ok_or(IncomingError::Malformed("a new entry carries no text"))?;
                let title = change
                    .proposed_title
                    .clone()
                    .unwrap_or_else(|| "Untitled".to_string());

                // A brand-new id. The identifier the file carried, if it
                // carried one, is not reused: it named nothing here, and
                // adopting a stranger's id would let a file choose what an
                // entry in this world is called from the outside.
                let lore_entry_id = Uuid::now_v7();
                let slug = crate::markdown::slug::unique_slug_for_world(
                    conn,
                    gate.world_id,
                    &title,
                    None,
                )?;

                diesel::insert_into(world_lore_entries::table)
                    .values((
                        world_lore_entries::id.eq(lore_entry_id),
                        world_lore_entries::world_id.eq(gate.world_id),
                        world_lore_entries::title.eq(&title),
                        world_lore_entries::slug.eq(&slug),
                        world_lore_entries::content.eq(&body),
                        // The accepting user, not the repository and not the
                        // connection's owner. Somebody in this world took
                        // responsibility for this text existing here.
                        world_lore_entries::created_by.eq(accepting_user),
                        world_lore_entries::created_at.eq(now),
                        world_lore_entries::updated_at.eq(now),
                    ))
                    .execute(conn)?;

                let revision_id = write_revision(conn, lore_entry_id, &body, accepting_user, now)?;
                diesel::update(
                    world_lore_entries::table.filter(world_lore_entries::id.eq(lore_entry_id)),
                )
                .set(world_lore_entries::current_revision_id.eq(revision_id))
                .execute(conn)?;

                reindex_links(conn, gate.world_id, lore_entry_id, &body)?;
                decide(conn, change.id, STATUS_ACCEPTED, accepting_user, now)?;
                diesel::update(
                    lore_pending_incoming_changes::table
                        .filter(lore_pending_incoming_changes::id.eq(change.id)),
                )
                .set((
                    lore_pending_incoming_changes::applied_revision_id.eq(revision_id),
                    lore_pending_incoming_changes::created_entry_id.eq(lore_entry_id),
                ))
                .execute(conn)?;

                Ok(Acceptance::Created {
                    lore_entry_id,
                    revision_id,
                })
            }
            _ => Err(IncomingError::Malformed("unknown kind")),
        }
    })
}

/// Refuse a proposed change.
///
/// Writes nothing to any lore table — that is the whole of what declining
/// means, and it is why this function is three lines. FR-026's "a declined
/// deletion MUST be reversed on the next synchronisation" needs no code here
/// either: the entry is still in the world, so the next export plan still
/// contains its file, the exported-entry record still names the path, and
/// `apply` writes a file that is missing. The reversal is the ordinary export
/// pass doing its ordinary job, which is a far better guarantee than a special
/// case that has to remember.
pub fn decline(
    conn: &mut PgConnection,
    gate: &IncomingEnabled,
    change_id: Uuid,
    declining_user: Uuid,
) -> Result<(), IncomingError> {
    let now = Utc::now().naive_utc();
    let affected = diesel::update(
        lore_pending_incoming_changes::table
            .filter(lore_pending_incoming_changes::id.eq(change_id))
            .filter(lore_pending_incoming_changes::connection_id.eq(gate.connection_id))
            .filter(lore_pending_incoming_changes::status.eq(STATUS_PENDING)),
    )
    .set((
        lore_pending_incoming_changes::status.eq(STATUS_DECLINED),
        lore_pending_incoming_changes::decided_at.eq(now),
        lore_pending_incoming_changes::decided_by.eq(declining_user),
    ))
    .execute(conn)?;

    if affected == 0 {
        return Err(IncomingError::NotPending);
    }
    Ok(())
}

/// Whether a revision came from a repository (FR-025's mark, read back).
///
/// One hop, because the mark is a row pointing at the revision rather than a
/// column on it. Anything rendering lore history can ask this; nothing has to
/// know how synchronisation works to do so.
pub fn is_repository_originated(
    conn: &mut PgConnection,
    revision_id: Uuid,
) -> Result<bool, IncomingError> {
    let count: i64 = lore_pending_incoming_changes::table
        .filter(lore_pending_incoming_changes::applied_revision_id.eq(revision_id))
        .count()
        .get_result(conn)?;
    Ok(count > 0)
}

fn decide(
    conn: &mut PgConnection,
    change_id: Uuid,
    status: &str,
    user: Uuid,
    now: chrono::NaiveDateTime,
) -> Result<(), diesel::result::Error> {
    diesel::update(
        lore_pending_incoming_changes::table
            .filter(lore_pending_incoming_changes::id.eq(change_id)),
    )
    .set((
        lore_pending_incoming_changes::status.eq(status),
        lore_pending_incoming_changes::decided_at.eq(now),
        lore_pending_incoming_changes::decided_by.eq(user),
    ))
    .execute(conn)?;
    Ok(())
}

/// An ordinary revision row (FR-025). No repository column, no special kind.
///
/// `restored_from_revision_id` is `None`: this is not a restore of an earlier
/// revision of this entry, it is new text that arrived by another route, and
/// claiming otherwise would make the history say something untrue.
fn write_revision(
    conn: &mut PgConnection,
    lore_entry_id: Uuid,
    body: &str,
    author_id: Uuid,
    now: chrono::NaiveDateTime,
) -> Result<Uuid, diesel::result::Error> {
    let revision_id = Uuid::now_v7();
    diesel::insert_into(world_lore_revisions::table)
        .values(NewLoreRevision {
            id: revision_id,
            lore_entry_id,
            content_markdown: body.to_string(),
            author_id,
            restored_from_revision_id: None,
            created_at: now,
        })
        .execute(conn)?;
    Ok(revision_id)
}

/// Rebuild the entry's outgoing cross-link index for the accepted text.
///
/// `world_lore_links` is a canonical index driving backlinks, so text that
/// arrived this way must be indexed exactly as text typed in the app is; an
/// entry whose backlinks are missing only when its last edit came from the
/// repository is a bug nobody would connect to synchronisation.
///
/// This repeats `mutations_lore::replace_lore_links`, which is private to that
/// module. The repetition is deliberate and narrow — delete this entry's rows,
/// insert the freshly resolved set — rather than widening a mutation module's
/// surface for one caller; the resolution itself, which is the part with rules
/// in it, is `markdown::links::extract_and_resolve` and is shared.
///
/// `viewer_is_dm` is `true` for the same reason the app's own save path passes
/// `true`: this is a canonical index, not a per-viewer view.
fn reindex_links(
    conn: &mut PgConnection,
    world_id: Uuid,
    lore_entry_id: Uuid,
    body: &str,
) -> Result<(), diesel::result::Error> {
    let (_, links) = crate::markdown::links::extract_and_resolve(conn, world_id, body, true)?;

    diesel::delete(
        world_lore_links::table.filter(world_lore_links::source_lore_entry_id.eq(lore_entry_id)),
    )
    .execute(conn)?;

    for link in links {
        diesel::insert_into(world_lore_links::table)
            .values(NewLoreLink {
                id: Uuid::now_v7(),
                source_lore_entry_id: lore_entry_id,
                raw_title: link.raw_title.clone(),
                target_kind: link.target_kind.to_string(),
                target_lore_entry_id: link.target_lore_entry_id,
                target_actor_id: link.target_actor_id,
                target_item_id: link.target_item_id,
                target_ability_id: link.target_ability_id,
            })
            .execute(conn)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lore_sync::document::DocumentHeader;
    use crate::test_support::{
        insert_test_lore_entry, insert_test_user, insert_test_world, test_app_state,
    };
    use chrono::{TimeZone, Utc};

    // ---------------------------------------------------------------------
    // Fixtures
    // ---------------------------------------------------------------------

    /// A connection row. The repository reference is unique per run because
    /// FR-033's constraint is instance-wide: a fixed name passes once and fails
    /// for the rest of the database's life.
    fn connection(world_id: Uuid, owner: Uuid, incoming_enabled: bool) -> LoreRepositoryConnection {
        let now = Utc::now().naive_utc();
        LoreRepositoryConnection {
            id: Uuid::now_v7(),
            world_id,
            host_kind: "test".to_string(),
            installation_ref: "test-installation".to_string(),
            repository_ref: format!("owner/{}", Uuid::now_v7()),
            branch: "main".to_string(),
            directory: format!("lore-{}", Uuid::now_v7().simple()),
            incoming_enabled,
            notice_acknowledged_at: Some(now),
            state: "working".to_string(),
            state_reason: None,
            repository_is_public: Some(false),
            visibility_checked_at: Some(now),
            deactivated_at: None,
            deactivated_reason: None,
            last_synced_at: Some(now),
            last_written_commit: None,
            created_by: owner,
            updated_by: owner,
            created_at: now,
            updated_at: now,
        }
    }

    fn exported(
        lore_entry_id: Uuid,
        title: &str,
        path: &str,
        body: &str,
        revision: Option<Uuid>,
    ) -> ExportedEntry {
        ExportedEntry {
            lore_entry_id,
            title: title.to_string(),
            current_path: path.to_string(),
            exported_revision_id: revision,
            exported_body: body.to_string(),
            current_revision_id: revision,
            current_body: body.to_string(),
        }
    }

    /// A file as export would have written it.
    fn file_for(entry: &ExportedEntry, id: Uuid, body: &str) -> RepositoryFile {
        let header = DocumentHeader {
            id,
            title: entry.title.clone(),
            tags: Vec::new(),
            updated: Utc.with_ymd_and_hms(2026, 9, 4, 12, 0, 0).unwrap(),
            unresolvable_links: Vec::new(),
        };
        RepositoryFile {
            path: entry.current_path.clone(),
            contents: document::render(&header, body),
        }
    }

    fn gate_for(connection: &LoreRepositoryConnection) -> IncomingEnabled {
        IncomingEnabled::for_connection(connection).expect("incoming acceptance is enabled")
    }

    // ---------------------------------------------------------------------
    // FR-022 — the rule that keeps every existing world as safe as yesterday
    // ---------------------------------------------------------------------

    /// FR-022, and the most important test in this file.
    ///
    /// A world that never enabled incoming acceptance must never be modified by
    /// anything in its repository. This asserts the gate refuses, but the real
    /// guarantee is stronger and is not expressible as an assertion: `detect`,
    /// `record`, `accept`, `decline` and `pending` all take an
    /// `&IncomingEnabled`, whose fields are private and whose only constructor
    /// is the one exercised here. There is no call, in this crate or any
    /// future one, that reaches a lore write for such a world — not because
    /// every path remembers to check, but because no such path compiles.
    #[test]
    fn a_connection_without_incoming_enabled_yields_no_gate() {
        let world = Uuid::now_v7();
        let owner = Uuid::now_v7();
        let connection = connection(world, owner, false);

        assert!(
            IncomingEnabled::for_connection(&connection).is_none(),
            "a world that never enabled incoming acceptance produced a gate — FR-022 is broken",
        );
    }

    /// FR-041a. An enforcement deactivation a repository could still write
    /// through would not be a deactivation.
    #[test]
    fn a_deactivated_connection_yields_no_gate() {
        let mut connection = connection(Uuid::now_v7(), Uuid::now_v7(), true);
        connection.state = "deactivated".to_string();
        connection.deactivated_at = Some(Utc::now().naive_utc());

        assert!(
            IncomingEnabled::for_connection(&connection).is_none(),
            "a deactivated connection produced a gate",
        );
    }

    /// The other half: a world that DID enable it gets a gate naming the right
    /// connection and world. A gate that never opens would pass the test above
    /// and make the feature dead.
    #[test]
    fn an_enabled_connection_yields_a_gate_naming_its_world() {
        let world = Uuid::now_v7();
        let connection = connection(world, Uuid::now_v7(), true);
        let gate = gate_for(&connection);

        assert_eq!(gate.connection_id(), connection.id);
        assert_eq!(gate.world_id(), world);
    }

    // ---------------------------------------------------------------------
    // Detection — pure, no database, no clone
    // ---------------------------------------------------------------------

    /// SC-008's round trip, from detection's side: export a file, change
    /// nothing, and there is nothing to propose. A detector that proposed the
    /// world's own text back at it would bury real changes in noise.
    #[test]
    fn an_unchanged_file_proposes_nothing() {
        let gate = gate_for(&connection(Uuid::now_v7(), Uuid::now_v7(), true));
        let entry = exported(
            Uuid::now_v7(),
            "The Red Keep",
            "westeros/the-red-keep.md",
            "A castle above the bay.",
            Some(Uuid::now_v7()),
        );
        let file = file_for(&entry, entry.lore_entry_id, &entry.exported_body);

        assert_eq!(
            detect(
                &gate,
                &[file],
                &[entry],
                &WithheldByModeration::exactly(&[])
            ),
            Vec::new()
        );
    }

    /// FR-027 and T058. A file a Game Master wrote in their editor carries no
    /// front matter at all, so it is offered as a new entry — and its text is
    /// carried whole, including whatever it has at the top, because a parser
    /// that could not read a header has no standing to delete it.
    #[test]
    fn a_file_with_no_front_matter_is_a_proposed_new_entry() {
        let gate = gate_for(&connection(Uuid::now_v7(), Uuid::now_v7(), true));
        let file = RepositoryFile {
            path: "westeros/the-salt-road.md".to_string(),
            contents: "# The Salt Road\n\nIt runs east.".to_string(),
        };

        assert_eq!(
            detect(&gate, &[file], &[], &WithheldByModeration::exactly(&[])),
            vec![DetectedChange::NewEntry {
                repository_path: "westeros/the-salt-road.md".to_string(),
                proposed_title: "the-salt-road".to_string(),
                incoming_body: "# The Salt Road\n\nIt runs east.".to_string(),
            }],
        );
    }

    /// FR-027, at its sharpest. The file sits at exactly the path of an
    /// existing entry and carries exactly its title, and still is not matched
    /// to it — because the identifier in its header is not one this world
    /// knows.
    ///
    /// The existing entry is reported as a proposed deletion in the same pass,
    /// which is the honest reading: from the repository's side its file is
    /// gone and a different file has taken the name. Both are proposals, and a
    /// human sees both.
    #[test]
    fn a_file_is_never_matched_to_an_entry_by_path_or_title() {
        let gate = gate_for(&connection(Uuid::now_v7(), Uuid::now_v7(), true));
        let entry = exported(
            Uuid::now_v7(),
            "The Red Keep",
            "westeros/the-red-keep.md",
            "A castle above the bay.",
            Some(Uuid::now_v7()),
        );
        // Same path, same title, a stranger's id.
        let file = file_for(&entry, Uuid::now_v7(), "Somebody else's castle.");

        let changes = detect(
            &gate,
            &[file],
            std::slice::from_ref(&entry),
            &WithheldByModeration::exactly(&[]),
        );

        assert!(
            !changes
                .iter()
                .any(|change| matches!(change, DetectedChange::Update { .. })),
            "a file was matched to an entry by path or title — FR-027 is broken: {changes:?}",
        );
        assert!(changes.contains(&DetectedChange::NewEntry {
            repository_path: "westeros/the-red-keep.md".to_string(),
            proposed_title: "The Red Keep".to_string(),
            incoming_body: "Somebody else's castle.".to_string(),
        }));
        assert!(changes.contains(&DetectedChange::Deletion {
            lore_entry_id: entry.lore_entry_id,
            repository_path: "westeros/the-red-keep.md".to_string(),
        }));
    }

    /// FR-024. Both sides moved, so the change is flagged as such — and the
    /// proposal carries the repository's text WHOLE, with no trace of the
    /// app's. There is nothing here that could have merged them, which is the
    /// point: the reviewer is choosing between two texts, not reviewing a
    /// third that nobody wrote.
    #[test]
    fn an_entry_changed_on_both_sides_is_presented_whole_and_never_merged() {
        let gate = gate_for(&connection(Uuid::now_v7(), Uuid::now_v7(), true));
        let base_revision = Uuid::now_v7();
        let app_revision = Uuid::now_v7();
        let mut entry = exported(
            Uuid::now_v7(),
            "The Red Keep",
            "westeros/the-red-keep.md",
            "A castle above the bay.",
            Some(base_revision),
        );
        // The app moved on.
        entry.current_revision_id = Some(app_revision);
        entry.current_body = "A castle above the bay, and a sept below.".to_string();

        let file = file_for(
            &entry,
            entry.lore_entry_id,
            "A castle above the bay, built by Maegor.",
        );

        let changes = detect(
            &gate,
            &[file],
            std::slice::from_ref(&entry),
            &WithheldByModeration::exactly(&[]),
        );

        assert_eq!(
            changes,
            vec![DetectedChange::Update {
                lore_entry_id: entry.lore_entry_id,
                repository_path: "westeros/the-red-keep.md".to_string(),
                incoming_body: "A castle above the bay, built by Maegor.".to_string(),
                base_revision_id: Some(base_revision),
                app_revision_id: Some(app_revision),
                also_changed_in_app: true,
            }],
        );

        let DetectedChange::Update { incoming_body, .. } = &changes[0] else {
            unreachable!("asserted above");
        };
        assert!(
            !incoming_body.contains("sept"),
            "the proposal carried text from the app as well as the repository — \
             something merged prose, which FR-024 forbids at any priority",
        );
    }

    /// The other side of FR-024: a change only the repository made is not
    /// dressed up as a conflict. Everything being a conflict trains a reviewer
    /// to stop reading the warning.
    #[test]
    fn a_change_on_one_side_only_is_not_flagged_as_a_conflict() {
        let gate = gate_for(&connection(Uuid::now_v7(), Uuid::now_v7(), true));
        let entry = exported(
            Uuid::now_v7(),
            "The Red Keep",
            "westeros/the-red-keep.md",
            "A castle above the bay.",
            Some(Uuid::now_v7()),
        );
        let file = file_for(&entry, entry.lore_entry_id, "A castle, and a bay.");

        let changes = detect(
            &gate,
            &[file],
            &[entry],
            &WithheldByModeration::exactly(&[]),
        );
        let DetectedChange::Update {
            also_changed_in_app,
            ..
        } = &changes[0]
        else {
            panic!("expected an update, got {changes:?}");
        };
        assert!(!also_changed_in_app);
    }

    /// FR-026. A file that is gone produces a PROPOSAL and nothing else. There
    /// is no variant of `DetectedChange` that deletes, and detection has no
    /// database handle with which to.
    #[test]
    fn a_missing_file_is_a_proposed_deletion_and_nothing_more() {
        let gate = gate_for(&connection(Uuid::now_v7(), Uuid::now_v7(), true));
        let entry = exported(
            Uuid::now_v7(),
            "The Red Keep",
            "westeros/the-red-keep.md",
            "A castle above the bay.",
            Some(Uuid::now_v7()),
        );

        assert_eq!(
            detect(
                &gate,
                &[],
                std::slice::from_ref(&entry),
                &WithheldByModeration::exactly(&[])
            ),
            vec![DetectedChange::Deletion {
                lore_entry_id: entry.lore_entry_id,
                repository_path: "westeros/the-red-keep.md".to_string(),
            }],
        );
    }

    /// FR-015, and the reason detection is told what export is withholding.
    ///
    /// An entry a takedown disabled is absent from the repository because we
    /// removed its file. Without this rule that absence reads as a proposed
    /// deletion, and worse, a stale clone pushing the file back would offer
    /// the disabled text as a NEW entry — laundering removed content into the
    /// world under a fresh identifier.
    #[test]
    fn a_withheld_entry_is_invisible_in_both_directions() {
        let gate = gate_for(&connection(Uuid::now_v7(), Uuid::now_v7(), true));
        let entry = exported(
            Uuid::now_v7(),
            "The Red Keep",
            "westeros/the-red-keep.md",
            "A castle above the bay.",
            Some(Uuid::now_v7()),
        );
        let resurrected = file_for(&entry, entry.lore_entry_id, "The text a takedown removed.");
        let withheld = WithheldByModeration::exactly(&[entry.lore_entry_id]);

        assert_eq!(
            detect(
                &gate,
                &[resurrected],
                std::slice::from_ref(&entry),
                &withheld
            ),
            Vec::new(),
            "a moderation-disabled entry was offered back",
        );
        assert_eq!(
            detect(&gate, &[], &[entry], &withheld),
            Vec::new(),
            "our own withholding of a file was reported as the user deleting it",
        );
    }

    /// A repository link comes back as the app's own link syntax, so that
    /// accepting an untouched file changes nothing (SC-008) and accepting an
    /// edited one does not silently break the entry's cross-links.
    #[test]
    fn repository_links_come_back_in_authored_form() {
        let gate = gate_for(&connection(Uuid::now_v7(), Uuid::now_v7(), true));
        let keep = exported(
            Uuid::now_v7(),
            "The Red Keep",
            "westeros/the-red-keep.md",
            "See [[Blackwater Bay]].",
            Some(Uuid::now_v7()),
        );
        let bay = exported(
            Uuid::now_v7(),
            "Blackwater Bay",
            "westeros/blackwater-bay.md",
            "Water.",
            Some(Uuid::now_v7()),
        );

        let destination = paths::relative_link(&keep.current_path, &bay.current_path);
        let file = RepositoryFile {
            path: keep.current_path.clone(),
            contents: document::render(
                &DocumentHeader {
                    id: keep.lore_entry_id,
                    title: keep.title.clone(),
                    tags: Vec::new(),
                    updated: Utc.with_ymd_and_hms(2026, 9, 4, 12, 0, 0).unwrap(),
                    unresolvable_links: Vec::new(),
                },
                &format!("Sail to [Blackwater Bay]({destination}) at dawn."),
            ),
        };

        let changes = detect(
            &gate,
            &[file],
            &[keep.clone(), bay],
            &WithheldByModeration::exactly(&[]),
        );
        let DetectedChange::Update { incoming_body, .. } = &changes[0] else {
            panic!("expected an update, got {changes:?}");
        };
        assert_eq!(incoming_body, "Sail to [[Blackwater Bay]] at dawn.");
    }

    // ---------------------------------------------------------------------
    // Recording, accepting and declining — against the database
    // ---------------------------------------------------------------------

    struct Fixture {
        conn: diesel::r2d2::PooledConnection<diesel::r2d2::ConnectionManager<PgConnection>>,
        user: Uuid,
        world: Uuid,
        gate: IncomingEnabled,
    }

    fn fixture(incoming_enabled: bool) -> Fixture {
        let mut conn = test_app_state().db_pool.get().expect("a connection");
        let user = insert_test_user(&mut conn);
        let world = insert_test_world(&mut conn, user);
        let row = connection(world, user, incoming_enabled);
        diesel::insert_into(crate::schema::lore_repository_connections::table)
            .values(row.clone())
            .execute(&mut conn)
            .expect("the connection is accepted");
        let gate = gate_for(&row);
        Fixture {
            conn,
            user,
            world,
            gate,
        }
    }

    fn entry_content(conn: &mut PgConnection, entry_id: Uuid) -> String {
        world_lore_entries::table
            .filter(world_lore_entries::id.eq(entry_id))
            .select(world_lore_entries::content)
            .first(conn)
            .expect("the entry still exists")
    }

    fn revision_count(conn: &mut PgConnection, entry_id: Uuid) -> i64 {
        world_lore_revisions::table
            .filter(world_lore_revisions::lore_entry_id.eq(entry_id))
            .count()
            .get_result(conn)
            .expect("counted")
    }

    /// FR-023, User Story 3 acceptance scenario 1. Detection plus recording
    /// leaves the world exactly as it was: the entry's text is untouched, no
    /// revision has appeared, and the only thing that changed anywhere is a row
    /// saying somebody should look.
    #[test]
    fn detection_records_a_proposal_and_alters_no_lore() {
        let mut f = fixture(true);
        let entry_id = insert_test_lore_entry(&mut f.conn, f.world, f.user);

        let change = DetectedChange::Update {
            lore_entry_id: entry_id,
            repository_path: "lore/entry.md".to_string(),
            incoming_body: "Text from the repository.".to_string(),
            base_revision_id: None,
            app_revision_id: None,
            also_changed_in_app: false,
        };
        record(&mut f.conn, &f.gate, &[change]).expect("recorded");

        assert_eq!(entry_content(&mut f.conn, entry_id), "");
        assert_eq!(revision_count(&mut f.conn, entry_id), 0);
        assert_eq!(
            pending(&mut f.conn, &f.gate).expect("listed").len(),
            1,
            "the proposal was not recorded, so nobody would ever be asked",
        );
    }

    /// FR-025 and T057. Accepting writes an ORDINARY revision — same table as
    /// every revision a person typed — authored by the accepting user, and the
    /// proposal points at it, which is what makes the revision identifiable as
    /// having come from the repository.
    #[test]
    fn accepting_writes_an_ordinary_revision_attributed_to_the_accepting_user() {
        let mut f = fixture(true);
        let entry_id = insert_test_lore_entry(&mut f.conn, f.world, f.user);
        let accepter = insert_test_user(&mut f.conn);

        let ids = record(
            &mut f.conn,
            &f.gate,
            &[DetectedChange::Update {
                lore_entry_id: entry_id,
                repository_path: "lore/entry.md".to_string(),
                incoming_body: "Text from the repository.".to_string(),
                base_revision_id: None,
                app_revision_id: None,
                also_changed_in_app: false,
            }],
        )
        .expect("recorded");

        let outcome = accept(&mut f.conn, &f.gate, ids[0], accepter).expect("accepted");
        let Acceptance::Updated {
            lore_entry_id,
            revision_id,
        } = outcome
        else {
            panic!("expected an update, got {outcome:?}");
        };
        assert_eq!(lore_entry_id, entry_id);

        assert_eq!(
            entry_content(&mut f.conn, entry_id),
            "Text from the repository.",
        );

        let (author, restored_from, content): (Uuid, Option<Uuid>, String) =
            world_lore_revisions::table
                .filter(world_lore_revisions::id.eq(revision_id))
                .select((
                    world_lore_revisions::author_id,
                    world_lore_revisions::restored_from_revision_id,
                    world_lore_revisions::content_markdown,
                ))
                .first(&mut f.conn)
                .expect("the revision exists");
        assert_eq!(
            author, accepter,
            "the revision was not attributed to the accepting user"
        );
        assert_eq!(content, "Text from the repository.");
        assert_eq!(
            restored_from, None,
            "an incoming change was recorded as a restore, which the history does not mean",
        );

        let current: Option<Uuid> = world_lore_entries::table
            .filter(world_lore_entries::id.eq(entry_id))
            .select(world_lore_entries::current_revision_id)
            .first(&mut f.conn)
            .expect("the entry exists");
        assert_eq!(current, Some(revision_id));

        assert!(
            is_repository_originated(&mut f.conn, revision_id).expect("checked"),
            "the revision is not identifiable as originating from the repository — FR-025",
        );
    }

    /// FR-023 and FR-024 together. Declining a conflicted change leaves the
    /// entry byte for byte as the app had it — no revision, no partial write,
    /// no merge.
    #[test]
    fn declining_leaves_the_entry_byte_for_byte() {
        let mut f = fixture(true);
        let entry_id = insert_test_lore_entry(&mut f.conn, f.world, f.user);
        diesel::update(world_lore_entries::table.filter(world_lore_entries::id.eq(entry_id)))
            .set(world_lore_entries::content.eq("What the app says."))
            .execute(&mut f.conn)
            .expect("set up the app's text");

        let ids = record(
            &mut f.conn,
            &f.gate,
            &[DetectedChange::Update {
                lore_entry_id: entry_id,
                repository_path: "lore/entry.md".to_string(),
                incoming_body: "What the repository says.".to_string(),
                base_revision_id: None,
                app_revision_id: None,
                also_changed_in_app: true,
            }],
        )
        .expect("recorded");

        decline(&mut f.conn, &f.gate, ids[0], f.user).expect("declined");

        assert_eq!(entry_content(&mut f.conn, entry_id), "What the app says.");
        assert_eq!(revision_count(&mut f.conn, entry_id), 0);
        assert!(pending(&mut f.conn, &f.gate).expect("listed").is_empty());
    }

    /// One acceptance per proposal. Two reviewers pressing accept, or a retried
    /// request, must not write the same text twice into an entry's history.
    #[test]
    fn a_change_cannot_be_accepted_twice() {
        let mut f = fixture(true);
        let entry_id = insert_test_lore_entry(&mut f.conn, f.world, f.user);
        let ids = record(
            &mut f.conn,
            &f.gate,
            &[DetectedChange::Update {
                lore_entry_id: entry_id,
                repository_path: "lore/entry.md".to_string(),
                incoming_body: "Once.".to_string(),
                base_revision_id: None,
                app_revision_id: None,
                also_changed_in_app: false,
            }],
        )
        .expect("recorded");

        accept(&mut f.conn, &f.gate, ids[0], f.user).expect("accepted");
        let second = accept(&mut f.conn, &f.gate, ids[0], f.user);

        assert!(matches!(second, Err(IncomingError::NotPending)));
        assert_eq!(revision_count(&mut f.conn, entry_id), 1);
    }

    /// A proposal belongs to the connection that detected it. An id from
    /// another world's review surface finds nothing here, so authority over one
    /// world never becomes authority over another's lore.
    #[test]
    fn a_change_from_another_connection_is_not_acceptable() {
        let mut f = fixture(true);
        let entry_id = insert_test_lore_entry(&mut f.conn, f.world, f.user);
        let ids = record(
            &mut f.conn,
            &f.gate,
            &[DetectedChange::Update {
                lore_entry_id: entry_id,
                repository_path: "lore/entry.md".to_string(),
                incoming_body: "Not yours.".to_string(),
                base_revision_id: None,
                app_revision_id: None,
                also_changed_in_app: false,
            }],
        )
        .expect("recorded");

        let other_world = insert_test_world(&mut f.conn, f.user);
        let other_row = connection(other_world, f.user, true);
        diesel::insert_into(crate::schema::lore_repository_connections::table)
            .values(other_row.clone())
            .execute(&mut f.conn)
            .expect("a second connection");
        let other_gate = gate_for(&other_row);

        assert!(matches!(
            accept(&mut f.conn, &other_gate, ids[0], f.user),
            Err(IncomingError::NotPending),
        ));
        assert!(matches!(
            decline(&mut f.conn, &other_gate, ids[0], f.user),
            Err(IncomingError::NotPending),
        ));
        assert_eq!(entry_content(&mut f.conn, entry_id), "");
    }

    /// FR-026. Confirming a deletion records the confirmation and deletes
    /// nothing — the entry is still there, and the caller is handed the
    /// obligation to remove it through the world's own deletion path, which
    /// knows about children, permissions and images.
    #[test]
    fn accepting_a_deletion_confirms_it_and_deletes_nothing() {
        let mut f = fixture(true);
        let entry_id = insert_test_lore_entry(&mut f.conn, f.world, f.user);
        let ids = record(
            &mut f.conn,
            &f.gate,
            &[DetectedChange::Deletion {
                lore_entry_id: entry_id,
                repository_path: "lore/entry.md".to_string(),
            }],
        )
        .expect("recorded");

        assert_eq!(
            accept(&mut f.conn, &f.gate, ids[0], f.user).expect("confirmed"),
            Acceptance::DeletionConfirmed {
                lore_entry_id: entry_id
            },
        );

        let still_there: i64 = world_lore_entries::table
            .filter(world_lore_entries::id.eq(entry_id))
            .count()
            .get_result(&mut f.conn)
            .expect("counted");
        assert_eq!(
            still_there, 1,
            "confirming a deletion deleted the entry from inside lore_sync",
        );
    }

    /// FR-026's second half: a declined deletion is reversed on the next
    /// synchronisation.
    ///
    /// Exercised as what actually happens rather than as an intention — the
    /// ordinary export pass, given the exported-entry record and a plan that
    /// still contains the entry, writes the file back into a subtree it is
    /// missing from. Nothing in this module is involved, which is the claim:
    /// declining needs no restoration code because the entry never left.
    #[test]
    fn a_declined_deletion_is_restored_by_the_next_export_pass() {
        use crate::lore_sync::apply;
        use crate::lore_sync::plan::{Plan, PlannedFile};

        let mut f = fixture(true);
        let entry_id = insert_test_lore_entry(&mut f.conn, f.world, f.user);
        let ids = record(
            &mut f.conn,
            &f.gate,
            &[DetectedChange::Deletion {
                lore_entry_id: entry_id,
                repository_path: "westeros/the-red-keep.md".to_string(),
            }],
        )
        .expect("recorded");
        decline(&mut f.conn, &f.gate, ids[0], f.user).expect("declined");

        let subtree = std::env::temp_dir().join(format!("tf-incoming-{}", Uuid::now_v7()));
        std::fs::create_dir_all(&subtree).expect("a subtree");

        let plan = Plan {
            files: vec![PlannedFile {
                entry_id,
                path: "westeros/the-red-keep.md".to_string(),
                contents: "A castle above the bay.".to_string(),
            }],
            images: Vec::new(),
            notes: Vec::new(),
        };
        let previously_written =
            HashMap::from([(entry_id, "westeros/the-red-keep.md".to_string())]);

        apply::apply(&subtree, &plan, &previously_written, &|_| None).expect("the pass applies");

        assert_eq!(
            std::fs::read_to_string(subtree.join("westeros/the-red-keep.md"))
                .expect("the file was restored"),
            "A castle above the bay.",
        );
        std::fs::remove_dir_all(&subtree).ok();
    }

    /// T058 and FR-027, through to acceptance. A proposal that matched nothing
    /// creates a NEW entry with a new identifier, and the entry that happened
    /// to share its path and title is untouched.
    #[test]
    fn accepting_a_proposed_new_entry_creates_one_and_touches_no_existing_entry() {
        let mut f = fixture(true);
        let existing = insert_test_lore_entry(&mut f.conn, f.world, f.user);

        let ids = record(
            &mut f.conn,
            &f.gate,
            &[DetectedChange::NewEntry {
                repository_path: "westeros/the-salt-road.md".to_string(),
                proposed_title: "The Salt Road".to_string(),
                incoming_body: "It runs east.".to_string(),
            }],
        )
        .expect("recorded");

        let outcome = accept(&mut f.conn, &f.gate, ids[0], f.user).expect("accepted");
        let Acceptance::Created {
            lore_entry_id,
            revision_id,
        } = outcome
        else {
            panic!("expected a creation, got {outcome:?}");
        };

        assert_ne!(lore_entry_id, existing);
        assert_eq!(entry_content(&mut f.conn, existing), "");
        assert_eq!(revision_count(&mut f.conn, existing), 0);

        let (title, world_id, content): (String, Uuid, String) = world_lore_entries::table
            .filter(world_lore_entries::id.eq(lore_entry_id))
            .select((
                world_lore_entries::title,
                world_lore_entries::world_id,
                world_lore_entries::content,
            ))
            .first(&mut f.conn)
            .expect("the new entry exists");
        assert_eq!(title, "The Salt Road");
        assert_eq!(world_id, f.world);
        assert_eq!(content, "It runs east.");
        assert!(is_repository_originated(&mut f.conn, revision_id).expect("checked"));

        let created: Option<Uuid> = lore_pending_incoming_changes::table
            .filter(lore_pending_incoming_changes::id.eq(ids[0]))
            .select(lore_pending_incoming_changes::created_entry_id)
            .first(&mut f.conn)
            .expect("the proposal exists");
        assert_eq!(
            created,
            Some(lore_entry_id),
            "the proposal does not record which entry it created, so FR-027's \
             'never matched to an existing entry' cannot be audited afterwards",
        );

        let matched: Option<Uuid> = lore_pending_incoming_changes::table
            .filter(lore_pending_incoming_changes::id.eq(ids[0]))
            .select(lore_pending_incoming_changes::lore_entry_id)
            .first(&mut f.conn)
            .expect("the proposal exists");
        assert_eq!(
            matched, None,
            "a proposed new entry ended up naming an entry"
        );
    }

    /// A repeated polling pass observing the same divergence updates the
    /// existing proposal rather than stacking a second one. Two accept buttons
    /// for one entry means pressing both writes the older text last.
    #[test]
    fn a_second_detection_pass_does_not_stack_a_second_proposal() {
        let mut f = fixture(true);
        let entry_id = insert_test_lore_entry(&mut f.conn, f.world, f.user);
        let change = |body: &str| DetectedChange::Update {
            lore_entry_id: entry_id,
            repository_path: "lore/entry.md".to_string(),
            incoming_body: body.to_string(),
            base_revision_id: None,
            app_revision_id: None,
            also_changed_in_app: false,
        };

        let first = record(&mut f.conn, &f.gate, &[change("First.")]).expect("recorded");
        let second = record(&mut f.conn, &f.gate, &[change("Second.")]).expect("recorded again");

        assert_eq!(first, second, "a second pass created a second proposal");
        let rows = pending(&mut f.conn, &f.gate).expect("listed");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].incoming_body.as_deref(), Some("Second."));
    }

    // ---------------------------------------------------------------------
    // The schema's own guarantees
    // ---------------------------------------------------------------------

    /// FR-023 in the database. A row claiming to have written a revision
    /// without being accepted would mean lore had changed with nobody agreeing
    /// to it — the one thing this story must not allow — so the constraint is
    /// exercised rather than assumed.
    #[test]
    fn an_applied_revision_cannot_be_recorded_without_an_acceptance() {
        let mut f = fixture(true);
        let entry_id = insert_test_lore_entry(&mut f.conn, f.world, f.user);
        let revision_id = write_revision(
            &mut f.conn,
            entry_id,
            "Text.",
            f.user,
            Utc::now().naive_utc(),
        )
        .expect("a revision");

        let mut row = row_for(
            &f.gate,
            &DetectedChange::Update {
                lore_entry_id: entry_id,
                repository_path: "lore/entry.md".to_string(),
                incoming_body: "Text.".to_string(),
                base_revision_id: None,
                app_revision_id: None,
                also_changed_in_app: false,
            },
            Utc::now().naive_utc(),
        );
        row.applied_revision_id = Some(revision_id);

        assert!(
            diesel::insert_into(lore_pending_incoming_changes::table)
                .values(row)
                .execute(&mut f.conn)
                .is_err(),
            "a pending row claimed to have applied a revision",
        );
    }

    /// A decision with no decider and no time is not a decision. Without this
    /// constraint "accepted by nobody" is representable and the audit trail
    /// FR-023 rests on has a hole in it.
    #[test]
    fn a_decision_without_a_decider_is_refused() {
        let mut f = fixture(true);
        let entry_id = insert_test_lore_entry(&mut f.conn, f.world, f.user);
        let mut row = row_for(
            &f.gate,
            &DetectedChange::Update {
                lore_entry_id: entry_id,
                repository_path: "lore/entry.md".to_string(),
                incoming_body: "Text.".to_string(),
                base_revision_id: None,
                app_revision_id: None,
                also_changed_in_app: false,
            },
            Utc::now().naive_utc(),
        );
        row.status = STATUS_ACCEPTED.to_string();

        assert!(
            diesel::insert_into(lore_pending_incoming_changes::table)
                .values(row)
                .execute(&mut f.conn)
                .is_err(),
            "a change was accepted by nobody at no time",
        );
    }

    /// FR-027 in the database. A proposal for a new entry that also names an
    /// existing one is the exact shape of the mistake FR-027 forbids, and it is
    /// refused by the schema rather than by whichever code path remembered.
    #[test]
    fn a_new_entry_proposal_cannot_name_an_existing_entry() {
        let mut f = fixture(true);
        let entry_id = insert_test_lore_entry(&mut f.conn, f.world, f.user);
        let mut row = row_for(
            &f.gate,
            &DetectedChange::NewEntry {
                repository_path: "westeros/the-salt-road.md".to_string(),
                proposed_title: "The Salt Road".to_string(),
                incoming_body: "It runs east.".to_string(),
            },
            Utc::now().naive_utc(),
        );
        row.lore_entry_id = Some(entry_id);

        assert!(
            diesel::insert_into(lore_pending_incoming_changes::table)
                .values(row)
                .execute(&mut f.conn)
                .is_err(),
            "a file was allowed to be a new entry and an existing entry at once",
        );
    }
}
