//! Writing a world's deltas: change, hide, add, restore (spec 050 FR-020 to
//! FR-024).
//!
//! Split from [`super`] so the rule for what a world *reads* and the rules
//! for what may be *written* can each be read whole. Every write here passes
//! the same two gates — who may change a table's books, and whether the book
//! is one the world is reading — and computes origin from the form rather
//! than taking it from a caller.

use diesel::prelude::*;
use uuid::Uuid;

use crate::compendium::ContentOrigin;
use crate::compendium::store::StoredEntry;
use crate::schema::{compendium_entries, compendiums, world_entry_deltas};

use super::{Content, Delta, DeltaError, DeltaForm, book_list};
use crate::library::book_list::BookListError;
use crate::play_pause::gate::refuse_if_paused;

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
        // A kept addition from a book that has left the shelf is this table's
        // own writing under that kind and name. A new addition with the same
        // identity beside a different book — a re-import of the same file
        // included — would be two entries this table cannot tell apart, which
        // is the ambiguity FR-025a refuses in a base. Refused, not merged: the
        // kept one is edited or removed on purpose, never absorbed.
        if let Some(title) = kept_from_a_removed_book(conn, world_id, kind, name)? {
            return Err(DeltaError::AlreadyWrittenHere {
                kind: kind.to_string(),
                name: name.to_string(),
                book_title: title,
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
/// An addition whose book is switched off can still be taken out: it is the
/// world's own writing and stays until somebody removes it (decision 5), so
/// there must be a way to remove it that does not require switching a book
/// back on first. A change or hide over a switched-off book cannot exist to be
/// restored — the database took it with the book.
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
    book_list::require_book_manager(conn, caller, world_id)?;
    refuse_if_paused(conn, world_id).map_err(BookListError::Paused)?;
    let held = held_over(conn, world_id, compendium_id, kind, name)?;
    let orphaned_addition = held
        .as_ref()
        .is_some_and(|held| held.form == DeltaForm::Added)
        && matches!(
            book_list::require_served(conn, world_id, compendium_id),
            Err(BookListError::NotOnTheList)
        );
    if !orphaned_addition {
        book_list::require_served(conn, world_id, compendium_id)?;
    }
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
    refuse_if_paused(conn, world_id).map_err(BookListError::Paused)?;
    book_list::require_served(conn, world_id, compendium_id)?;
    Ok(())
}

pub(super) fn origin_of_book(
    conn: &mut PgConnection,
    compendium_id: Uuid,
) -> QueryResult<ContentOrigin> {
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

/// The title of the removed book beside which this world kept an addition of
/// this kind and name, if it did.
fn kept_from_a_removed_book(
    conn: &mut PgConnection,
    world_id: Uuid,
    kind: &str,
    name: &str,
) -> QueryResult<Option<String>> {
    world_entry_deltas::table
        .filter(world_entry_deltas::world_id.eq(world_id))
        .filter(world_entry_deltas::compendium_id.is_null())
        .filter(world_entry_deltas::kind.eq(kind))
        .filter(world_entry_deltas::name.eq(name))
        .select(world_entry_deltas::written_beside_title)
        .first::<Option<String>>(conn)
        .optional()
        .map(Option::flatten)
}

/// Remove an addition this world kept after its book was switched off or
/// removed (decision 5), by the addition's id.
///
/// By id rather than by book, kind and name, because an addition whose book
/// was removed has no book to address it by. Only additions: a change or a
/// hide is restored on its book's page, and an id naming one is refused as
/// nothing to remove here rather than removed by a side door.
pub fn remove_kept_addition(
    conn: &mut PgConnection,
    caller: Uuid,
    world_id: Uuid,
    addition_id: Uuid,
) -> Result<Delta, DeltaError> {
    book_list::require_book_manager(conn, caller, world_id)?;
    refuse_if_paused(conn, world_id).map_err(BookListError::Paused)?;
    diesel::delete(
        world_entry_deltas::table
            .filter(world_entry_deltas::id.eq(addition_id))
            .filter(world_entry_deltas::world_id.eq(world_id))
            .filter(world_entry_deltas::form.eq(DeltaForm::Added)),
    )
    .returning(Delta::as_select())
    .get_result(conn)
    .optional()?
    .ok_or(DeltaError::NoSuchAddition)
}
