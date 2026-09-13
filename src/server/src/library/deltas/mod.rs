//! A world's own changes to the books it inherited (spec 050 US2, FR-020 to
//! FR-028, FR-052, FR-052a).
//!
//! # What a world reads
//!
//! The base with this world's delta applied (FR-022), and nothing copied to
//! get there. [`world_reads`] fetches the book's entries from the owner's
//! shelf exactly as [`super::book_list::entries_served_by`] always has, fetches
//! this world's deltas over that book, and lays one over the other in memory
//! through [`resolve`]. The base is never written by anything in this module;
//! there is no function here that names `compendium_entries` in an UPDATE or
//! DELETE, and the end-to-end test compares the base's bytes before and after
//! to show there is no route around that either.
//!
//! # Identity, and when it refuses
//!
//! A delta attaches to an entry's **kind and name** (FR-025) — never to its
//! id, which a re-import replaces. Spec 049's Phase 10 measured that identity
//! stable for every real entry across a genuine re-parse, and measured it
//! **not unique** for 3.9% of them: two Goblins in one Monster Manual, a
//! stat block and a variant.
//!
//! Where kind and name name more than one base entry, a delta attaches to
//! neither (FR-025a). The refusal happens twice, because the ambiguity can
//! arrive twice:
//!
//! * **at write time**, where [`change_entry`] and [`hide_entry`] refuse with
//!   [`DeltaError::Ambiguous`] and change nothing — the person is told the
//!   book holds two of those and nothing is guessed;
//! * **at read time**, where a delta that was written when the identity was
//!   unique and has since gained a twin (a re-read found a second Goblin) is
//!   not applied to either. Both entries are served as the book has them, and
//!   the delta is returned in [`Resolution::unattached`] with the reason, so
//!   it is reported rather than silently dropped (FR-027) and rather than
//!   silently applied to whichever row happened to come first.
//!
//! Guessing wrong here is the worst outcome available: it rewrites the wrong
//! entry and says nothing, and nobody finds out until a player's goblin has
//! the wrong hit points mid-fight.
//!
//! # Origin, per entry
//!
//! The subtlety most likely to be implemented wrong, so it is decided in one
//! place. A **changed** or **hidden** entry is a mutation of a book entry and
//! carries the book's origin — an uploaded sword stays uploaded however much
//! it is retuned (FR-052). An **added** entry was written by a person at this
//! table and is **authored**, whatever book it sits beside (FR-052a). Same
//! screen, same people, opposite rights.
//!
//! [`DeltaForm::origin_over`] is the single statement of that rule on this
//! side; the migration's trigger is the same rule on the database's side, so
//! a write that disagrees with it cannot land. [`origin_of_delta_entry`] is
//! how anything outside this module asks.

use std::collections::{BTreeMap, HashMap};

use diesel::prelude::*;
use diesel_derive_enum::DbEnum;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::compendium::ContentOrigin;
use crate::compendium::store::StoredEntry;
use crate::content::ReadValue;
use crate::schema::{compendiums, world_entry_deltas};

use super::book_list::{self, BookListError};

/// The three forms a delta takes (FR-021).
#[derive(DbEnum, Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[ExistingTypePath = "crate::schema::sql_types::DeltaForm"]
// The migration writes PascalCase, for the reason `ContentOrigin` records:
// without this every insert fails at the database, invisibly to the compiler.
#[DbValueStyle = "PascalCase"]
pub enum DeltaForm {
    /// An entry in the book, with some of what it says replaced.
    Changed,
    /// An entry in the book that this world does not show.
    Hidden,
    /// An entry that is not in the book at all, written at this table.
    Added,
}

impl DeltaForm {
    /// What origin an entry in this form carries, over a book of `book`'s
    /// origin (FR-052, FR-052a).
    ///
    /// Exhaustive on purpose: a fourth form must be given an origin by
    /// somebody deciding, not inherit one from a wildcard arm.
    pub fn origin_over(self, book: ContentOrigin) -> ContentOrigin {
        match self {
            // A mutation has no meaning apart from the thing it mutates.
            DeltaForm::Changed | DeltaForm::Hidden => book,
            // Nothing was read out of a document to make this.
            DeltaForm::Added => ContentOrigin::Authored,
        }
    }

    /// How the form reads in a sentence a person is shown.
    pub fn word(self) -> &'static str {
        match self {
            DeltaForm::Changed => "changed",
            DeltaForm::Hidden => "hidden",
            DeltaForm::Added => "added",
        }
    }
}

/// One delta, as it is stored.
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = world_entry_deltas)]
pub struct Delta {
    pub id: Uuid,
    pub world_id: Uuid,
    /// The book this delta is over, or was written beside. `None` only for an
    /// addition whose book has since been removed from the shelf (decision 5),
    /// in which case `written_beside_title` names it.
    pub compendium_id: Option<Uuid>,
    pub kind: String,
    pub name: String,
    pub form: DeltaForm,
    pub origin: ContentOrigin,
    /// Changed: only the fields that differ. Added: every field. Hidden:
    /// nothing.
    pub field_values: Option<serde_json::Value>,
    pub prose_text: Option<String>,
    pub changed_by: Option<Uuid>,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
    /// The title of the book an addition was written beside, kept when that
    /// book left the shelf. A name and not a book: nothing can be read from
    /// it, and nothing can attach the addition to a book again.
    pub written_beside_title: Option<String>,
}

/// What a person asks to change an entry's content to.
///
/// Fields **or** prose, never both, because an entry holds one or the other
/// (049 FR-001b) and a request that could carry both is a request somebody
/// eventually sends with both filled in.
#[derive(Debug, Clone)]
pub enum Content {
    /// Declared fields to set. For a change, only the ones named are touched;
    /// for an addition, these are the entry's fields.
    Fields(BTreeMap<String, ReadValue>),
    /// What a prose entry says.
    Prose(String),
}

/// How an entry stands in this world.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryState {
    /// As the book has it.
    Inherited,
    /// The book's entry, with this world's changes over it.
    Changed,
    /// The book's entry, not shown to this world. Only returned when a person
    /// arranging the books asks to see what is hidden, so they can restore it.
    Hidden,
    /// Not in the book; written at this table.
    Added,
}

/// What an entry was before this world changed it (FR-024).
///
/// The base itself, read from the shelf — not a snapshot kept beside the
/// delta. The base is immutable while it is the base (FR-006), so "before"
/// never needs storing, and a stored copy would be a second thing that could
/// disagree with the book.
#[derive(Debug, Clone, PartialEq)]
pub struct Before {
    pub field_values: serde_json::Value,
    pub prose_text: Option<String>,
}

/// One entry as this world reads it.
#[derive(Debug, Clone)]
pub struct WorldEntry {
    /// The base entry's id where there is one, so a cursor over a book means
    /// the same place whether or not an entry has been changed; the delta's
    /// id for an addition, which has no base entry.
    pub id: Uuid,
    /// The book this entry is read from or was written beside; `None` for an
    /// addition whose book has been removed from the shelf.
    pub compendium_id: Option<Uuid>,
    pub kind: String,
    pub name: String,
    pub name_uncertain: bool,
    /// The page of the book it is on. `None` for an addition, which is on no
    /// page of any book, and saying otherwise would send somebody looking.
    pub page: Option<i32>,
    pub field_values: serde_json::Value,
    pub prose_text: Option<String>,
    pub suspect: bool,
    pub state: EntryState,
    /// Whether this entry may leave the account (FR-052, FR-052a) — decided
    /// per entry, which is the whole point of carrying it here rather than on
    /// the book.
    pub origin: ContentOrigin,
    /// The delta over this entry, if there is one.
    pub delta_id: Option<Uuid>,
    /// What the book says, for a changed entry (FR-024).
    pub before: Option<Before>,
    /// This entry shares its kind and name with another in the same book, so
    /// no delta can attach to it (FR-025a). Shown so a person understands why
    /// the change controls refuse, rather than discovering it by trying.
    pub ambiguous: bool,
}

/// Why a delta held by this world is not being applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unattached {
    /// The book holds more than one entry with this kind and name (FR-025a).
    Ambiguous { count: usize },
    /// The book holds no entry with this kind and name any more — a re-read
    /// renamed or lost it (FR-027).
    NoSuchEntry,
    /// An addition whose identity the book now has an entry for. Serving both
    /// would make the identity ambiguous for every later delta, so the book's
    /// entry is served and the addition reported.
    ShadowsTheBook,
}

/// What a world reads from one book, and what it holds that could not be
/// applied.
#[derive(Debug, Clone)]
pub struct Resolution {
    pub entries: Vec<WorldEntry>,
    /// Reported, never discarded (FR-027). Each is still stored; restoring it
    /// is how a person lets it go.
    pub unattached: Vec<(Delta, Unattached)>,
}

/// Why a change did not happen.
#[derive(Debug, thiserror::Error)]
pub enum DeltaError {
    /// Who may change a table's books, whose they are, and whether the book
    /// is on and still matches — every refusal the book list already words.
    #[error("{0}")]
    Book(#[from] BookListError),
    #[error("this book has no {kind} named \"{name}\"")]
    NoSuchEntry { kind: String, name: String },
    /// FR-025a, at write time.
    #[error(
        "this book has {count} {kind} entries named \"{name}\", so a change cannot tell which one you mean; nothing was changed"
    )]
    Ambiguous {
        kind: String,
        name: String,
        count: usize,
    },
    /// An addition may not take an identity the book already has: that would
    /// manufacture exactly the ambiguity FR-025a refuses.
    #[error("this book already has a {kind} named \"{name}\"; change that entry instead")]
    AlreadyInTheBook { kind: String, name: String },
    #[error("this world has already added a {kind} named \"{name}\"")]
    AlreadyAdded { kind: String, name: String },
    /// Decision 5 kept an addition of this identity when its book left the
    /// shelf; a second would be indistinguishable from it.
    #[error(
        "this table already wrote a {kind} named \"{name}\" beside {book_title}, which has left the shelf; change or remove that one instead"
    )]
    AlreadyWrittenHere {
        kind: String,
        name: String,
        book_title: String,
    },
    #[error("this world has no addition of its own with that id")]
    NoSuchAddition,
    #[error("this world has not changed, hidden or added a {kind} named \"{name}\"")]
    NothingToRestore { kind: String, name: String },
    #[error("\"{name}\" is prose, and prose has no fields to change")]
    ProseHasNoFields { name: String },
    #[error("\"{name}\" is read as fields, and has no prose to change")]
    FieldsHaveNoProse { name: String },
    #[error("an entry needs a kind and a name")]
    Unnamed,
    #[error("an entry could not be stored as written: {0}")]
    Unstorable(String),
    #[error("database error: {0}")]
    Database(String),
}

impl From<diesel::result::Error> for DeltaError {
    fn from(e: diesel::result::Error) -> Self {
        DeltaError::Database(e.to_string())
    }
}

type Identity = (String, String);

fn identity_of(kind: &str, name: &str) -> Identity {
    (kind.to_string(), name.to_string())
}

/// Lay a world's deltas over a book's entries (FR-022).
///
/// Pure, and deliberately so: the rule for what a world reads is the part
/// that must be right, and a function of two lists can be tested at every
/// edge without a database, and measured without one (T076).
///
/// `show_hidden` returns hidden entries marked [`EntryState::Hidden`] rather
/// than leaving them out, for the person who wants to put one back.
pub fn resolve(
    book_origin: ContentOrigin,
    base: Vec<StoredEntry>,
    deltas: Vec<Delta>,
    show_hidden: bool,
) -> Resolution {
    let mut in_the_book: HashMap<Identity, usize> = HashMap::with_capacity(base.len());
    for entry in &base {
        *in_the_book
            .entry(identity_of(&entry.kind, &entry.name))
            .or_default() += 1;
    }

    let mut held: HashMap<Identity, Delta> = deltas
        .into_iter()
        .map(|delta| (identity_of(&delta.kind, &delta.name), delta))
        .collect();

    let mut entries = Vec::with_capacity(base.len() + held.len());
    let mut unattached = Vec::new();

    for entry in base {
        let identity = identity_of(&entry.kind, &entry.name);
        let count = in_the_book[&identity];
        let ambiguous = count > 1;

        // Taken out of `held` the first time its identity is met, so a delta
        // over an ambiguous identity is reported once and not once per twin.
        let delta = held.remove(&identity);

        let mut read = WorldEntry {
            id: entry.id,
            compendium_id: Some(entry.compendium_id),
            kind: entry.kind,
            name: entry.name,
            name_uncertain: entry.name_uncertain,
            page: Some(entry.page),
            field_values: entry.field_values,
            prose_text: entry.prose_text,
            suspect: entry.suspect,
            state: EntryState::Inherited,
            origin: book_origin,
            delta_id: None,
            before: None,
            ambiguous,
        };

        match delta {
            None => {}
            // FR-025a: attach to neither. The book is served as it is and the
            // delta is reported.
            Some(delta) if ambiguous => {
                unattached.push((delta, Unattached::Ambiguous { count }));
            }
            Some(delta) => match delta.form {
                DeltaForm::Changed => {
                    read.before = Some(Before {
                        field_values: read.field_values.clone(),
                        prose_text: read.prose_text.clone(),
                    });
                    if let Some(serde_json::Value::Object(changed)) = &delta.field_values {
                        if let serde_json::Value::Object(fields) = &mut read.field_values {
                            for (field, value) in changed {
                                fields.insert(field.clone(), value.clone());
                            }
                        } else {
                            read.field_values = serde_json::Value::Object(changed.clone());
                        }
                    }
                    if delta.prose_text.is_some() {
                        read.prose_text = delta.prose_text.clone();
                    }
                    read.state = EntryState::Changed;
                    read.origin = DeltaForm::Changed.origin_over(book_origin);
                    read.delta_id = Some(delta.id);
                }
                DeltaForm::Hidden => {
                    if !show_hidden {
                        continue;
                    }
                    read.state = EntryState::Hidden;
                    read.origin = DeltaForm::Hidden.origin_over(book_origin);
                    read.delta_id = Some(delta.id);
                }
                DeltaForm::Added => {
                    unattached.push((delta, Unattached::ShadowsTheBook));
                }
            },
        }

        entries.push(read);
    }

    // What is left named nothing in the book.
    for (_, delta) in held {
        match delta.form {
            DeltaForm::Added => entries.push(WorldEntry {
                id: delta.id,
                compendium_id: delta.compendium_id,
                kind: delta.kind.clone(),
                name: delta.name.clone(),
                name_uncertain: false,
                page: None,
                field_values: delta
                    .field_values
                    .clone()
                    .unwrap_or_else(|| serde_json::json!({})),
                prose_text: delta.prose_text.clone(),
                suspect: false,
                state: EntryState::Added,
                origin: DeltaForm::Added.origin_over(book_origin),
                delta_id: Some(delta.id),
                before: None,
                ambiguous: false,
            }),
            DeltaForm::Changed | DeltaForm::Hidden => {
                unattached.push((delta, Unattached::NoSuchEntry));
            }
        }
    }

    // A total order, for the reason `page_of` gives: two entries may share a
    // kind and a name, and a cursor over an order with ties can skip one.
    entries.sort_by(|left, right| {
        (&left.kind, &left.name, left.id).cmp(&(&right.kind, &right.name, right.id))
    });
    unattached
        .sort_by(|(left, _), (right, _)| (&left.kind, &left.name).cmp(&(&right.kind, &right.name)));

    Resolution {
        entries,
        unattached,
    }
}

/// Every delta this world holds over one book, optionally of one kind.
pub fn deltas_over(
    conn: &mut PgConnection,
    world_id: Uuid,
    compendium_id: Uuid,
    kind: Option<&str>,
) -> QueryResult<Vec<Delta>> {
    let mut query = world_entry_deltas::table
        .filter(world_entry_deltas::world_id.eq(world_id))
        .filter(world_entry_deltas::compendium_id.eq(compendium_id))
        .into_boxed();
    if let Some(kind) = kind {
        query = query.filter(world_entry_deltas::kind.eq(kind.to_string()));
    }
    query
        .order((
            world_entry_deltas::kind.asc(),
            world_entry_deltas::name.asc(),
        ))
        .select(Delta::as_select())
        .load(conn)
}

/// What this world reads from one book: the base with its delta applied
/// (FR-022), and what could not be applied.
///
/// Authorised as [`book_list::entries_served_by`] is — by the world, not the
/// account — and refused where that is refused: a book not on the list, or
/// one the world's system no longer matches. The caller decides who may ask.
pub fn world_reads(
    conn: &mut PgConnection,
    world_id: Uuid,
    compendium_id: Uuid,
    kind: Option<&str>,
    show_hidden: bool,
) -> Result<(String, Resolution), DeltaError> {
    let (book_title, base) = book_list::entries_served_by(conn, world_id, compendium_id, kind)?;
    let book_origin = origin_of_book(conn, compendium_id)?;
    let deltas = deltas_over(conn, world_id, compendium_id, kind)?;
    Ok((book_title, resolve(book_origin, base, deltas, show_hidden)))
}

/// The one entry this world reads under a kind and name, hidden included, if
/// there is exactly one — what a mutation hands back after changing it.
pub fn world_entry(
    conn: &mut PgConnection,
    world_id: Uuid,
    compendium_id: Uuid,
    kind: &str,
    name: &str,
) -> Result<Option<WorldEntry>, DeltaError> {
    let (_, resolution) = world_reads(conn, world_id, compendium_id, Some(kind), true)?;
    let mut matching = resolution
        .entries
        .into_iter()
        .filter(|entry| entry.name == name);
    Ok(match (matching.next(), matching.next()) {
        (Some(only), None) => Some(only),
        _ => None,
    })
}

/// The origin of one delta entry, by the delta's id (FR-052, FR-052a).
///
/// **The meeting point with the collection invariant** (spec 049 Phase 8).
/// Anything that must decide whether an entry a world shows may be shared
/// asks here when the id it holds is a delta's — which is the id
/// [`WorldEntry::id`] carries for an addition. A changed or hidden entry
/// carries its base entry's id, whose origin is its book's.
///
/// `None` means no such delta, which a sharing check must treat as a refusal
/// rather than as permission: an id this function does not recognise is not
/// evidence of authorship.
pub fn origin_of_delta_entry(
    conn: &mut PgConnection,
    delta_id: Uuid,
) -> QueryResult<Option<ContentOrigin>> {
    world_entry_deltas::table
        .filter(world_entry_deltas::id.eq(delta_id))
        .select(world_entry_deltas::origin)
        .first(conn)
        .optional()
}

/// Each delta this world holds over a book, as a person would name it (050
/// FR-013, 049 FR-046), split by what switching the book off does to it:
/// changes and hides are lost, additions are kept (decision 5).
pub fn deltas_named(
    conn: &mut PgConnection,
    world_id: Uuid,
    compendium_id: Uuid,
) -> Result<(Vec<String>, Vec<String>), BookListError> {
    let (kept, lost): (Vec<Delta>, Vec<Delta>) = deltas_over(conn, world_id, compendium_id, None)?
        .into_iter()
        .partition(|delta| delta.form == DeltaForm::Added);
    let named = |deltas: Vec<Delta>| {
        deltas
            .into_iter()
            .map(|delta| format!("{}: {} \"{}\"", delta.form.word(), delta.kind, delta.name))
            .collect()
    };
    Ok((named(lost), named(kept)))
}

/// What this world wrote beside books it is no longer reading (spec 050
/// decision 5), each with the title of the book it was written beside.
///
/// Two ways to get here, and both keep the writing:
///
/// * the book was **switched off**. The addition still names the book, and
///   switching the book back on brings it back onto the book's page as the
///   same row — the identity (world, book, kind, name) was never released, so
///   there is no second copy to reconcile;
/// * the book was **removed from the shelf**. The database detached the
///   addition as the book went and kept the book's title in its place. It
///   cannot rejoin anything, including a re-import of the same file, which is
///   a different book with a different id.
///
/// Listed on their own rather than under a book, because showing them inside a
/// book the world is not running would suggest the book is partly on.
///
/// Not gated here; the caller decides who may read a world's book material.
pub fn additions_without_their_book(
    conn: &mut PgConnection,
    world_id: Uuid,
) -> QueryResult<Vec<(String, WorldEntry)>> {
    use crate::schema::world_books;

    let on_the_list: Vec<Uuid> = world_books::table
        .filter(world_books::world_id.eq(world_id))
        .select(world_books::compendium_id)
        .load(conn)?;

    let additions: Vec<(Delta, Option<String>)> = world_entry_deltas::table
        .left_join(
            compendiums::table.on(world_entry_deltas::compendium_id.eq(compendiums::id.nullable())),
        )
        .filter(world_entry_deltas::world_id.eq(world_id))
        .filter(world_entry_deltas::form.eq(DeltaForm::Added))
        .order((
            world_entry_deltas::kind.asc(),
            world_entry_deltas::name.asc(),
        ))
        .select((Delta::as_select(), compendiums::book_title.nullable()))
        .load(conn)?;

    Ok(additions
        .into_iter()
        .filter(|(delta, _)| {
            delta
                .compendium_id
                .is_none_or(|book| !on_the_list.contains(&book))
        })
        .filter_map(|(delta, current_title)| {
            // The book's title while it is on the shelf; the title kept when
            // it left. The constraint on the table makes one of them present.
            let title = current_title.or_else(|| delta.written_beside_title.clone())?;
            // An addition's origin does not depend on the book: it is
            // authored. The book origin passed here is therefore unused by
            // the rule, and `Authored` says so rather than inventing a read.
            let entry = resolve(ContentOrigin::Authored, Vec::new(), vec![delta], false)
                .entries
                .pop()?;
            Some((title, entry))
        })
        .collect())
}

/// What removing a book from the shelf would do to each world's deltas over
/// it: what goes, and what stays (spec 050 decision 5, FR-060, FR-061).
///
/// Per world, for every world holding any delta over the book — including a
/// world that has since switched the book off and kept an addition, which the
/// book list alone would not name. The names are formatted as the switch-off
/// report formats them, so a person reads one vocabulary in both places.
pub fn removal_consequences(
    conn: &mut PgConnection,
    compendium_id: Uuid,
) -> QueryResult<Vec<RemovalConsequence>> {
    use crate::schema::worlds;

    let held: Vec<(Delta, String)> = world_entry_deltas::table
        .inner_join(worlds::table.on(worlds::id.eq(world_entry_deltas::world_id)))
        .filter(world_entry_deltas::compendium_id.eq(compendium_id))
        .order((
            worlds::name.asc(),
            world_entry_deltas::kind.asc(),
            world_entry_deltas::name.asc(),
        ))
        .select((Delta::as_select(), worlds::name))
        .load(conn)?;

    let mut consequences: Vec<RemovalConsequence> = Vec::new();
    for (delta, world_name) in held {
        let named = format!("{}: {} \"{}\"", delta.form.word(), delta.kind, delta.name);
        let at = match consequences
            .iter()
            .position(|each| each.world_id == delta.world_id)
        {
            Some(at) => at,
            None => {
                consequences.push(RemovalConsequence {
                    world_id: delta.world_id,
                    world_name,
                    lost: Vec::new(),
                    kept: Vec::new(),
                });
                consequences.len() - 1
            }
        };
        if delta.form == DeltaForm::Added {
            consequences[at].kept.push(named);
        } else {
            consequences[at].lost.push(named);
        }
    }
    Ok(consequences)
}

/// One world's share of a book's removal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemovalConsequence {
    pub world_id: Uuid,
    pub world_name: String,
    /// Changes and hides, which go with the book.
    pub lost: Vec<String>,
    /// Additions, which stay in the world as its own writing.
    pub kept: Vec<String>,
}

mod write;

use write::origin_of_book;
pub use write::{add_entry, change_entry, hide_entry, remove_kept_addition, restore_entry};

#[cfg(test)]
#[path = "rule_tests.rs"]
mod rule_tests;

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

#[cfg(test)]
#[path = "lifecycle_tests.rs"]
mod lifecycle_tests;

#[cfg(test)]
#[path = "removal_tests.rs"]
mod removal_tests;
