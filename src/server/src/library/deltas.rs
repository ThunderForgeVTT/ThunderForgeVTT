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
use crate::schema::{compendium_entries, compendiums, world_entry_deltas};

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
    pub compendium_id: Uuid,
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
    pub compendium_id: Uuid,
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
            compendium_id: entry.compendium_id,
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
/// FR-013, 049 FR-046) — what switching the book off would lose.
pub fn deltas_named(
    conn: &mut PgConnection,
    world_id: Uuid,
    compendium_id: Uuid,
) -> Result<Vec<String>, BookListError> {
    Ok(deltas_over(conn, world_id, compendium_id, None)?
        .into_iter()
        .map(|delta| format!("{}: {} \"{}\"", delta.form.word(), delta.kind, delta.name))
        .collect())
}

/// Change what an entry says in this world (FR-020, FR-023).
///
/// Only what differs from the book is kept. Fields named here are laid over
/// any change already held; a field set back to exactly what the book says
/// drops out of the delta; and a change that leaves nothing different removes
/// the delta altogether, so "changed" never marks an entry identical to its
/// book. Changing a hidden entry shows it again, changed.
///
/// An addition is changed through here too — it has no book entry, so what
/// it holds is simply replaced.
///
/// Returns the delta as it now stands, or `None` when nothing differs.
pub fn change_entry(
    conn: &mut PgConnection,
    caller: Uuid,
    world_id: Uuid,
    compendium_id: Uuid,
    kind: &str,
    name: &str,
    content: Content,
) -> Result<Option<Delta>, DeltaError> {
    may_change(conn, caller, world_id, compendium_id)?;

    conn.transaction(|conn| {
        let held = held_over(conn, world_id, compendium_id, kind, name)?;
        let base = base_with_identity(conn, compendium_id, kind, name)?;

        if base.is_empty()
            && let Some(addition) = held.clone().filter(|held| held.form == DeltaForm::Added)
        {
            return change_addition(conn, caller, addition, content).map(Some);
        }
        let base = the_one(base, kind, name)?;

        let (field_values, prose_text) = match content {
            Content::Fields(fields) => {
                if base.prose_text.is_some() {
                    return Err(DeltaError::ProseHasNoFields {
                        name: name.to_string(),
                    });
                }
                let mut differing = match held.as_ref() {
                    Some(held) if held.form == DeltaForm::Changed => held
                        .field_values
                        .as_ref()
                        .and_then(|values| values.as_object().cloned())
                        .unwrap_or_default(),
                    _ => serde_json::Map::new(),
                };
                for (field, value) in fields {
                    let value = serde_json::to_value(&value)
                        .map_err(|e| DeltaError::Unstorable(e.to_string()))?;
                    differing.insert(field, value);
                }
                // FR-023: what matches the book is not a difference.
                differing.retain(|field, value| base.field_values.get(field) != Some(value));
                (
                    (!differing.is_empty()).then_some(serde_json::Value::Object(differing)),
                    None,
                )
            }
            Content::Prose(text) => {
                if base.prose_text.is_none() {
                    return Err(DeltaError::FieldsHaveNoProse {
                        name: name.to_string(),
                    });
                }
                let differs = base.prose_text.as_deref() != Some(text.as_str());
                (None, differs.then_some(text))
            }
        };

        if field_values.is_none() && prose_text.is_none() {
            remove_held(conn, world_id, compendium_id, kind, name)?;
            return Ok(None);
        }

        let book_origin = origin_of_book(conn, compendium_id)?;
        upsert(
            conn,
            caller,
            world_id,
            compendium_id,
            kind,
            name,
            DeltaForm::Changed,
            book_origin,
            field_values,
            prose_text,
        )
        .map(Some)
    })
}

/// Stop showing an entry in this world, leaving it in the book and in every
/// other world (FR-021).
///
/// Any change already held over it goes: a hidden entry holds no content, so
/// restoring it brings back the book's entry rather than a half-remembered
/// edit.
pub fn hide_entry(
    conn: &mut PgConnection,
    caller: Uuid,
    world_id: Uuid,
    compendium_id: Uuid,
    kind: &str,
    name: &str,
) -> Result<Delta, DeltaError> {
    may_change(conn, caller, world_id, compendium_id)?;

    conn.transaction(|conn| {
        let base = base_with_identity(conn, compendium_id, kind, name)?;
        the_one(base, kind, name)?;
        let book_origin = origin_of_book(conn, compendium_id)?;
        upsert(
            conn,
            caller,
            world_id,
            compendium_id,
            kind,
            name,
            DeltaForm::Hidden,
            book_origin,
            None,
            None,
        )
    })
}

/// Write an entry into this world beside the book (FR-021, FR-052a).
///
/// Authored, always, and not because a caller said so: [`DeltaForm::Added`]
/// has one origin and the database refuses any other.
pub fn add_entry(
    conn: &mut PgConnection,
    caller: Uuid,
    world_id: Uuid,
    compendium_id: Uuid,
    kind: &str,
    name: &str,
    content: Content,
) -> Result<Delta, DeltaError> {
    may_change(conn, caller, world_id, compendium_id)?;
    if kind.trim().is_empty() || name.trim().is_empty() {
        return Err(DeltaError::Unnamed);
    }

    conn.transaction(|conn| {
        if !base_with_identity(conn, compendium_id, kind, name)?.is_empty() {
            return Err(DeltaError::AlreadyInTheBook {
                kind: kind.to_string(),
                name: name.to_string(),
            });
        }
        if held_over(conn, world_id, compendium_id, kind, name)?.is_some() {
            return Err(DeltaError::AlreadyAdded {
                kind: kind.to_string(),
                name: name.to_string(),
            });
        }

        let (field_values, prose_text) = whole(content)?;
        let book_origin = origin_of_book(conn, compendium_id)?;
        upsert(
            conn,
            caller,
            world_id,
            compendium_id,
            kind,
            name,
            DeltaForm::Added,
            book_origin,
            Some(field_values),
            prose_text,
        )
    })
}

/// Put an entry back as the book has it (FR-024) — or, for an addition, take
/// it out of the world.
///
/// Returns the delta that was removed, so a caller can say what went.
pub fn restore_entry(
    conn: &mut PgConnection,
    caller: Uuid,
    world_id: Uuid,
    compendium_id: Uuid,
    kind: &str,
    name: &str,
) -> Result<Delta, DeltaError> {
    may_change(conn, caller, world_id, compendium_id)?;
    remove_held(conn, world_id, compendium_id, kind, name)?.ok_or_else(|| {
        DeltaError::NothingToRestore {
            kind: kind.to_string(),
            name: name.to_string(),
        }
    })
}

/// Both gates a change passes: the caller is trusted with this table's books
/// (FR-020a), and the book is one this world is reading (FR-042).
fn may_change(
    conn: &mut PgConnection,
    caller: Uuid,
    world_id: Uuid,
    compendium_id: Uuid,
) -> Result<(), DeltaError> {
    book_list::require_book_manager(conn, caller, world_id)?;
    book_list::require_served(conn, world_id, compendium_id)?;
    Ok(())
}

fn origin_of_book(conn: &mut PgConnection, compendium_id: Uuid) -> QueryResult<ContentOrigin> {
    compendiums::table
        .filter(compendiums::id.eq(compendium_id))
        .select(compendiums::origin)
        .first(conn)
}

fn base_with_identity(
    conn: &mut PgConnection,
    compendium_id: Uuid,
    kind: &str,
    name: &str,
) -> QueryResult<Vec<StoredEntry>> {
    compendium_entries::table
        .filter(compendium_entries::compendium_id.eq(compendium_id))
        .filter(compendium_entries::kind.eq(kind))
        .filter(compendium_entries::name.eq(name))
        .select(StoredEntry::as_select())
        .load(conn)
}

/// The base entry a delta may attach to, or the refusal (FR-025, FR-025a).
fn the_one(mut base: Vec<StoredEntry>, kind: &str, name: &str) -> Result<StoredEntry, DeltaError> {
    match base.len() {
        0 => Err(DeltaError::NoSuchEntry {
            kind: kind.to_string(),
            name: name.to_string(),
        }),
        1 => Ok(base.remove(0)),
        count => Err(DeltaError::Ambiguous {
            kind: kind.to_string(),
            name: name.to_string(),
            count,
        }),
    }
}

fn held_over(
    conn: &mut PgConnection,
    world_id: Uuid,
    compendium_id: Uuid,
    kind: &str,
    name: &str,
) -> QueryResult<Option<Delta>> {
    world_entry_deltas::table
        .filter(world_entry_deltas::world_id.eq(world_id))
        .filter(world_entry_deltas::compendium_id.eq(compendium_id))
        .filter(world_entry_deltas::kind.eq(kind))
        .filter(world_entry_deltas::name.eq(name))
        .select(Delta::as_select())
        .first(conn)
        .optional()
}

fn remove_held(
    conn: &mut PgConnection,
    world_id: Uuid,
    compendium_id: Uuid,
    kind: &str,
    name: &str,
) -> QueryResult<Option<Delta>> {
    diesel::delete(
        world_entry_deltas::table
            .filter(world_entry_deltas::world_id.eq(world_id))
            .filter(world_entry_deltas::compendium_id.eq(compendium_id))
            .filter(world_entry_deltas::kind.eq(kind))
            .filter(world_entry_deltas::name.eq(name)),
    )
    .returning(Delta::as_select())
    .get_result(conn)
    .optional()
}

/// An addition's content, whole.
fn whole(content: Content) -> Result<(serde_json::Value, Option<String>), DeltaError> {
    Ok(match content {
        Content::Fields(fields) => (
            serde_json::to_value(&fields).map_err(|e| DeltaError::Unstorable(e.to_string()))?,
            None,
        ),
        Content::Prose(text) => (serde_json::json!({}), Some(text)),
    })
}

/// Replace what an addition holds. There is no book to differ from, so the
/// fields named are laid over the addition's own and prose is replaced.
fn change_addition(
    conn: &mut PgConnection,
    caller: Uuid,
    addition: Delta,
    content: Content,
) -> Result<Delta, DeltaError> {
    let is_prose = addition.prose_text.is_some();
    let (field_values, prose_text) = match content {
        Content::Fields(fields) => {
            if is_prose {
                return Err(DeltaError::ProseHasNoFields {
                    name: addition.name,
                });
            }
            let mut held = addition
                .field_values
                .as_ref()
                .and_then(|values| values.as_object().cloned())
                .unwrap_or_default();
            for (field, value) in fields {
                held.insert(
                    field,
                    serde_json::to_value(&value)
                        .map_err(|e| DeltaError::Unstorable(e.to_string()))?,
                );
            }
            (serde_json::Value::Object(held), None)
        }
        Content::Prose(text) => {
            let has_fields = addition
                .field_values
                .as_ref()
                .and_then(|values| values.as_object())
                .is_some_and(|values| !values.is_empty());
            if has_fields {
                return Err(DeltaError::FieldsHaveNoProse {
                    name: addition.name,
                });
            }
            (serde_json::json!({}), Some(text))
        }
    };

    diesel::update(world_entry_deltas::table.filter(world_entry_deltas::id.eq(addition.id)))
        .set((
            world_entry_deltas::field_values.eq(Some(field_values)),
            world_entry_deltas::prose_text.eq(prose_text),
            world_entry_deltas::changed_by.eq(Some(caller)),
            world_entry_deltas::updated_at.eq(diesel::dsl::now),
        ))
        .returning(Delta::as_select())
        .get_result(conn)
        .map_err(Into::into)
}

/// Write a delta over an identity, replacing whatever this world held there.
///
/// The origin is computed here from the form, never taken from a caller; the
/// trigger checks it again. An update never names `origin`, so a change can
/// never become an addition in place — the trigger would refuse it, and this
/// function gives it nothing to try with.
#[allow(clippy::too_many_arguments)]
fn upsert(
    conn: &mut PgConnection,
    caller: Uuid,
    world_id: Uuid,
    compendium_id: Uuid,
    kind: &str,
    name: &str,
    form: DeltaForm,
    book_origin: ContentOrigin,
    field_values: Option<serde_json::Value>,
    prose_text: Option<String>,
) -> Result<Delta, DeltaError> {
    diesel::insert_into(world_entry_deltas::table)
        .values((
            world_entry_deltas::id.eq(Uuid::now_v7()),
            world_entry_deltas::world_id.eq(world_id),
            world_entry_deltas::compendium_id.eq(compendium_id),
            world_entry_deltas::kind.eq(kind),
            world_entry_deltas::name.eq(name),
            world_entry_deltas::form.eq(form),
            world_entry_deltas::origin.eq(form.origin_over(book_origin)),
            world_entry_deltas::field_values.eq(&field_values),
            world_entry_deltas::prose_text.eq(&prose_text),
            world_entry_deltas::changed_by.eq(Some(caller)),
        ))
        .on_conflict((
            world_entry_deltas::world_id,
            world_entry_deltas::compendium_id,
            world_entry_deltas::kind,
            world_entry_deltas::name,
        ))
        .do_update()
        .set((
            world_entry_deltas::form.eq(form),
            world_entry_deltas::field_values.eq(&field_values),
            world_entry_deltas::prose_text.eq(&prose_text),
            world_entry_deltas::changed_by.eq(Some(caller)),
            world_entry_deltas::updated_at.eq(diesel::dsl::now),
        ))
        .returning(Delta::as_select())
        .get_result(conn)
        .map_err(Into::into)
}

#[cfg(test)]
#[path = "deltas_rule_tests.rs"]
mod rule_tests;

#[cfg(test)]
#[path = "deltas_tests.rs"]
mod tests;
